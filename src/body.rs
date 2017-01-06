use math;

pub struct Body {
    pub position: math::Vec3,
    pub velocity: math::Vec3, // TODO add mesh, Rc<Mesh> perhaps?
}

impl Body {
    pub fn new() -> Body {
        Body {
            position: math::Vec3::new(0.0, 0.0, 0.0),
            velocity: math::Vec3::new(0.0, 0.0, 0.0),
        }
    }
}
