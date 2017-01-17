use obj;
use mesh;
use texture;
use na;
use ode;
use math::*;
use errors::*;
use std::rc::Rc;
use std::path::Path;

pub enum BodyShape {
    Sphere { radius: f32 },
    TriangleSoup {
        vertices: Vec<f64>,
        indices: Vec<u32>,
    },
}

impl BodyShape {
    #[allow(dead_code)]
    pub fn from_obj<P: AsRef<Path> + ?Sized>(path: &P) -> Result<BodyShape> {
        let (positions, _normals, _texcoord) =
            obj::load_obj(path).chain_err(|| "unable to load .obj")?;

        BodyShape::from_vertices(positions)
    }

    pub fn from_vertices(positions: Vec<Pnt3>) -> Result<BodyShape> {
        let mut vertices = Vec::with_capacity(positions.len() * 3);
        for v in positions.iter() {
            vertices.push(v.x as f64);
            vertices.push(v.y as f64);
            vertices.push(v.z as f64);
        }
        let indices = (0u32..positions.len() as u32).collect();
        Ok(BodyShape::TriangleSoup {
            vertices: vertices,
            indices: indices,
        })
    }
}

#[derive(Debug)]
pub struct BodyConfig {
    pub fixed: bool,
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl Default for BodyConfig {
    fn default() -> Self {
        BodyConfig {
            fixed: false,
            friction: 0.9,
            restitution: 0.0,
            density: 1.0,
        }
    }
}

pub struct Body {
    pub mesh: Rc<mesh::Mesh>,
    pub texture: Rc<texture::Texture>,
    pub config: BodyConfig,
    pub shape: BodyShape, // NOTE: holds memory of TriMesh!
    pub ode_body: ode::dBodyID,
    pub ode_geom: ode::dGeomID,
    pub id: u64,
}

impl Body {
    pub fn get_position(&self) -> Vec3 {
        unsafe {
            let v = ode::dBodyGetPosition(self.ode_body);
            Vec3::new(*v.offset(0) as f32,
                      *v.offset(1) as f32,
                      *v.offset(2) as f32)
        }
    }
    pub fn set_position(&mut self, pos: Vec3) {
        unsafe {
            ode::dBodySetPosition(self.ode_body, pos.x as f64, pos.y as f64, pos.z as f64);
        }
    }
    pub fn get_linear_velocity(&mut self) -> Vec3 {
        unsafe {
            let v = ode::dBodyGetLinearVel(self.ode_body);
            Vec3::new(*v.offset(0) as f32,
                      *v.offset(1) as f32,
                      *v.offset(2) as f32)
        }
    }
    pub fn set_linear_velocity(&mut self, vel: Vec3) {
        unsafe {
            ode::dBodySetLinearVel(self.ode_body, vel.x as f64, vel.y as f64, vel.z as f64);
        }
    }
    pub fn add_torque(&mut self, torque: Vec3) {
        unsafe {
            ode::dBodyAddTorque(self.ode_body,
                                torque.x as f64,
                                torque.y as f64,
                                torque.z as f64)
        }
    }
    pub fn add_force(&mut self, force: Vec3) {
        unsafe {
            ode::dBodyAddForce(self.ode_body,
                                force.x as f64,
                                force.y as f64,
                                force.z as f64)
        }
    }
    // 0  1  2  3
    // 4  5  6  7
    // 8  9  10 11
    // 12 13 14 15
    pub fn get_posrot_homogeneous(&mut self) -> na::Matrix4<f32> {
        unsafe {
            let pos = ode::dBodyGetPosition(self.ode_body);
            let rot = ode::dBodyGetRotation(self.ode_body);
            na::Matrix4::new(*rot.offset(0) as f32,
                             *rot.offset(1) as f32,
                             *rot.offset(2) as f32,
                             *pos.offset(0) as f32,
                             *rot.offset(4) as f32,
                             *rot.offset(5) as f32,
                             *rot.offset(6) as f32,
                             *pos.offset(1) as f32,
                             *rot.offset(8) as f32,
                             *rot.offset(9) as f32,
                             *rot.offset(10) as f32,
                             *pos.offset(2) as f32,
                             0.0,
                             0.0,
                             0.0,
                             1.0)
        }
    }
}
