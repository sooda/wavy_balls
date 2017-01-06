#![feature(conservative_impl_trait)]

#[macro_use]
extern crate glium;
extern crate glium_sdl2;
extern crate sdl2;

#[macro_use]
extern crate error_chain;

extern crate image;

extern crate nalgebra as na;
extern crate ncollide as nc;

extern crate inotify;

mod math;
mod body;
mod world;
mod mesh;
mod obj;
mod texture;

mod errors {
    error_chain! {
        errors {
            ObjLoadError
        }
    }
}
use errors::*;

use std::fs::File;
use std::io::Read;

use glium::Surface;
use na::{Transformation, ToHomogeneous, Transform, Translation, Norm};
use math::*;
use std::rc::Rc;

use inotify::INotify;
use inotify::ffi::*;
use std::path::Path;

static VERTEX_SHADER: &'static str = r#"
    #version 140

    uniform mat4 perspective;
    uniform mat4 modelview;

    in vec3 position;
    in vec3 normal;
    in vec2 tex_coord;

    out vec2 f_tex_coord;

    void main() {
        gl_Position = perspective * modelview * vec4(position, 1.0);

        f_tex_coord = tex_coord;
    }
"#;

static FRAGMENT_SHADER: &'static str = r#"
    #version 140

    in vec2 f_tex_coord;

    uniform sampler2D tex;

    void main() {
        gl_FragColor = texture(tex, f_tex_coord);
    }
"#;

#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 2],
}
implement_vertex!(Vertex, position);

struct SampleModel {
    buffer: glium::VertexBuffer<Vertex>,
    program: glium::Program,
}

fn render<S: glium::Surface>(surface: &mut S, state: &SampleModel, time: f32) {
    let params = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::draw_parameters::DepthTest::IfLess,
            write: true,
            ..Default::default()
        },
        ..Default::default()
    };

    surface.draw(&state.buffer,
              &glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList),
              &state.program,
              &uniform! {
                     time: time,
                 },
              &params)
        .unwrap();
}

fn read_file(name: &str) -> Result<String> {
    let mut f = File::open(name).chain_err(|| "failed to open file")?;
    let mut s = String::new();
    f.read_to_string(&mut s).chain_err(|| "failed to read file")?;

    Ok(s)
}

fn load_shader_prog<F: glium::backend::Facade>(facade: &F, name: &str) -> Result<glium::Program> {
    let vert = read_file(&("src/".to_owned() + name + ".vert")).chain_err(|| "no vert shader")?;
    let frag = read_file(&("src/".to_owned() + name + ".frag")).chain_err(|| "no frag shader")?;

    glium::Program::from_source(facade, &vert, &frag, None).chain_err(|| "shader does not compile")
}

fn main() {
    use glium_sdl2::DisplayBuild;

    let sdl_ctx = sdl2::init().unwrap();
    let sdl_video = sdl_ctx.video().unwrap();
    let sdl_glattr = sdl_video.gl_attr();
    sdl_glattr.set_context_profile(sdl2::video::GLProfile::Core);
    sdl_glattr.set_context_version(3, 3);

    let display_width = 800;
    let display_height = 600;
    let display = sdl_video.window("FGJ", display_width, display_height).build_glium().unwrap();

    let mut event_pump = sdl_ctx.event_pump().unwrap();
    let mut sdl_timer = sdl_ctx.timer().unwrap();

    let projection = na::Perspective3::new(display_width as f32 / display_height as f32,
                                           3.1416 / 2.0,
                                           0.01,
                                           50.0f32)
        .to_matrix();

    let mut mesh = vec![];
    mesh.push(Vertex { position: [0.0f32, 0.0f32] });
    mesh.push(Vertex { position: [0.0f32, 1.0f32] });
    mesh.push(Vertex { position: [1.0f32, 1.0f32] });

    let buffer = glium::VertexBuffer::new(&display, &mesh).unwrap();
    let mut state = SampleModel {
        buffer: buffer,
        program: load_shader_prog(&display, "test").unwrap(),
    };

    let mut last_t = sdl_timer.ticks();

    let body = body::Body::new(Rc::new(mesh::Mesh::from_obj(&display, "ballo.obj").unwrap()),
                               body::BodyShape::Sphere { radius: 1.0 });
    let mut world = world::World::new();
    world.add_body(body);

    let texture = texture::load_texture(&display, "eh.png").unwrap();

    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();

    let mut ino = INotify::init().unwrap();

    ino.add_watch(Path::new("src"), IN_MODIFY | IN_CREATE | IN_DELETE).unwrap();

    'mainloop: loop {
        let evs = ino.available_events().unwrap();

        if evs.len() > 0 {
            match load_shader_prog(&display, "test") {
                Ok(prog) => state.program = prog,
                Err(bad) => {
                    println!("sorry: {}", bad);
                    for e in bad.iter().skip(1) {
                        println!("because: {}", e);
                    }
                }
            }
        }

        for ev in event_pump.poll_iter() {
            use sdl2::event::Event;
            use sdl2::keyboard::Keycode;

            match ev {
                Event::Quit { .. } => break 'mainloop,
                Event::KeyDown { keycode, .. } => {
                    match keycode {
                        Some(Keycode::Return) => {
                            match load_shader_prog(&display, "test") {
                                Ok(prog) => state.program = prog,
                                Err(bad) => {
                                    println!("sorry: {}", bad);
                                    for e in bad.iter().skip(1) {
                                        println!("because: {}", e);
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        struct Input {
            left: bool,
            right: bool,
            up: bool,
            down: bool,
        }
        let input = {
            use sdl2::keyboard::Scancode::*;

            let kb = event_pump.keyboard_state();

            Input {
                left: kb.is_scancode_pressed(Left),
                right: kb.is_scancode_pressed(Right),
                up: kb.is_scancode_pressed(Up),
                down: kb.is_scancode_pressed(Down),
            }
        };

        let mut force_x = 0.0;
        let mut force_z = 0.0;
        if input.left {
            force_x -= 1.0;
        }
        if input.right {
            force_x += 1.0;
        }
        if input.up {
            force_z -= 1.0;
        }
        if input.down {
            force_z += 1.0;
        }

        world.bodies_mut()[0].force = Vec3::new(force_x, 0.0, force_z);

        let dt = (sdl_timer.ticks() - last_t) as f32 / 1000.0;
        last_t = sdl_timer.ticks();

        // Step the world
        world.step(dt);

        let mut target = display.draw();

        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        for body in world.bodies() {
            let modelview = Iso3::new(body.position, Vec3::new(0.0, 0.0, 0.0)).to_homogeneous();

            body.mesh
                .draw(&mut target,
                      &uniform! {
                      perspective: *projection.as_ref(),
                      modelview: *modelview.as_ref(),
                      tex: &texture,
                  },
                      &program)
                .unwrap();
        }

        render(&mut target, &state, sdl_timer.ticks() as f32 / 1000.0);

        target.finish().unwrap();

        std::thread::sleep_ms(1);
    }
}
