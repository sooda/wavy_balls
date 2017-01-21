use obj;
use mesh;
use texture;
use na;
use ode;
use math::*;
use errors::*;
use std::rc::Rc;
use std::path::Path;
use std;

pub enum BodyShape {
    Sphere { radius: f32 },
    TriangleSoup {
        vertices: Vec<f64>,
        indices: Vec<u32>,
    },
    HeightField,
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
    pub category_bits: u64,
    pub collide_bits: u64,
}

// individual bits for where object belongs to
pub const BODY_CATEGORY_PLAYER_BIT: u64 = 1 << 0;
pub const BODY_CATEGORY_OBJS_BIT: u64 = 1 << 1;
pub const BODY_CATEGORY_TERRAIN_BIT: u64 = 1 << 2;
pub const BODY_CATEGORY_GEAR_BIT: u64 = 1 << 3;

pub const BODY_CATEGORY_ALL_BIT: u64 = 0xffffffff;

// per-type bitmasks about which other categories the particular category can collide with
pub const BODY_COLLIDE_PLAYER: u64 = BODY_CATEGORY_ALL_BIT;
pub const BODY_COLLIDE_OBJS: u64 = BODY_CATEGORY_ALL_BIT & !BODY_CATEGORY_GEAR_BIT;
pub const BODY_COLLIDE_TERRAIN: u64 = BODY_CATEGORY_PLAYER_BIT | BODY_CATEGORY_OBJS_BIT;
pub const BODY_COLLIDE_GEAR: u64 = BODY_CATEGORY_PLAYER_BIT;

impl Default for BodyConfig {
    fn default() -> Self {
        BodyConfig {
            fixed: false,
            friction: 0.9,
            restitution: 0.0,
            density: 1.0,
            category_bits: BODY_CATEGORY_OBJS_BIT,
            collide_bits: BODY_COLLIDE_OBJS,
        }
    }
}

pub struct Body {
    pub mesh: Option<Rc<mesh::Mesh>>,
    pub texture: Option<Rc<texture::Texture>>,
    pub config: BodyConfig,
    pub shape: Rc<BodyShape>, // NOTE: holds memory of TriMesh!
    pub ode_body: ode::dBodyID,
    pub ode_geom: ode::dGeomID,
    pub id: u64,
}

impl std::cmp::PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
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

    pub fn set_finite_rotation_mode(&mut self, enabled: bool) {
        // true: allegedly stabler fast speed rotation
        unsafe {
            ode::dBodySetFiniteRotationMode(self.ode_body, enabled as i32);
        }
    }
}
