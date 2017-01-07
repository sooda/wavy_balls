use body::{Body, BodyShape};
use na;
use nc;
use np;
use glium;
use math::*;
use std::rc::Rc;
use std::cell::RefCell;
use mesh;

pub struct World {
    phys_world: np::world::World<f32>,
    leftover_dt: f32,
}

struct ContactHandler {
    callback: Box<FnMut(&np::object::RigidBodyHandle<f32>,
                        &np::object::RigidBodyHandle<f32>)>,
}

impl ContactHandler {
    pub fn new<F>(callback: F) -> ContactHandler
        where F: FnMut(&np::object::RigidBodyHandle<f32>,
                       &np::object::RigidBodyHandle<f32>) + 'static
    {
        ContactHandler { callback: Box::new(callback) }
    }
}

impl nc::narrow_phase::ContactHandler<Pnt3, Iso3, np::object::WorldObject<f32>> for ContactHandler {
    fn handle_contact_started(&mut self,
                              co1: &nc::world::CollisionObject<Pnt3,
                                                               Iso3,
                                                               np::object::WorldObject<f32>>,
                              co2: &nc::world::CollisionObject<Pnt3,
                                                               Iso3,
                                                               np::object::WorldObject<f32>>,
                              contacts: &nc::narrow_phase::ContactAlgorithm<Pnt3, Iso3>) {

        if co1.data.is_rigid_body() && co2.data.is_rigid_body() {

            let o1 = match co1.data {
                np::object::WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let o2 = match co2.data {
                np::object::WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };

            (self.callback)(o1, o2);
        }
    }
    fn handle_contact_stopped(&mut self,
                              co1: &nc::world::CollisionObject<Pnt3,
                                                               Iso3,
                                                               np::object::WorldObject<f32>>,
                              co2: &nc::world::CollisionObject<Pnt3,
                                                               Iso3,
                                                               np::object::WorldObject<f32>>) {
    }
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

    pub fn add_contact_handler<F>(&mut self, handler: F)
        where F: FnMut(&np::object::RigidBodyHandle<f32>,
                       &np::object::RigidBodyHandle<f32>) + 'static
    {

        let handler = ContactHandler::new(handler);
        self.phys_world.register_contact_handler("default_handler", handler);

    }

    pub fn add_body(&mut self,
                    mesh: Rc<mesh::Mesh>,
                    texture: Rc<glium::texture::Texture2d>,
                    shape: BodyShape,
                    fixed: bool)
                    -> Rc<RefCell<np::object::RigidBody<f32>>> {

        let restitution = 0.01;
        let friction = 0.7;

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
            texture: texture,
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
