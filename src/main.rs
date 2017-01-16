#![deny(warnings)]

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

#[link(name = "ode")]
extern "C" {}

mod audio;
mod math;
mod body;
mod world;
mod mesh;
mod obj;
mod texture;
mod particle;
mod input;

mod ode;

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
use std::path::Path;

use na::{ToHomogeneous, Rotation3};
use glium::Surface;
use inotify::INotify;
use inotify::ffi::*;

use math::*;
use audio::{AudioMixer, JumpSound};

static VERTEX_SHADER: &'static str = r#"
    #version 140

    uniform mat4 perspective;
    uniform mat4 modelview;

    in vec3 position;
    in vec3 normal;
    in vec3 tex_coord;

    out vec3 f_tex_coord;
    out vec3 f_position;

    void main() {
        gl_Position = perspective * modelview * vec4(position, 1.0);

        f_tex_coord = tex_coord;
        f_position = position;
    }
"#;

static FRAGMENT_SHADER: &'static str = r#"
    #version 140

    in vec3 f_tex_coord;
    in vec3 f_position;

    uniform sampler2D tex;
    uniform vec3 player_pos;

    void main() {
        vec4 color = texture(tex, f_tex_coord.xy);
        if (f_position.y < player_pos.y && length(player_pos.xz - f_position.xz) <= 1.0)
            color.rgb = color.rgb * 0.4;
        gl_FragColor = color;
    }
"#;

static FRAGMENT_SHADER_ARRAY: &'static str = r#"
    #version 140

    in vec3 f_tex_coord;
    in vec3 f_position;

    uniform sampler2DArray tex;
    uniform vec3 player_pos;

    void main() {
        vec4 color = texture(tex, f_tex_coord.xyz);
        if (f_position.y < player_pos.y && length(player_pos.xz - f_position.xz) <= 1.0)
            color.rgb = color.rgb * 0.4;
        gl_FragColor = color;
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

    unsafe {
        ode::dInitODE();
    }

    let sdl_ctx = sdl2::init().map_err(sdl_err).chain_err(|| "failed to initialize SDL")?;
    let sdl_video = sdl_ctx.video().map_err(sdl_err).chain_err(|| "failed to initialize video")?;
    let _sdl_audio = sdl_ctx.audio().map_err(sdl_err).chain_err(|| "failed to initialize audio")?;
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

    let sdl_gcon = sdl_ctx.game_controller().unwrap();
    let num_gcons = sdl_gcon.num_joysticks().unwrap();
    let mut sel_gcon = None;
    if num_gcons > 0 {
        let mut buffer = String::new();
        File::open("gamecontrollerdb.txt").chain_err(|| "failed to open gamecontrollerdb.txt")?
            .read_to_string(&mut buffer)
            .chain_err(|| "failed to read gamecontrollerdb.txt")?;
        for line in buffer.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            sdl_gcon.add_mapping(line).chain_err(|| "cannot add mapping")?;
        }

        println!("{} game controllers detected.", num_gcons);

        for id in 0..num_gcons {
            let gcon = sdl_gcon.open(id).unwrap();
            let gcon_name = if gcon.name().is_empty() {
                "unknown".to_string()
            } else {
                gcon.name()
            };
            println!("Found game controller {}: {}", id, gcon_name);
            if sel_gcon.is_none() {
                println!("  Setting as active");
                sel_gcon = Some(gcon);
            }

            if !sdl_gcon.is_game_controller(id) {
                println!("Warning: Unknown controller model");
            }
        }
    }

    let mut input_state = input::InputState::new(sel_gcon);

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

    let eh_texture = Rc::new(texture::load_texture(&display, "eh.png").chain_err(|| "failed to load ball texture")?);
    let landscape_texture = Rc::new(texture::load_texture_array(&display, &["mappi.png"]).chain_err(|| "failed to load landscape texture")?);

    let player = world.add_body(Rc::new(mesh::Mesh::from_obj(&display, "ballo.obj").chain_err(|| "failed to load ball mesh")?), 
        eh_texture.clone(),
                   body::BodyShape::Sphere{radius: 1.0},
                   body::BodyConfig{
                       friction: 3.0,
                       density: 0.1,
                        restitution: 0.0,
                       ..body::BodyConfig::default() }
    );
    player.borrow_mut().set_position(Vec3::new(0.0, 3.0, 0.0));

    let world_mesh = mesh::Mesh::from_obj(&display, "mappi.obj")
        .chain_err(|| "failed to load level mesh for draw")?;
    let world_shape =
        body::BodyShape::from_obj("mappi.obj").chain_err(|| "failed to load level mesh for phys")?;

    let landscape = world.add_body(Rc::new(world_mesh),
                                   landscape_texture,
                                   world_shape,
                                   body::BodyConfig { fixed: true, ..Default::default() });
    landscape.borrow_mut().set_position(Vec3::new(0.0, 0.0, 0.0));

    for i in 0..10i32 {
        let ball = world.add_body(Rc::new(mesh::Mesh::from_obj(&display, "ballo.obj").chain_err(|| "failed to load ball mesh")?),
        eh_texture.clone(),
     body::BodyShape::Sphere{radius: 1.0}, body::BodyConfig::default());
        ball.borrow_mut().set_position(Vec3::new(3.0, 3.0 + 3.0 * (i as f32), 0.0));
    }

    let envmap = texture::load_texture(&display, "cubemap.jpg").chain_err(|| "failed to load environment map")?;

    let cube = mesh::Mesh::for_cubemap(&display).unwrap();

    let mut particles = particle::Particles::new(
        &display, vec![texture::load_image("starAlpha.png")?], 100)
                    .chain_err(|| "failed to initialize particle engine")?;

    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();
    let program_array =
        glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER_ARRAY, None).unwrap();

    let mut ino = INotify::init().chain_err(|| "failed to initialize inotify")?;

    ino.add_watch(Path::new("src"), IN_MODIFY | IN_CREATE | IN_DELETE)
        .chain_err(|| "failed to add inotify watch")?;

    let mixer = Rc::new(AudioMixer::new("foldplop_-_memory_song_part_2.ogg")
        .chain_err(|| "failed to initialize audio")?);
    let jump_sound = JumpSound::new().chain_err(|| "failed to load jump sound")?;
    // let hit_sound = Rc::new(HitSound::new().chain_err(|| "failed to load hit sound")?);

    {
        /*
        let player = player.clone();
        let mixer = mixer.clone();
        let hit_sound = hit_sound.clone();
        let handler = move |o1: &np::object::RigidBodyHandle<f32>,
                                o2: &np::object::RigidBodyHandle<f32>| {
            let oi1: isize = o1.index();
            let oi2: isize = o2.index();
            let plri: isize = player.index();
            if oi1 == plri || oi2 == plri {
                // bleh, can't ".chain_err(foo)?" this
                mixer.play(&*hit_sound, ()).expect("failed to play hit sound");
            }
        };
        world.add_contact_handler(handler);*/
    }

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

    let mut times_jumped = 0u32;

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
        //let mut force_y = 0.0;
        let mut force_z = 0.0;

        let force_mag = 10.0;

        let input = input_state.process_input(&mut event_pump);

        if input.quit {
            break 'mainloop;
        }

        if input.jump && allow_jump {
            //force_y = 2.0 * GRAVITY * force_mag;
            times_jumped += 1;
            mixer.play(&jump_sound, (1.0 / (times_jumped as f32),))
                .chain_err(|| "failed to play jump sound")?;
            allow_jump = false;
        } else if !input.jump {
            allow_jump = true;
        }

        if input.reset_camera {
            camera.yaw = 0.0;
            camera.pitch = 0.0;
        }

        if input.stop {
            player.borrow_mut().set_linear_velocity(na::zero());
        }

        camera.yaw += input.camera.x / 10.0;
        camera.pitch += input.camera.y / 10.0;
        camera.pitch = na::clamp(camera.pitch, -PI / 2.0, PI / 2.0);

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
        let camera_pos = player.borrow_mut().get_position() + Vec3::new(0.0, 3.0, 5.0) * camera_rot;
        let znear = 0.01f32;
        /*{
            let cam = camera_pos.to_point();
            let ball = player.borrow_mut().position().translation.to_point();
            let cam_to_ball = (ball - cam).normalize();
            let ray = nc::query::Ray::new(cam, cam_to_ball);
            let mut groups = nc::world::CollisionGroups::new();
            // won't collide with statics with these set
            groups.modify_membership(STATIC_GROUP_ID, false);
            groups.modify_membership(SENSOR_GROUP_ID, false);
            let groups = groups;
            let collisions = world.phys_world().collision_world().interferences_with_ray(&ray, &groups);
            let dep = (ball - cam).norm() - 1.0; // radius
            let mut min = std::f32::MAX;
            for collision in collisions {
                let _obj = collision.0;
                let intersection = collision.1;
                min = min.min(intersection.toi);
            }
            let eps = 0.001;
            // closer than depth to the player ball surface? cut everything to be able to see when
            // camera goes inside walls or other objects
            if min < dep - eps { znear = min; }
        }*/
        let znear = znear;

        force_x += force_mag * input.player.x;
        force_z += force_mag * input.player.y;

        // impulse based:
        // player
        //    .apply_central_impulse(Vec3::new(0.0, force_y, 0.0) * camera_rot);

        // angular momentum based control:
        player.borrow_mut().add_torque(Vec3::new(force_z, 0.0, -force_x) * camera_rot);

        let projection = na::Perspective3::new(display_width as f32 / display_height as f32,
                                               PI / 2.0,
                                               znear,
                                               500.0f32)
            .to_matrix();

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
                  false,
                  false)
            .chain_err(|| "failed to draw cubemap")?;

        let player_pos = player.borrow_mut().get_position();
        for body in world.bodies() {
            let model = body.borrow_mut().get_posrot_homogeneous();
            let modelview = cam_view * model;

            let body::Body { ref mesh, ref texture, .. } = *body.borrow_mut();

            let texture = &**texture;

            let prog = match *texture {
                texture::Texture::Twod(_) => &program,
                texture::Texture::Array(_) => &program_array,
            };

            mesh
                .draw(&mut target,
                      &uniform! {
                      perspective: *projection.as_ref(),
                      modelview: *modelview.as_ref(),
                      tex: texture,
                      player_pos: *player_pos.as_ref(),
                  },
                      prog,
                      true,
                      true) // FIXME only do alpha rendering for ball
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
        if let Some(backtrace) = e.backtrace() {
            println!("Backtrace:\n{:?}", backtrace);
        }
    }
}
