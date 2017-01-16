use body::{Body, BodyShape, BodyConfig};
use nc;
use np;
use math::*;
use std::rc::Rc;
use std::cell::RefCell;
use mesh;
use texture;

use nc::world::CollisionObject3;
use np::object::{WorldObject, RigidBodyHandle};

pub struct World {
    phys_world: np::world::World<f32>,
    leftover_dt: f32,
}

struct ContactHandler {
    callback: Box<FnMut(&RigidBodyHandle<f32>, &RigidBodyHandle<f32>)>,
}

impl ContactHandler {
    pub fn new<F>(callback: F) -> ContactHandler
        where F: FnMut(&RigidBodyHandle<f32>, &RigidBodyHandle<f32>) + 'static
    {
        ContactHandler { callback: Box::new(callback) }
    }
}

impl nc::narrow_phase::ContactHandler<Pnt3, Iso3, WorldObject<f32>> for ContactHandler {
    fn handle_contact_started(&mut self,
                              co1: &CollisionObject3<f32, WorldObject<f32>>,
                              co2: &CollisionObject3<f32, WorldObject<f32>>,
                              _contacts: &nc::narrow_phase::ContactAlgorithm3<f32>) {

        if co1.data.is_rigid_body() && co2.data.is_rigid_body() {

            let o1 = match co1.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let o2 = match co2.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            (self.callback)(o1, o2);
        }
    }
    fn handle_contact_stopped(&mut self,
                              _co1: &CollisionObject3<f32, WorldObject<f32>>,
                              _co2: &CollisionObject3<f32, WorldObject<f32>>) {
    }
}

struct SmoothContactHandler {
    rigid: Rc<RefCell<np::object::RigidBody<f32>>>,
    fixed: Rc<RefCell<np::object::RigidBody<f32>>>,
    num_touches: u8,
}

impl SmoothContactHandler {
    pub fn new(rigid: Rc<RefCell<np::object::RigidBody<f32>>>,
               fixed: Rc<RefCell<np::object::RigidBody<f32>>>)
               -> SmoothContactHandler {
        SmoothContactHandler {
            rigid: rigid,
            fixed: fixed,
            num_touches: 0,
        }
    }
    fn begin(&mut self,
             _rigid_co: &CollisionObject3<f32, WorldObject<f32>>,
             _fixed_co: &CollisionObject3<f32, WorldObject<f32>>) {
        self.num_touches += 1;
        println!("num touches {}", self.num_touches);
    }
    fn end(&mut self,
           _rigid_co: &CollisionObject3<f32, WorldObject<f32>>,
           _fixed_co: &CollisionObject3<f32, WorldObject<f32>>) {
        if self.num_touches > 0 {
            self.num_touches -= 1;
        }
        println!("num touches {}", self.num_touches);
    }
}

impl nc::narrow_phase::ContactHandler<Pnt3, Iso3, WorldObject<f32>> for SmoothContactHandler {
    fn handle_contact_started(&mut self,
                              co1: &CollisionObject3<f32, WorldObject<f32>>,
                              co2: &CollisionObject3<f32, WorldObject<f32>>,
                              _contacts: &nc::narrow_phase::ContactAlgorithm3<f32>) {
        if co1.data.is_rigid_body() && co2.data.is_rigid_body() {
            let o1 = match co1.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let o2 = match co2.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let rigid_index = self.rigid.borrow_mut().index();
            let fixed_index = self.fixed.borrow_mut().index();
            if o1.borrow_mut().index() == rigid_index && o2.borrow_mut().index() == fixed_index {
                self.begin(co1, co2);
            } else if o1.borrow_mut().index() == fixed_index &&
                      o2.borrow_mut().index() == rigid_index {
                self.begin(co2, co1);
            }
        }
    }
    fn handle_contact_stopped(&mut self,
                              co1: &CollisionObject3<f32, WorldObject<f32>>,
                              co2: &CollisionObject3<f32, WorldObject<f32>>) {
        if co1.data.is_rigid_body() && co2.data.is_rigid_body() {
            let o1 = match co1.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let o2 = match co2.data {
                WorldObject::RigidBody(ref handle) => handle,
                _ => {
                    panic!();
                }
            };
            let rigid_index = self.rigid.borrow_mut().index();
            let fixed_index = self.fixed.borrow_mut().index();
            if o1.borrow_mut().index() == rigid_index && o2.borrow_mut().index() == fixed_index {
                self.end(co1, co2);
            } else if o1.borrow_mut().index() == fixed_index &&
                      o2.borrow_mut().index() == rigid_index {
                self.end(co2, co1);
            }
        }
    }
}
impl World {
    pub fn new() -> World {
        let mut pw = np::world::World::new();
        pw.set_gravity(Vec3::new(0.0, -GRAVITY, 0.0));
        // println!("1st order {}, 2nd order {}",
        //        pw.constraints_solver().num_first_order_iter(),
        //         pw.constraints_solver().num_second_order_iter());
        // pw.constraints_solver().set_num_first_order_iter(20);
        // pw.constraints_solver().set_num_second_order_iter(20);

        World {
            phys_world: pw,
            leftover_dt: 0.0,
        }
    }

    pub fn set_smooth_collision(&mut self,
                                rigid: &Rc<RefCell<np::object::RigidBody<f32>>>,
                                fixed: &Rc<RefCell<np::object::RigidBody<f32>>>) {
        let handler = SmoothContactHandler::new(rigid.clone(), fixed.clone());
        self.phys_world.register_contact_handler("smooth_handler", handler);
    }

    pub fn add_contact_handler<F>(&mut self, handler: F)
        where F: FnMut(&RigidBodyHandle<f32>, &RigidBodyHandle<f32>) + 'static
    {

        let handler = ContactHandler::new(handler);
        self.phys_world.register_contact_handler("default_handler", handler);

    }

    pub fn add_body(&mut self,
                    mesh: Rc<mesh::Mesh>,
                    texture: Rc<texture::Texture>,
                    shape: BodyShape,
                    config: BodyConfig)
                    -> Rc<RefCell<np::object::RigidBody<f32>>> {

        let mut rigid_body = if config.fixed {
            match shape {
                BodyShape::Sphere { radius } => {
                    np::object::RigidBody::new_static(nc::shape::Ball::new(radius),
                                                      config.restitution,
                                                      config.friction)
                }
                BodyShape::TriangleSoup(ref trimesh) => {
                    np::object::RigidBody::new_static(trimesh.clone(),
                                                      config.restitution,
                                                      config.friction)
                }
            }
        } else {
            match shape {
                BodyShape::Sphere { radius } => {
                    np::object::RigidBody::new_dynamic(nc::shape::Ball::new(radius),
                                                       config.density,
                                                       config.restitution,
                                                       config.friction)
                }
                BodyShape::TriangleSoup(ref _trimesh) => {
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
            texture: texture,
            config: config,
        })));

        rigid_body.set_margin(0.5);

        self.phys_world.add_rigid_body(rigid_body)
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, frame_dt: f32) {
        self.leftover_dt += frame_dt;

        while self.leftover_dt >= PHYS_DT {
            self.leftover_dt -= PHYS_DT;

            // apply damping to all phys objects
            for body in self.phys_world.rigid_bodies() {
                let mut body = body.borrow_mut();

                let ang = body.ang_vel();
                body.set_ang_vel(ang * 0.9997);
            }

            self.phys_world.step(PHYS_DT);
        }
    }

    pub fn rigid_bodies(&self) -> np::world::RigidBodies<f32> {
        self.phys_world.rigid_bodies()
    }
    pub fn phys_world(&mut self) -> &mut np::world::World<f32> {
        &mut self.phys_world
    }
}
