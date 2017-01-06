use glium;
use glium::backend::Facade;
use glium::Surface;
use glium::uniforms::Uniforms;

use obj;

use std::path::Path;

use math::*;
use errors::*;

#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}
implement_vertex!(Vertex, position, normal);

pub struct Mesh {
    buffer: glium::VertexBuffer<Vertex>,
}

impl Mesh {
    pub fn new<F: Facade>(f: &F, positions: Vec<Pnt3>, normals: Vec<Vec3>) -> Result<Mesh> {
        let mut vs = Vec::with_capacity(positions.len());
        for (p, n) in positions.into_iter().zip(normals.into_iter()) {
            let v = Vertex {
                position: *p.as_ref(),
                normal: *n.as_ref(),
            };
            vs.push(v);
        }

        Ok(Mesh {
            buffer: glium::VertexBuffer::new(f, &vs).chain_err(|| "unable to create buffer")?,
        })
    }

    pub fn from_obj<F: Facade, P: AsRef<Path> + ?Sized>(f: &F, path: &P) -> Result<Mesh> {
        let (positions, normals) = obj::load_obj(path).chain_err(|| "unable to load .obj")?;

        Mesh::new(f, positions, normals)
    }

    pub fn draw<S: Surface, U: Uniforms>(&self,
                                         surface: &mut S,
                                         uniforms: &U,
                                         program: &glium::Program)
                                         -> Result<()> {
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::draw_parameters::DepthTest::IfLess,
                write: true,
                ..Default::default()
            },
            ..Default::default()
        };

        surface.draw(&self.buffer,
                  &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                  program,
                  uniforms,
                  &params)
            .chain_err(|| "drawcall failed")
    }
}