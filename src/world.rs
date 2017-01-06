use body::{Body, BodyShape};
use na;
use nc;
use std;
use math::*;

const PHYS_DT: f32 = 0.01;

pub struct World {
    cw: nc::world::CollisionWorld3<f32, usize>,
    bodies: Vec<Body>,
    dt_time_left: f32,
}

impl World {
    pub fn new() -> World {
        let mut cw = nc::world::CollisionWorld3::new(0.01, true);

        // let mut cg = nc::world::CollisionGroups::new();
        // cg.set_membership(&[COLL_WORLD]);
        // cg.set_whitelist(&[COLL_FOO]);

        World {
            cw: cw,
            bodies: Vec::new(),
            dt_time_left: 0.0,
        }
    }

    pub fn add_body(&mut self, body: Body) {
        let cg = nc::world::CollisionGroups::new();
        let shape_handle = match body.shape {
            BodyShape::Sphere { radius } => {
                let shape = nc::shape::Ball::new(radius);
                nc::shape::ShapeHandle::new(shape)
            }
            BodyShape::TriangleSoup(ref trimesh) => nc::shape::ShapeHandle::new(trimesh.clone()),
        };

        let uid = self.bodies.len();
        self.cw.deferred_add(uid,
                             Iso3::new(body.position, na::zero()),
                             shape_handle,
                             cg,
                             nc::world::GeometricQueryType::Contacts(0.0),
                             uid);
        self.bodies.push(body)
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, frame_dt: f32) {

        self.dt_time_left += frame_dt;

        while self.dt_time_left >= PHYS_DT {
            self.dt_time_left -= PHYS_DT;

            // Check collisions and accumulate forces
            for obj in self.bodies.iter_mut() {
                // continue;
                if !obj.fixed {
                    obj.force += Vec3::new(0.0, -9.80665, 0.0);
                }
            }

            for (uid, obj) in self.bodies.iter_mut().enumerate() {
                let mass = 1.0;
                obj.velocity += PHYS_DT * obj.force / mass;
                obj.position += PHYS_DT * obj.velocity; // euler was a geniose
                obj.force = Vec3::new(0.0, 0.0, 0.0);

                self.cw.deferred_set_position(uid, Iso3::new(obj.position, na::zero()));
            }

            self.cw.update();

            for (mut a, mut b, mut contact) in self.cw.contacts() {
                if self.bodies[b.data].fixed {
                    std::mem::swap(&mut a, &mut b);
                    contact.flip();
                }

                assert!(!self.bodies[b.data].fixed);

                let velocity_towards_surface =
                    0f32.min(na::dot(&self.bodies[b.data].velocity, &contact.normal));

                self.bodies[b.data].velocity -= contact.normal * velocity_towards_surface;
                self.bodies[b.data].position += contact.normal * contact.depth;
                //             self.bodies[b.data].position -= contact.normal * contact.depth;
            }
        }
    }

    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    pub fn bodies_mut(&mut self) -> &mut [Body] {
        &mut self.bodies
    }
}
