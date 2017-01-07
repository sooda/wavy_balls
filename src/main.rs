#[macro_use]
extern crate glium;
extern crate glium_sdl2;
extern crate sdl2;

#[macro_use]
extern crate error_chain;

extern crate image;

extern crate nalgebra as na;
extern crate ncollide as nc;
extern crate nphysics3d as np;

extern crate inotify;

extern crate rand;

mod math;
mod body;
mod world;
mod mesh;
mod obj;
mod texture;
mod particle;

mod errors {
    error_chain! {
        errors {
            SdlError(t: String) {
                description("sdl error")
                display("sdl error: {}", t)
            }
            ObjLoadError
        }
    }
}
use errors::*;

use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use std::time;
use std::path::Path;

use na::{Transformation, ToHomogeneous, Transform, Translation, Norm, Rotation3};
use glium::Surface;
use inotify::INotify;
use inotify::ffi::*;

use rand::Rng;

use math::*;

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

fn sdl_err(r: String) -> Error {
    Error::from_kind(ErrorKind::SdlError(r))
}

fn run() -> Result<()> {
    use glium_sdl2::DisplayBuild;

    let sdl_ctx = sdl2::init().map_err(sdl_err).chain_err(|| "failed to initialize SDL")?;
    let sdl_video = sdl_ctx.video().map_err(sdl_err).chain_err(|| "failed to initialize video")?;
    let sdl_audio = sdl_ctx.audio().map_err(sdl_err).chain_err(|| "failed to initialize audio")?;
    let sdl_glattr = sdl_video.gl_attr();
    sdl_glattr.set_context_profile(sdl2::video::GLProfile::Core);
    sdl_glattr.set_context_version(3, 3);

    let display_width = 800;
    let display_height = 600;
    let display = sdl_video.window("FGJ", display_width, display_height)
        .build_glium()
        .chain_err(|| "failed to initialize glium context")?;

    let mut event_pump =
        sdl_ctx.event_pump().map_err(sdl_err).chain_err(|| "failed to initialize SDL event pump")?;
    let mut sdl_timer =
        sdl_ctx.timer().map_err(sdl_err).chain_err(|| "failed to initialize SDL timer")?;

    let sdl_mixer = sdl2::mixer::init(sdl2::mixer::INIT_OGG).map_err(sdl_err)
        .chain_err(|| "failed to initialize SDL mixer")?;
    sdl2::mixer::open_audio(44100, sdl2::mixer::AUDIO_S16LSB, 2, 2048).map_err(sdl_err)
        .chain_err(|| "failed to open SDL audio")?;
    sdl2::mixer::allocate_channels(16);

    let projection = na::Perspective3::new(display_width as f32 / display_height as f32,
                                           PI / 2.0,
                                           0.01,
                                           500.0f32)
        .to_matrix();

    let mut mesh = vec![];
    mesh.push(Vertex { position: [0.0f32, 0.9f32] });
    mesh.push(Vertex { position: [0.0f32, 1.0f32] });
    mesh.push(Vertex { position: [1.0f32, 1.0f32] });

    let buffer = glium::VertexBuffer::new(&display, &mesh).chain_err(|| "failed to allocate GPU vertex buffer")?;
    let mut state = SampleModel {
        buffer: buffer,
        program: load_shader_prog(&display, "test").chain_err(|| "failed to load shader")?,
    };

    let mut last_t = sdl_timer.ticks();
    let mut world = world::World::new();

    let player = world.add_body(Rc::new(mesh::Mesh::from_obj(&display, "ballo.obj").chain_err(|| "failed to load ball mesh")?), 
                   body::BodyShape::Sphere{radius: 1.0}, false);
    player.borrow_mut().set_translation(Vec3::new(0.0, 3.0, 0.0));
    player.borrow_mut().set_deactivation_threshold(None); // prevent deactivation

    let plane = world.add_body(Rc::new(mesh::Mesh::from_obj(&display, "plane.obj").chain_err(|| "failed to load plane mesh")?),
                                body::BodyShape::from_obj("plane.obj").unwrap(), true);
    for i in 0..10 {
        let ball = world.add_body(Rc::new(mesh::Mesh::from_obj(&display, "ballo.obj").chain_err(|| "failed to load ball mesh")?),
     body::BodyShape::Sphere{radius: 1.0}, false);
        ball.borrow_mut().set_translation(Vec3::new(3.0, 3.0 + 3.0 * (i as f32), 0.0));
    }

    let texture =
        texture::load_texture(&display, "eh.png").chain_err(|| "failed to load ball texture")?;
    let envmap = texture::load_texture(&display, "cubemap.jpg").chain_err(|| "failed to load environment map")?;

    let cube = mesh::Mesh::for_cubemap(&display).unwrap();

    let mut particles = particle::Particles::new(
        &display, vec![texture::load_image("starAlpha.png")?], 100)
                    .chain_err(|| "failed to initialize particle engine")?;

    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();

    let mut ino = INotify::init().chain_err(|| "failed to initialize inotify")?;

    ino.add_watch(Path::new("src"), IN_MODIFY | IN_CREATE | IN_DELETE)
        .chain_err(|| "failed to add inotify watch")?;

    let music = sdl2::mixer::Music::from_file(Path::new("foldplop_-_memory_song_part_2.ogg"))
        .map_err(sdl_err).chain_err(|| "failed to load background music")?;
    music.play(-1).map_err(sdl_err).chain_err(|| "failed to play background music")?;

    let jump_sound =
        sdl2::mixer::Chunk::from_file(Path::new("146718__fins__button.wav")).map_err(sdl_err)
            .chain_err(|| "failed to load jump sound")?;

    let mut allow_jump = true;

    let mut last_particle = 0.0;

    struct CameraAngles {
        yaw: f32, // no restrictions for this
        pitch: f32, // only between ground and ceiling, can't snap your neck
    }

    let mut camera = CameraAngles {
        yaw: 0.0,
        pitch: 0.0,
    };

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

        let dt = (sdl_timer.ticks() - last_t) as f32 / 1000.0;
        last_t = sdl_timer.ticks();
        let curr_t = last_t as f32 / 1000.0;

        let mut force_x = 0.0;
        let mut force_y = 0.0;
        let mut force_z = 0.0;

        let force_mag = 1.0;

        for ev in event_pump.poll_iter() {
            use sdl2::event::Event;
            use sdl2::keyboard::Keycode;

            match ev {
                Event::Quit { .. } => break 'mainloop,
                Event::KeyDown { keycode, .. } => {
                    match keycode {
                        Some(Keycode::Escape) => break 'mainloop,
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
                        Some(Keycode::Space) if allow_jump => {
                            force_y = 2.0 * GRAVITY * force_mag;
                            sdl2::mixer::Channel::all().play(&jump_sound, 0)
                                .map_err(sdl_err)
                                .chain_err(|| "failed to play jump sound")?;
                            allow_jump = false;
                        }
                        Some(Keycode::R) => {
                            camera.yaw = 0.0;
                            camera.pitch = 0.0;
                        }
                        Some(Keycode::S) => player.borrow_mut().set_lin_vel(na::zero()),
                        _ => (),
                    }
                }
                Event::KeyUp { keycode, .. } => {
                    match keycode {
                        Some(Keycode::Space) => allow_jump = true,
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        struct MouseDelta {
            x: i32,
            y: i32,
        }

        let mouse = {
            let m = event_pump.relative_mouse_state();
            if m.left() {
                MouseDelta {
                    x: m.x(),
                    y: m.y(),
                }
            } else {
                MouseDelta { x: 0, y: 0 }
            }
        };

        camera.yaw += mouse.x as f32 / 100.0;
        camera.pitch += mouse.y as f32 / 100.0;
        camera.pitch = na::clamp(camera.pitch, -PI / 2.0, PI / 2.0);

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



        if curr_t - last_particle > 0.15 {
            last_particle = curr_t;

            particles.add(particle::Particle {
                position: Pnt3::new(0.0, 0.0, 0.0), // global world coordinate
                scale: Vec2::new(0.4, 0.4),
                velocity: Vec3::new(rand::random::<f32>() * 0.5,
                                    2.5,
                                    rand::random::<f32>() * 0.5),
                color: Vec4::new(0.0, 0.0, 1.0, 0.8),
                lifetime: Some(5.0),
                alive: 0.0,
                texture: 0,
            });
        }

        // Step the world
        world.step(dt);
        particles.step(dt);

        let mut target = display.draw();

        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        let camera_rot = Rotation3::new(Vec3::new(camera.pitch, 0.0, 0.0)) *
                         Rotation3::new(Vec3::new(0.0, camera.yaw, 0.0));
        let camera_pos = player.borrow_mut().position().translation +
                         Vec3::new(0.0, 3.0, 5.0) * camera_rot;


        if input.left {
            force_x -= force_mag;
        }
        if input.right {
            force_x += force_mag;
        }
        if input.up {
            force_z -= force_mag;
        }
        if input.down {
            force_z += force_mag;
        }

        player.borrow_mut()
            .apply_central_impulse(Vec3::new(force_x, force_y, force_z) * camera_rot);

        // iso is rotation followed by translation, can't use it directly just like that
        let cam_rotate = Iso3::from_rotation_matrix(na::zero(), camera_rot).to_homogeneous();
        let cam_translate = Iso3::new(-camera_pos, na::zero()).to_homogeneous();
        let cam_view = cam_rotate * cam_translate;

        cube.draw(&mut target,
                  &uniform! {
                perspective: *projection.as_ref(),
                modelview: *cam_rotate.as_ref(),
                tex: &envmap
        },
                  &program,
                  false)
            .chain_err(|| "failed to draw cubemap")?;

        for body in world.rigid_bodies() {
            let body = body.borrow_mut();
            let model = body.position().to_homogeneous(); //Iso3::new(body.position().translation, body.position().rotation)
           //     .to_homogeneous();
            let modelview = cam_view * model;

            let body = body.user_data().unwrap().downcast_ref::<body::Body>().unwrap();
            body.mesh
                .draw(&mut target,
                      &uniform! {
                      perspective: *projection.as_ref(),
                      modelview: *modelview.as_ref(),
                      tex: &texture,
                  },
                      &program,
                      true)
                .chain_err(|| "failed to draw mesh")?;
        }

        particles.draw(&mut target, *projection.as_ref(), *cam_view.as_ref())
            .chain_err(|| "failed to render particles")?;

        render(&mut target, &state, sdl_timer.ticks() as f32 / 1000.0);

        target.finish().chain_err(|| "failed to finish frame")?;

        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    Ok(())
}

fn main() {
    if let Err(ref e) = run() {
        println!("Error: {}", e);
        for cause in e.iter().skip(1) {
            println!(".. because: {}", cause);
        }
    }
}
