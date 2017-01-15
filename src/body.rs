use math;
use obj;
use mesh;
use na;
use nc;
use np;
use glium;
use errors::*;
use std::rc::Rc;
use std::sync::Arc;
use std::path::Path;

pub enum BodyShape {
    Sphere { radius: f32 },
    TriangleSoup(nc::shape::TriMesh<math::Pnt3>),
}

impl BodyShape {
    pub fn from_obj<P: AsRef<Path> + ?Sized>(path: &P) -> Result<BodyShape> {
        let (positions, _normals, _texcoord) =
            obj::load_obj(path).chain_err(|| "unable to load .obj")?;

        let indices = (0usize..positions.len() / 3)
            .map(|i| na::Point3::<usize>::new(i * 3, i * 3 + 1, i * 3 + 2))
            .collect();

        let trimesh = nc::shape::TriMesh::new(Arc::new(positions), Arc::new(indices), None, None);
        Ok(BodyShape::TriangleSoup(trimesh))
    }
}

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
    pub texture: Rc<glium::texture::Texture2d>,
    pub config: BodyConfig,
}

impl Body {}
