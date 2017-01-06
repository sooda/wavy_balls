use body::Body;
use nc;
use std::slice;

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

        for obj in self.bodies.iter_mut() {
            obj.position += obj.velocity * dt; // euler was a geniose
        }

        // Check collisions

    }

    pub fn bodies(&self) -> slice::Iter<Body> {
        self.bodies.iter()
    }
}
