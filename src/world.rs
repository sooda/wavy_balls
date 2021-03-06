use body::{Body, BodyShape, BodyConfig, BODY_CATEGORY_TERRAIN_BIT, BODY_COLLIDE_TERRAIN};
use glium;
use glium::backend::Facade;
use mesh::Mesh;
use ode;
use na::Norm;
use std;
use math::*;
use std::rc::Rc;
use std::cell::RefCell;
use mesh;
use texture;

unsafe extern "C" fn near_callback(user_data: *mut std::os::raw::c_void,
                                   ode_g1: ode::dGeomID,
                                   ode_g2: ode::dGeomID) {
    let ode_b1 = ode::dGeomGetBody(ode_g1);
    let ode_b2 = ode::dGeomGetBody(ode_g2);
    let mut world: &mut World = &mut *(user_data as *mut World);

    let (mut b1, mut b2) = if ode_b1 != std::ptr::null_mut() && ode_b2 != std::ptr::null_mut() {
        // find references to Body instances
        let mut bi1 = ode::dBodyGetData(ode_b1) as u64;
        let mut bi2 = ode::dBodyGetData(ode_b2) as u64;

        // bi1 and bi2 are the ids of some bodies in the bodies vec
        // find indices of those bodies first and then obtain two mutable references to them
        for (i, obj) in world.bodies.iter().enumerate() {
            if obj.borrow().id == bi1 {
                bi1 = i as u64;
                break;
            }
        }
        for (i, obj) in world.bodies.iter().enumerate() {
            if obj.borrow().id == bi2 {
                bi2 = i as u64;
                break;
            }
        }
        if bi1 == bi2 {
            println!("{}", bi1);
            return;
        }
        assert!(bi1 != bi2);

        if bi1 < bi2 {
            let (begin, end) = world.bodies.split_at_mut(bi2 as usize);
            (Some(begin[bi1 as usize].borrow_mut()), Some(end[0].borrow_mut()))
        } else {
            let (begin, end) = world.bodies.split_at_mut(bi1 as usize);
            (Some(end[0].borrow_mut()), Some(begin[bi2 as usize].borrow_mut()))
        }
    } else {
        (None, None)
    };

    const MAX_CONTACTS: usize = 100;
    let mut contact: [ode::dContact; MAX_CONTACTS] = std::mem::zeroed();

    let numc = ode::dCollide(ode_g1,
                             ode_g2,
                             MAX_CONTACTS as i32,
                             &mut contact[0].geom,
                             std::mem::size_of::<ode::dContact>() as i32);

    for i in 0..numc {

        let contact = &mut contact[i as usize];
        // friction
        contact.surface.mu = 50.0;

        // rolling friction
        // contact.surface.rho = 0.1;

        // rolling friction (spin direction, beyblade prevention)
        // contact.surface.rhoN = 8000.0;

        // contact.surface.bounce = 0.0;
        // contact.surface.mode |= ode::dContactBounce as i32;

        // contact.surface.mode |= ode::dContactRolling as i32;

        let mut ignore_collision = false;
        // NOTE: handler if skipped if some geoms have no associated body
        if let (&mut Some(ref mut b1), &mut Some(ref mut b2)) = (&mut b1, &mut b2) {
            for mut handler in world.contact_handlers.iter_mut() {
                if handler(&mut *b1, &mut *b2, contact) {
                    ignore_collision = true;
                }
            }
        }
        if !ignore_collision {
            let id = ode::dJointCreateContact(world.ode_world, world.ode_contact_group, contact);
            ode::dJointAttach(id, ode_b1, ode_b2);
        }
    }
}

type ContactHandlerT = Box<FnMut(&mut Body, &mut Body, &mut ode::dContact) -> bool + 'static>;

unsafe extern "C" fn heightfield_callback(user_data: *mut std::os::raw::c_void,
                                          x: i32,
                                          z: i32)
                                          -> f64 {
    let world: &mut World = &mut *(user_data as *mut World);
    world.heightfield[(x + z * world.heightfield_resolution.0) as usize] as f64
}

pub struct World {
    ode_world: ode::dWorldID,
    ode_space: ode::dSpaceID,
    ode_contact_group: ode::dJointGroupID,
    bodies: Vec<Rc<RefCell<Body>>>,
    leftover_dt: f32,
    accum_dt: f32,
    contact_handlers: Vec<ContactHandlerT>,
    body_id_counter: u64,

    landscape_mesh: Option<Rc<RefCell<Mesh>>>,

    pub heightfield: Vec<f32>,
    pub heightfield_origin: Vec<f32>,
    pub heightfield_velocity: Vec<f32>,
    pub heightfield_resolution: (i32, i32),
    pub heightfield_idx: Vec<usize>,
    pub heightfield_scale: f32,
}

impl World {
    pub fn new(scale: f32) -> World {

        let ode_world = unsafe {
            let w = ode::dWorldCreate();
            ode::dWorldSetGravity(w, 0.0, -GRAVITY as f64, 0.0);
            w
        };

        let ode_space = unsafe { ode::dHashSpaceCreate(std::ptr::null_mut()) };

        // Set damping parameters
        unsafe {
            ode::dWorldSetDamping(ode_world, 0.0015 /* linear */, 0.0015 /* angular */);
        };

        World {
            ode_world: ode_world,
            ode_space: ode_space,
            ode_contact_group: unsafe { ode::dJointGroupCreate(0) },
            leftover_dt: 0.0,
            accum_dt: 0.0,
            bodies: Vec::new(),
            contact_handlers: Vec::new(),
            body_id_counter: 0,
            heightfield: Vec::new(),
            heightfield_origin: Vec::new(),
            heightfield_velocity: Vec::new(),
            heightfield_resolution: (0, 0),
            heightfield_scale: scale,
            heightfield_idx: Vec::new(),
            landscape_mesh: None,
        }
    }

    pub fn ode_world(&self) -> ode::dWorldID {
        self.ode_world
    }

    pub fn ode_space(&self) -> ode::dSpaceID {
        self.ode_space
    }

    pub fn add_contact_handler(&mut self, handler: ContactHandlerT) {
        self.contact_handlers.push(handler);
    }

    pub fn add_body(&mut self,
                    mesh: Rc<RefCell<mesh::Mesh>>,
                    texture: Rc<texture::Texture>,
                    shape: Rc<BodyShape>,
                    config: BodyConfig)
                    -> Rc<RefCell<Body>> {

        let ode_body = unsafe { ode::dBodyCreate(self.ode_world) };

        let ode_geom = match *shape {
            BodyShape::Sphere { radius } => unsafe {
                ode::dCreateSphere(self.ode_space, radius as f64)
            },
            BodyShape::TriangleSoup { ref vertices, ref indices } => {
                unsafe {
                    let trimesh_data = ode::dGeomTriMeshDataCreate();

                    ode::dGeomTriMeshDataBuildDouble(trimesh_data,
                                                     vertices.as_ptr() as
                                                     *const std::os::raw::c_void,
                                                     8 * 3, // vertex stride
                                                     vertices.len() as i32 / 3,
                                                     indices.as_ptr() as
                                                     *const std::os::raw::c_void,
                                                     indices.len() as i32,
                                                     4 * 3);

                    ode::dCreateTriMesh(self.ode_space, trimesh_data, None, None, None)

                }
            }
            _ => unreachable!(), // heightfield is special
        };

        println!("Create body {:?}", config);
        unsafe {
            ode::dBodySetData(ode_body, self.body_id_counter as *mut std::os::raw::c_void);
            ode::dBodySetPosition(ode_body, 0.0, 0.0, 0.0);
            if config.fixed {
                ode::dBodySetKinematic(ode_body);
            } else {
                ode::dBodySetDynamic(ode_body);
                let mut mass: ode::dMass = std::mem::zeroed();
                ode::dMassSetSphere(&mut mass, config.density as f64, 1.0);
                ode::dBodySetMass(ode_body, &mass);
            }
            ode::dGeomSetBody(ode_geom, ode_body);

            ode::dGeomSetCategoryBits(ode_geom, config.category_bits);
            ode::dGeomSetCollideBits(ode_geom, config.collide_bits);
        };

        let body = Rc::new(RefCell::new(Body {
            mesh: Some(mesh),
            shape: shape,
            texture: Some(texture),
            config: config.clone(),
            ode_body: ode_body,
            ode_geom: ode_geom,
            id: self.body_id_counter,
            collide_sound: config.collide_sound,
            shaded: false,
        }));
        self.bodies.push(body.clone());
        self.body_id_counter += 1;
        body
    }

    pub fn setup_heightfield<F: Facade>(&mut self,
                                        f: &F,
                                        texture: &glium::texture::RawImage2d<'static, u8>,
                                        visible_texture: Rc<texture::Texture>)
                                        -> Rc<RefCell<Body>> {

        // create mesh based on texture

        let (mesh, reso, hfield, idx) = Mesh::from_texture(f, texture, self.heightfield_scale);
        let mesh = mesh.expect("mesh load fail");

        self.heightfield_idx = idx;


        self.heightfield = hfield.clone();
        self.heightfield_velocity = vec![0.0; hfield.len()];
        self.heightfield_origin = hfield;
        self.heightfield_resolution = reso;

        let heightfield_data = unsafe { ode::dGeomHeightfieldDataCreate() };

        let scale = 1.0; // "vertical height scale multiplier"
        let offset = 0.0f64; // vetical height offset
        let thickness = 0.1;
        let wrap = false as i32; // whether to wrap the heightfield infinitely

        unsafe {
            ode::dGeomHeightfieldDataBuildCallback(heightfield_data,
                                                   // user ptr is self
                                                   self as *mut _ as *mut std::os::raw::c_void,
                                                   Some(heightfield_callback),
                                                   (self.heightfield_resolution.0 as f64 - 1.0) *
                                                   self.heightfield_scale as f64,
                                                   (self.heightfield_resolution.1 as f64 - 1.0) *
                                                   self.heightfield_scale as f64,
                                                   self.heightfield_resolution.0,
                                                   self.heightfield_resolution.1,
                                                   scale,
                                                   offset,
                                                   thickness,
                                                   wrap);
            ode::dGeomHeightfieldDataSetBounds(heightfield_data,
                                               -5000.0, // min height
                                               5000.0 /* max height */);
        };

        let geom =
            unsafe { ode::dCreateHeightfield(self.ode_space, heightfield_data, true as i32) };

        unsafe {
            // FIXME: use add_body
            let ode_body = ode::dBodyCreate(self.ode_world);
            ode::dGeomSetBody(geom, ode_body);
            ode::dBodySetData(ode_body, self.body_id_counter as *mut std::os::raw::c_void);
            ode::dBodySetKinematic(ode_body);
            // ode::dGeomSetBody(geom, std::ptr::null_mut());
            ode::dGeomSetPosition(geom, 0.0, 0.0, 0.0);
            ode::dBodySetPosition(ode_body, 0.0, 0.0, 0.0);
            ode::dGeomSetCategoryBits(geom, BODY_CATEGORY_TERRAIN_BIT);
            ode::dGeomSetCollideBits(geom, BODY_COLLIDE_TERRAIN);
            let mesh = Rc::new(RefCell::new(mesh));
            self.landscape_mesh = Some(mesh.clone());
            let body = Rc::new(RefCell::new(Body {
                mesh: Some(mesh),
                shape: Rc::new(BodyShape::HeightField),
                texture: Some(visible_texture),
                config: Default::default(),
                ode_body: ode_body,
                ode_geom: geom,
                id: self.body_id_counter,
                collide_sound: None,
                shaded: true,
            }));
            self.body_id_counter += 1;
            self.bodies.push(body.clone());
            body
        }
    }

    pub fn del_body(&mut self, body_id: u64 /* body: &Body */) {
        // assume it's found because it's added earlier
        // let idx = self.bodies.iter().position(|ref x| *x.borrow() == *body).unwrap();
        let idx = self.bodies.iter().position(|ref x| x.borrow().id == body_id).unwrap();
        unsafe {
            ode::dSpaceRemove(self.ode_space, self.bodies[idx].borrow().ode_geom);
            // Should do these when the body gets dropped
            // ode::dGeomDestroy(body.ode_geom);
            // ode::dBodyDestroy(body.ode_body);
            //
        }
        self.bodies.remove(idx);
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self,
                frame_dt: f32,
                player_position: Vec3,
                player_action: bool,
                _p: (f32, f32, f32)) {
        self.leftover_dt += frame_dt;

        while self.leftover_dt >= PHYS_DT {
            self.leftover_dt -= PHYS_DT;
            self.accum_dt += PHYS_DT;

            let player_velocity = self.bodies[0].borrow_mut().get_linear_velocity();
            let effect_position = player_position - player_velocity.normalize() * 8.0;
            let amplitude = Vec3::new(player_velocity.x, 0.0, player_velocity.z).norm() / 200.0;

            if player_action || amplitude > 0.03 {
                let amplitude = amplitude - 0.03;
                // self.heightfield_velocity[p] += 1.0;

                let offset =
                    effect_position +
                    Vec3::new(self.heightfield_resolution.0 as f32 *
                              self.heightfield_scale as f32 * 0.5,
                              0.0,
                              self.heightfield_resolution.1 as f32 *
                              self.heightfield_scale as f32 * 0.5);


                for xx in -3..4 {
                    for zz in -3..4 {
                        let xcoord = (offset.x / self.heightfield_scale).floor() as i32 + xx;
                        let zcoord = (offset.z / self.heightfield_scale).floor() as i32 + zz;
                        let p = xcoord + zcoord * self.heightfield_resolution.0 as i32;
                        if p >= 0 && (p as usize) < self.heightfield.len() {
                            self.heightfield_velocity[p as usize] +=
                                amplitude * 1.0 / (1.0 + (xx * xx + zz * zz) as f32);
                        }
                    }
                }
            }

            const WAVE_DT: f32 = 0.1;

            for v in self.heightfield_velocity.iter_mut() {
                *v *= 0.998;
            }

            for x in 0..self.heightfield_resolution.0 {
                for z in 0..self.heightfield_resolution.1 {

                    let stride = self.heightfield_resolution.0 as usize;
                    let i = (x as usize + z as usize * stride) as usize;

                    let neighs = 0.0 +
                                 if x > 0 {
                        self.heightfield[i - 1] - self.heightfield_origin[i - 1]
                    } else {
                        0.0
                    } +
                                 if z > 0 {
                        self.heightfield[i - stride] - self.heightfield_origin[i - stride]
                    } else {
                        0.0
                    } +
                                 if x + 1 < self.heightfield_resolution.0 {
                        self.heightfield[i + 1] - self.heightfield_origin[i + 1]
                    } else {
                        0.0
                    } +
                                 if z + 1 < self.heightfield_resolution.1 {
                        self.heightfield[i + stride] - self.heightfield_origin[i + stride]
                    } else {
                        0.0
                    };

                    self.heightfield_velocity[i] +=
                        (neighs / 4.0 - (self.heightfield[i] - self.heightfield_origin[i])) *
                        WAVE_DT;
                }
            }

            for ((x, o), v) in self.heightfield
                .iter_mut()
                .zip(self.heightfield_origin.iter())
                .zip(self.heightfield_velocity.iter_mut()) {
                *v += (*o - *x) * WAVE_DT * 0.001;
            }
            for (x, v) in self.heightfield.iter_mut().zip(self.heightfield_velocity.iter()) {
                *x += *v * WAVE_DT;
            }
            unsafe {
                ode::dSpaceCollide(self.ode_space,
                                   self as *mut _ as *mut std::os::raw::c_void,
                                   Some(near_callback));

                ode::dWorldStep(self.ode_world, PHYS_DT as f64);
                ode::dJointGroupEmpty(self.ode_contact_group);
            }
        }

        // deform mesh based on heightfield

        let &mut World { ref mut landscape_mesh,
                         ref heightfield,
                         // ref heightfield_origin,
                         ref heightfield_idx,
                         ref heightfield_velocity,
                         ref heightfield_resolution,
                         .. } = self;
        let mut mesh = landscape_mesh.as_mut().unwrap().borrow_mut();

        mesh.update_mesh(|orig_verts, new_verts| {
            for (index, (_orig_vert, gpu_vert)) in orig_verts.iter()
                .zip(new_verts.iter_mut())
                .enumerate() {

                use na::Norm;
                let hi = heightfield_idx[index];
                // let offset = heightfield[hi] - heightfield_origin[hi];
                // let velo = heightfield_velocity[hi];

                // if offset.abs() < 0.01 {
                //    *gpu_vert = *orig_vert;
                //    continue;
                // }

                let xi = heightfield_idx[index] % heightfield_resolution.0 as usize;
                let zi = heightfield_idx[index] / heightfield_resolution.0 as usize;
                let h = heightfield[hi];

                let xm = if xi > 0 { heightfield[hi - 1] } else { 0.0 };
                let xp = if xi + 1 < heightfield_resolution.0 as usize {
                    heightfield[hi + 1]
                } else {
                    0.0
                };
                let zm = if zi > 0 {
                    heightfield[hi - heightfield_resolution.0 as usize]
                } else {
                    0.0
                };
                let zp = if zi + 1 < heightfield_resolution.1 as usize {
                    heightfield[hi + heightfield_resolution.0 as usize]
                } else {
                    0.0
                };

                let normal = Vec3::new((xm - h) + (h - xp), 1.0, (zm - h) + (h - zp));
                let normal = normal.normalize();
                gpu_vert.position[1] = heightfield[hi];
                gpu_vert.normal[0] = normal.x;
                gpu_vert.normal[1] = normal.y;
                gpu_vert.normal[2] = normal.z;

                let offset = heightfield_velocity[hi];

                let offset = offset * 0.2;

                gpu_vert.color_tint[0] = offset;
                gpu_vert.color_tint[1] = offset;
                gpu_vert.color_tint[2] = offset;
            }
        });
    }
    pub fn bodies<'a>(&'a self) -> &'a Vec<Rc<RefCell<Body>>> {
        &self.bodies
    }
}
