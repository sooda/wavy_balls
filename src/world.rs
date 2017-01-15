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

pub struct World {
    ode_world: ode::dWorldID,
    ode_space: ode::dSpaceID,
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
            BodyShape::TriangleSoup { vertices, indices } => {
                unsafe {
                    let trimesh_data = ode::dGeomTriMeshDataCreate();

                    ode::dGeomTriMeshDataBuildDouble(trimesh_data,
                                                     vertices.as_ptr() as *const std::os::raw::c_void,
                                                     8 * 3, // vertex stride
                                                     vertices.len() as i32,
                                                     indices.as_ptr() as *const std::os::raw::c_void,
                                                     indices.len() as i32,
                                                     4 * 3);

                    ode::dCreateTriMesh(self.ode_space, trimesh_data, None, None, None)

                }
            }
        };

        unsafe {
            if !config.fixed {
                let mut mass: ode::dMass = std::mem::zeroed();
                ode::dMassSetSphere(&mut mass, config.density as f64, 1.0);
            }
        }

        unsafe {
            ode::dGeomSetBody(ode_geom, ode_body);
            if config.fixed {
                ode::dBodySetKinematic(ode_body);
            } else {
                ode::dBodySetDynamic(ode_body);
            }
        };

        let body = Rc::new(RefCell::new(Body {
            mesh: mesh,
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
                ode::dWorldStep(self.ode_world, PHYS_DT as f64);
            }
        }
    }
    pub fn bodies<'a>(&'a mut self) -> &'a mut Vec<Rc<RefCell<Body>>> {
        &mut self.bodies
    }
}
