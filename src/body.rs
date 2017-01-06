use math;
use std::rc::Rc;
use mesh::Mesh;

pub struct Body {
    pub position: math::Vec3,
    pub velocity: math::Vec3,
    pub force: math::Vec3,
    pub mesh: Rc<Mesh>,
}

impl Body {
    pub fn new(mesh: Rc<Mesh>) -> Body {
        Body {
            position: math::Vec3::new(0.0, 0.0, 0.0),
            velocity: math::Vec3::new(0.0, 0.0, 0.0),
            force: math::Vec3::new(0.0, 0.0, 0.0),
            mesh: mesh,
        }
    }
}
