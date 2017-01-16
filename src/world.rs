use body::{Body, BodyShape, BodyConfig};
use ode;
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

    // find references to Body instances
    let mut bi1 = ode::dBodyGetData(ode_b1) as u64;
    let mut bi2 = ode::dBodyGetData(ode_b2) as u64;

    // bi1 and bi2 are the ids of some bodies in the bodies vec
    // find indices of those bodies first and then obtain two mutable references to them
    for (i, obj) in world.bodies.iter().enumerate() {
        if obj.borrow_mut().id == bi1 {
            bi1 = i as u64;
            break;
        }
    }
    for (i, obj) in world.bodies.iter().enumerate() {
        if obj.borrow_mut().id == bi2 {
            bi2 = i as u64;
            break;
        }
    }
    assert!(bi1 != bi2);

    let (mut b1, mut b2) = if bi1 < bi2 {
        let (begin, end) = world.bodies.split_at_mut(bi2 as usize);
        (begin[bi1 as usize].borrow_mut(), end[0].borrow_mut())
    } else {
        let (begin, end) = world.bodies.split_at_mut(bi1 as usize);
        (end[0].borrow_mut(), begin[bi2 as usize].borrow_mut())
    };

    const MAX_CONTACTS: usize = 1024;
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
        contact.surface.rho = 0.1;

        // rolling friction (spin direction, beyblade prevention)
        contact.surface.rhoN = 8000.0;

        // contact.surface.bounce = 0.0;
        // contact.surface.mode |= ode::dContactBounce as i32;
        contact.surface.mode |= ode::dContactRolling as i32;

        for mut handler in world.contact_handlers.iter_mut() {
            handler(&mut *b1, &mut *b2, contact);
        }
        let id = ode::dJointCreateContact(world.ode_world, world.ode_contact_group, contact);
        ode::dJointAttach(id, ode_b1, ode_b2);
    }
}


type ContactHandlerT = Box<FnMut(&mut Body, &mut Body, &mut ode::dContact) + 'static>;

pub struct World {
    ode_world: ode::dWorldID,
    ode_space: ode::dSpaceID,
    ode_contact_group: ode::dJointGroupID,
    bodies: Vec<Rc<RefCell<Body>>>,
    leftover_dt: f32,
    contact_handlers: Vec<ContactHandlerT>,
    body_id_counter: u64,
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
            contact_handlers: Vec::new(),
            body_id_counter: 0,
        }
    }

    pub fn ode_space(&self) -> ode::dSpaceID {
        self.ode_space
    }

    pub fn add_contact_handler(&mut self, handler: ContactHandlerT) {
        self.contact_handlers.push(handler);
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
        };

        let body = Rc::new(RefCell::new(Body {
            mesh: mesh,
            shape: shape,
            texture: texture,
            config: config,
            ode_body: ode_body,
            ode_geom: ode_geom,
            id: self.body_id_counter,
        }));
        self.bodies.push(body.clone());
        self.body_id_counter += 1;
        body
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, frame_dt: f32) {
        self.leftover_dt += frame_dt;

        while self.leftover_dt >= PHYS_DT {
            self.leftover_dt -= PHYS_DT;

            unsafe {
                ode::dSpaceCollide(self.ode_space,
                                   self as *mut _ as *mut std::os::raw::c_void,
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
