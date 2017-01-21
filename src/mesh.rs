use glium;
use glium::backend::Facade;
use glium::Surface;
use glium::uniforms::Uniforms;

use obj;

use std::path::Path;

use math::*;
use errors::*;

#[derive(Clone, Copy)]
pub struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coord: [f32; 3],
}
implement_vertex!(Vertex, position, normal, tex_coord);

pub struct Mesh {
    buffer: glium::VertexBuffer<Vertex>,
    orig_buffer: Option<Vec<Vertex>>,
}

impl Mesh {
    pub fn new<F: Facade>(f: &F,
                          positions: Vec<Pnt3>,
                          normals: Vec<Vec3>,
                          texture_coordinates: Vec<Pnt3>,
                          retain: bool)
                          -> Result<Mesh> {
        let mut vs = Vec::with_capacity(positions.len());
        for ((p, n), t) in positions.into_iter()
            .zip(normals.into_iter())
            .zip(texture_coordinates.into_iter()) {
            let v = Vertex {
                position: *p.as_ref(),
                normal: *n.as_ref(),
                tex_coord: *t.as_ref(),
            };
            vs.push(v);
        }

        let orig_buffer = if retain { Some(vs.clone()) } else { None };

        Ok(Mesh {
            buffer: glium::VertexBuffer::new(f, &vs).chain_err(|| "unable to create buffer")?,
            orig_buffer: orig_buffer,
        })
    }

    pub fn from_obj<F: Facade, P: AsRef<Path> + ?Sized>(f: &F,
                                                        path: &P,
                                                        retain: bool)
                                                        -> Result<Mesh> {
        let (positions, normals, texcoord) =
            obj::load_obj(path).chain_err(|| "unable to load .obj")?;

        Mesh::new(f, positions, normals, texcoord, retain)
    }

    pub fn for_cubemap<F: Facade>(f: &F) -> Result<Mesh> {
        let scale = 10.0;
        let fr = -scale;
        let bk = scale;
        let l = -scale;
        let r = scale;
        let d = -scale;
        let u = scale;

        let positions = vec![// -X
                             Pnt3::new(l, u, bk),
                             Pnt3::new(l, d, bk),
                             Pnt3::new(l, u, fr),
                             Pnt3::new(l, u, fr),
                             Pnt3::new(l, d, bk),
                             Pnt3::new(l, d, fr),

                             // +X
                             Pnt3::new(r, u, fr),
                             Pnt3::new(r, d, fr),
                             Pnt3::new(r, u, bk),
                             Pnt3::new(r, u, bk),
                             Pnt3::new(r, d, fr),
                             Pnt3::new(r, d, bk),

                             // -Z
                             Pnt3::new(l, u, fr),
                             Pnt3::new(l, d, fr),
                             Pnt3::new(r, u, fr),
                             Pnt3::new(r, u, fr),
                             Pnt3::new(l, d, fr),
                             Pnt3::new(r, d, fr),

                             // +Z
                             Pnt3::new(r, u, bk),
                             Pnt3::new(r, d, bk),
                             Pnt3::new(l, u, bk),
                             Pnt3::new(l, u, bk),
                             Pnt3::new(r, d, bk),
                             Pnt3::new(l, d, bk),

                             // -Y
                             Pnt3::new(l, d, fr),
                             Pnt3::new(l, d, bk),
                             Pnt3::new(r, d, fr),
                             Pnt3::new(r, d, fr),
                             Pnt3::new(l, d, bk),
                             Pnt3::new(r, d, bk),

                             // +Y
                             Pnt3::new(l, u, bk),
                             Pnt3::new(l, u, fr),
                             Pnt3::new(r, u, bk),
                             Pnt3::new(r, u, bk),
                             Pnt3::new(l, u, fr),
                             Pnt3::new(r, u, fr)];

        let normals = vec![Vec3::new(r, 0.0, 0.0),
                           Vec3::new(r, 0.0, 0.0),
                           Vec3::new(r, 0.0, 0.0),
                           Vec3::new(r, 0.0, 0.0),
                           Vec3::new(r, 0.0, 0.0),
                           Vec3::new(r, 0.0, 0.0),

                           Vec3::new(l, 0.0, 0.0),
                           Vec3::new(l, 0.0, 0.0),
                           Vec3::new(l, 0.0, 0.0),
                           Vec3::new(l, 0.0, 0.0),
                           Vec3::new(l, 0.0, 0.0),
                           Vec3::new(l, 0.0, 0.0),

                           Vec3::new(0.0, 0.0, bk),
                           Vec3::new(0.0, 0.0, bk),
                           Vec3::new(0.0, 0.0, bk),
                           Vec3::new(0.0, 0.0, bk),
                           Vec3::new(0.0, 0.0, bk),
                           Vec3::new(0.0, 0.0, bk),

                           Vec3::new(0.0, 0.0, fr),
                           Vec3::new(0.0, 0.0, fr),
                           Vec3::new(0.0, 0.0, fr),
                           Vec3::new(0.0, 0.0, fr),
                           Vec3::new(0.0, 0.0, fr),
                           Vec3::new(0.0, 0.0, fr),

                           Vec3::new(0.0, u, 0.0),
                           Vec3::new(0.0, u, 0.0),
                           Vec3::new(0.0, u, 0.0),
                           Vec3::new(0.0, u, 0.0),
                           Vec3::new(0.0, u, 0.0),
                           Vec3::new(0.0, u, 0.0),

                           Vec3::new(0.0, d, 0.0),
                           Vec3::new(0.0, d, 0.0),
                           Vec3::new(0.0, d, 0.0),
                           Vec3::new(0.0, d, 0.0),
                           Vec3::new(0.0, d, 0.0),
                           Vec3::new(0.0, d, 0.0)];

        let r = 1.0 / 3.0;
        let c = 1.0 / 4.0;
        // origins of left bottom corner
        // arranged such that the triangle order above works
        let uv_l = Pnt3::new(0.0 * c, 1.0 * r, 0.0); // left
        let uv_r = Pnt3::new(2.0 * c, 1.0 * r, 0.0); // right
        let uv_f = Pnt3::new(1.0 * c, 1.0 * r, 0.0); // front
        let uv_b = Pnt3::new(3.0 * c, 1.0 * r, 0.0); // back
        let uv_d = Pnt3::new(1.0 * c, 0.0 * r, 0.0); // down
        let uv_u = Pnt3::new(1.0 * c, 2.0 * r, 0.0); // up

        let uv_0 = Vec3::new(0.0, r, 0.0);
        let uv_1 = Vec3::new(0.0, 0.0, 0.0);
        let uv_2 = Vec3::new(c, r, 0.0);
        let uv_3 = Vec3::new(c, 0.0, 0.0);

        let mut uvs = vec![];
        for base in &[uv_l, uv_r, uv_f, uv_b, uv_d, uv_u] {
            uvs.push(*base + uv_0);
            uvs.push(*base + uv_1);
            uvs.push(*base + uv_2);
            uvs.push(*base + uv_2);
            uvs.push(*base + uv_1);
            uvs.push(*base + uv_3);
        }

        let retain = false;
        Ok(Mesh::new(f, positions, normals, uvs, retain)?)
    }

    pub fn draw<S: Surface, U: Uniforms>(&self,
                                         surface: &mut S,
                                         uniforms: &U,
                                         program: &glium::Program,
                                         depth_test: bool,
                                         alpha_dual_render: bool)
                                         -> Result<()> {
        use glium::draw_parameters::{DepthTest, BackfaceCullingMode};

        let mut params: glium::draw_parameters::DrawParameters = Default::default();
        if depth_test {
            params.depth = glium::Depth {
                test: DepthTest::IfLess,
                write: true,
                ..Default::default()
            };
        }
        if alpha_dual_render {
            params.blend = glium::Blend::alpha_blending();
            params.backface_culling = BackfaceCullingMode::CullCounterClockwise;
        } else {
            params.backface_culling = BackfaceCullingMode::CullClockwise;
        }

        surface.draw(&self.buffer,
                  &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                  program,
                  uniforms,
                  &params)
            .chain_err(|| "drawcall failed")?;

        if alpha_dual_render {
            params.backface_culling = BackfaceCullingMode::CullClockwise;

            surface.draw(&self.buffer,
                      &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
                      program,
                      uniforms,
                      &params)
                .chain_err(|| "drawcall failed")?;
        }

        Ok(())
    }

    pub fn update_mesh<T: FnOnce(&mut Vec<Vertex>)>(&mut self, func: T) {
        if self.orig_buffer.is_none() {
            self.orig_buffer = Some(Vec::new());
        }
        func(&mut self.orig_buffer.as_mut().unwrap());
        for (buf_vert, orig_vert) in self.buffer
            .map()
            .iter_mut()
            .zip(self.orig_buffer.as_mut().unwrap().iter()) {
            *buf_vert = *orig_vert;
        }
    }
}
