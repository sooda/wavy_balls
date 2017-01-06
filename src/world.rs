use body::Body;

pub struct World {
    bodies: Vec<Body>,
    time: f32,
}

impl World {
    pub fn new() -> World {
        World {
            bodies: Vec::new(),
            time: 0.0,
        }
    }

    pub fn add_body(&mut self, body: Body) {
        self.bodies.push(body)
    }

    pub fn step(&mut self, dt: f32) {
        // Advance the world state forwards by dt seconds
    }
}
