use math;
use obj;
use mesh;
use na;
use nc;
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
        let (positions, normals, texcoord) =
            obj::load_obj(path).chain_err(|| "unable to load .obj")?;

        let indices = (0usize..positions.len() / 3)
            .map(|i| na::Point3::<usize>::new(i * 3, i * 3 + 1, i * 3 + 2))
            .collect();

        let trimesh = nc::shape::TriMesh::new(Arc::new(positions), Arc::new(indices), None, None);
        Ok(BodyShape::TriangleSoup(trimesh))
    }
}

pub struct Body {
    pub position: math::Vec3,
    pub velocity: math::Vec3,
    pub force: math::Vec3,
    pub mesh: Rc<mesh::Mesh>,
    pub shape: BodyShape,
    pub fixed: bool,
}

impl Body {
    pub fn new(mesh: Rc<mesh::Mesh>, shape: BodyShape, fixed: bool) -> Body {
        Body {
            position: math::Vec3::new(0.0, 0.0, 0.0),
            velocity: math::Vec3::new(0.0, 0.0, 0.0),
            force: math::Vec3::new(0.0, 0.0, 0.0),
            mesh: mesh,
            shape: shape,
            fixed: fixed
        }
    }
}
