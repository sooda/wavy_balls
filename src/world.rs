use body::{Body, BodyShape, BodyConfig};
use nc;
use np;
use ode;
use std;
use math::*;
use std::rc::Rc;
use std::cell::RefCell;
use mesh;
use texture;

use nc::world::CollisionObject3;
use np::object::{WorldObject, RigidBodyHandle};

struct NearCallbackContext {
    world: ode::dWorldID,
    contact_group: ode::dJointGroupID,
}

unsafe extern "C" fn near_callback(user_data: *mut std::os::raw::c_void,
                                   o1: ode::dGeomID,
                                   o2: ode::dGeomID) {
    let i = 0;

    let b1 = ode::dGeomGetBody(o1);
    let b2 = ode::dGeomGetBody(o2);

    let ctx: &NearCallbackContext = &*(user_data as *const NearCallbackContext);

    const MAX_CONTACTS: usize = 1024;
    let mut contact: [ode::dContact; MAX_CONTACTS] = std::mem::zeroed();

    let numc = ode::dCollide(o1,
                             o2,
                             MAX_CONTACTS as i32,
                             &mut contact[0].geom,
                             std::mem::size_of::<ode::dContact>() as i32);

    for i in 0..numc {

        let contact = &mut contact[i as usize];

        // friction
        contact.surface.mu = 50.0;

        // rolling friction
        contact.surface.rho = 0.1;

        // rolling friction (spin direction, beyblade prevention)
        contact.surface.rhoN = 8000.0;

        // contact.surface.bounce = 0.0;
        // contact.surface.mode |= ode::dContactBounce as i32;
        contact.surface.mode |= ode::dContactRolling as i32;

        let id = ode::dJointCreateContact(ctx.world, ctx.contact_group, contact);
        ode::dJointAttach(id, b1, b2);
    }
}



pub struct World {
    ode_world: ode::dWorldID,
    ode_space: ode::dSpaceID,
    ode_contact_group: ode::dJointGroupID,
    bodies: Vec<Rc<RefCell<Body>>>,
    leftover_dt: f32,
}
impl World {
    pub fn new() -> World {

        let ode_world = unsafe {
            let w = ode::dWorldCreate();
            ode::dWorldSetGravity(w, 0.0, -GRAVITY as f64, 0.0);
            w
        };

        let ode_space = unsafe { ode::dHashSpaceCreate(std::ptr::null_mut()) };

        World {
            ode_world: ode_world,
            ode_space: ode_space,
            ode_contact_group: unsafe { ode::dJointGroupCreate(0) },
            leftover_dt: 0.0,
            bodies: Vec::new(),
        }
    }

    pub fn add_contact_handler<F>(&mut self, handler: F)
        where F: FnMut(&RigidBodyHandle<f32>, &RigidBodyHandle<f32>) + 'static
    {
    }

    pub fn add_body(&mut self,
                    mesh: Rc<mesh::Mesh>,
                    texture: Rc<texture::Texture>,
                    shape: BodyShape,
                    config: BodyConfig)
                    -> Rc<RefCell<Body>> {

        let ode_body = unsafe { ode::dBodyCreate(self.ode_world) };

        let ode_geom = match shape {
            BodyShape::Sphere { radius } => unsafe {
                ode::dCreateSphere(self.ode_space, radius as f64)
            },
            BodyShape::TriangleSoup { ref vertices, ref indices } => {
                unsafe {
                    let trimesh_data = ode::dGeomTriMeshDataCreate();

                    ode::dGeomTriMeshDataBuildDouble(trimesh_data,
                                                     vertices.as_ptr() as *const std::os::raw::c_void,
                                                     8 * 3, // vertex stride
                                                     vertices.len() as i32 / 3,
                                                     indices.as_ptr() as *const std::os::raw::c_void,
                                                     indices.len() as i32,
                                                     4 * 3);

                    ode::dCreateTriMesh(self.ode_space, trimesh_data, None, None, None)

                }
            }
        };

        println!("Create body {:?}", config);
        unsafe {
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
        };

        let body = Rc::new(RefCell::new(Body {
            mesh: mesh,
            shape: shape,
            texture: texture,
            config: config,
            ode_body: ode_body,
            ode_geom: ode_geom,
        }));
        self.bodies.push(body.clone());
        body
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, frame_dt: f32) {
        self.leftover_dt += frame_dt;

        while self.leftover_dt >= PHYS_DT {
            self.leftover_dt -= PHYS_DT;

            unsafe {
                let mut ctx = NearCallbackContext {
                    world: self.ode_world,
                    contact_group: self.ode_contact_group,
                };
                ode::dSpaceCollide(self.ode_space,
                                   &mut ctx as *mut _ as *mut std::os::raw::c_void,
                                   Some(near_callback));

                ode::dWorldStep(self.ode_world, PHYS_DT as f64);

                ode::dJointGroupEmpty(self.ode_contact_group);
            }
        }
    }
    pub fn bodies<'a>(&'a mut self) -> &'a mut Vec<Rc<RefCell<Body>>> {
        &mut self.bodies
    }
}
