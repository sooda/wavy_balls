use body::Body;
use nc;
use std::slice;
use math;

enum PhysRole {
    Body(u32),
}

pub struct World {
    cw: nc::world::CollisionWorld3<f32, PhysRole>,
    bodies: Vec<Body>,
    time: f32,
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
            time: 0.0,
        }
    }

    pub fn add_body(&mut self, body: Body) {
        self.bodies.push(body)
    }

    // Advance the world state forwards by dt seconds
    pub fn step(&mut self, dt: f32) {

        self.time += dt;

        // Check collisions and accumulate forces
        for obj in self.bodies.iter_mut() {
            obj.force += math::Vec3::new(0.0, -9.80665, 0.0);
        }

        for obj in self.bodies.iter_mut() {
            let mass = 1.0;
            obj.velocity += dt * obj.force / mass;
            obj.position += dt * obj.velocity; // euler was a geniose
        }

    }

    pub fn bodies(&self) -> slice::Iter<Body> {
        self.bodies.iter()
    }
}
