use glium;
use na;
use errors::*;

use math::*;

#[derive(Clone, Copy, Default)]
struct Vertex {
    position: [f32; 3],
    scale: [f32; 2],
    color: [f32; 4],
    tex: f32,
}
implement_vertex!(Vertex, position, scale, color, tex);

static PARTICLE_V: &'static str = r#"
    #version 140

    in vec3 position;
    in vec2 scale;
    in vec4 color;
    in float tex;

    out vec2 gScale;
    out float gTex;
    out vec4 gColor;

    void main() {
        gl_Position = vec4(position, 1.0);
        gScale = scale;
        gTex = tex;
        gColor = color;
    }
"#;

static PARTICLE_F: &'static str = r#"
    #version 140

    uniform sampler2DArray sTextures;

    in vec3 fTex;
    in vec3 fPosition;
    in vec4 fColor;

    out vec4 color;

    void main() {
        color = fColor * texture(sTextures, fTex);
    }
"#;

static PARTICLE_G: &'static str = r#"
    #version 150

    layout(points) in;
    layout(triangle_strip, max_vertices = 4) out;

    uniform mat4 mPerspective;
    uniform mat4 mModelview;

    in vec2 gScale[];
    in float gTex[];
    in vec4 gColor[];

    out vec3 fPosition;
    out vec3 fTex;
    out vec4 fColor;

    void main() {
        vec4 Fr = vec4(mModelview[0][0], mModelview[1][0], mModelview[2][0], 0.0);
        vec4 Ta = vec4(mModelview[0][1], mModelview[1][1], mModelview[2][1], 0.0);

        float xs = gScale[0].x;
        float zs = gScale[0].y;
        float tex = gTex[0];

        fColor = gColor[0];

        gl_Position = gl_in[0].gl_Position - xs * Ta - zs * Fr;
        fPosition = gl_Position.xyz;
        gl_Position = mPerspective * mModelview * gl_Position;
        fTex = vec3(0.0, 0.0, tex);
        EmitVertex();

        gl_Position = gl_in[0].gl_Position + xs * Ta - zs * Fr;
        fPosition = gl_Position.xyz;
        gl_Position = mPerspective * mModelview * gl_Position;
        fTex = vec3(1.0, 0.0, tex);
        EmitVertex();

        gl_Position = gl_in[0].gl_Position - xs * Ta + zs * Fr;
        fPosition = gl_Position.xyz;
        gl_Position = mPerspective * mModelview * gl_Position;
        fTex = vec3(0.0, 1.0, tex);
        EmitVertex();

        gl_Position = gl_in[0].gl_Position + xs * Ta + zs * Fr;
        fPosition = gl_Position.xyz;
        gl_Position = mPerspective * mModelview * gl_Position;
        fTex = vec3(1.0, 1.0, tex);
        EmitVertex();

        EndPrimitive();
    }
"#;

pub struct Particle {
    pub position: Pnt3,
    pub scale: Vec2,
    pub velocity: Vec3,
    pub color: Vec4,
    pub lifetime: Option<f32>,
    pub alive: f32,
    pub texture: u32,
}

impl Default for Particle {
    fn default() -> Particle {
        Particle {
            position: Pnt3::new(0.0, 0.0, 0.0),
            scale: Vec2::new(0.5, 0.5),
            velocity: na::zero(),
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            lifetime: None,
            alive: 0.0,
            texture: 0,
        }
    }
}

pub struct Particles {
    buffer: glium::VertexBuffer<Vertex>,
    particles: Vec<Particle>,
    program: glium::Program,
    textures: glium::texture::Texture2dArray,
}

impl Particles {
    pub fn new<'t, F, T>(facade: &F, textures: Vec<T>, max_particles: usize) -> Result<Particles>
        where F: glium::backend::Facade,
              T: glium::texture::Texture2dDataSource<'t>
    {
        let particles = vec![Default::default(); max_particles];
        let buf = glium::VertexBuffer::persistent(facade, &particles)
                                      .chain_err(|| "failed to allocate gpu buffer")?;

        Ok(Particles {
            buffer: buf,
            particles: vec![],
            program: glium::Program::from_source(facade, PARTICLE_V, PARTICLE_F, Some(PARTICLE_G))
                                    .chain_err(|| "failed to load particle shader")?,
            textures: glium::texture::Texture2dArray::new(facade, textures)
                                    .chain_err(|| "failed to create particle texture array")?,
        })
    }

    pub fn add(&mut self, particle: Particle) {
        self.particles.push(particle);
    }

    pub fn draw<S>(&self,
                   surface: &mut S,
                   perspective: [[f32; 4]; 4],
                   modelview: [[f32; 4]; 4])
                   -> Result<()>
        where S: glium::Surface
    {
        let params = glium::DrawParameters {
            depth: glium::Depth {
                test: glium::draw_parameters::DepthTest::IfLess,
                write: false,
                ..Default::default()
            },
            blend: glium::Blend::alpha_blending(),
            ..Default::default()
        };

        surface.draw(&self.buffer,
                  &glium::index::NoIndices(glium::index::PrimitiveType::Points),
                  &self.program,
                  &uniform! {
                         mPerspective: perspective,
                         mModelview: modelview,
                         sTextures: &self.textures,
                     },
                  &params)
            .chain_err(|| "drawcall failed")
    }

    pub fn step(&mut self, dt: f32) {
        for particle in &mut self.particles {
            particle.position = particle.position + particle.velocity * dt;
            particle.alive += dt;
        }

        self.particles.retain(|particle| {
            if let Some(lifetime) = particle.lifetime {
                particle.alive <= lifetime
            } else {
                true
            }
        });

        let mut m = self.buffer.map();
        for (idx, bp) in m.iter_mut().enumerate() {
            if let Some(p) = self.particles.get(idx) {
                bp.position = [p.position.x, p.position.y, p.position.z];
                bp.scale = [p.scale.x, p.scale.y];
                bp.color = [p.color.x, p.color.y, p.color.z, p.color.w];
                bp.tex = p.texture as f32;
            } else {
                // Hide old particle
                bp.position = [0.0, -1000.0, 0.0];
            }
        }
    }
}
