use body::{Body, BodyShape};
use na;
use nc;
use np;
use math::*;
use std::rc::Rc;
use std::cell::RefCell;
use mesh;

pub struct World {
    phys_world: np::world::World<f32>,
    leftover_dt: f32,
}

impl World {
    pub fn new() -> World {
        let mut pw = np::world::World::new();
        pw.set_gravity(Vec3::new(0.0, -GRAVITY, 0.0));
        World {
            phys_world: pw,
            leftover_dt: 0.0,
        }
    }

    pub fn add_body(&mut self,
                    mesh: Rc<mesh::Mesh>,
                    shape: BodyShape,
                    fixed: bool)
                    -> Rc<RefCell<np::object::RigidBody<f32>>> {

        let restitution = 0.01;
        let friction = 10.0;

        let mut rigid_body = if fixed {
            match shape {
                BodyShape::Sphere { radius } => {
                    np::object::RigidBody::new_static(nc::shape::Ball::new(radius),
                                                      restitution,
                                                      friction)
                }
                BodyShape::TriangleSoup(ref trimesh) => {
                    np::object::RigidBody::new_static(trimesh.clone(), restitution, friction)
                }
            }
        } else {
            let density = 1.0;
            match shape {
                BodyShape::Sphere { radius } => {
                    np::object::RigidBody::new_dynamic(nc::shape::Ball::new(radius),
                                                       density,
                                                       restitution,
                                                       friction)
                }
                BodyShape::TriangleSoup(ref trimesh) => {
                    unimplemented!();
                    // np::object::RigidBody::new_dynamic(trimesh.clone(),
                    // density,
                    // restitution,
                    // friction)
                }
            }
        };

        rigid_body.set_user_data(Some(Box::new(Body {
            mesh: mesh,
            fixed: fixed,
        })));

        self.phys_world.add_rigid_body(rigid_body)
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, frame_dt: f32) {

        self.leftover_dt += frame_dt;

        while self.leftover_dt >= PHYS_DT {
            self.leftover_dt -= PHYS_DT;
            self.phys_world.step(PHYS_DT);
        }
    }

    pub fn rigid_bodies(&self) -> np::world::RigidBodies<f32> {
        self.phys_world.rigid_bodies()
    }
}
