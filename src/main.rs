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

extern crate nanovg;

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
mod gear;
mod settings;

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
use std::cell::RefCell;
use std::path::Path;
use std::collections::HashSet;

use na::{ToHomogeneous, Rotation3, Norm};
use glium::Surface;
use inotify::INotify;
use inotify::ffi::*;

use math::*;
use audio::{AudioMixer, JumpSound, HitSound, SimpleSound};
use body::Body;
use gear::{Gear, dJointTypeHinge, dParamFMax, dParamVel};
use settings::Settings;

static VERTEX_SHADER: &'static str = r#"
    #version 140

    uniform mat4 perspective;
    uniform mat4 modelview;

    in vec3 position;
    in vec3 normal;
    in vec3 tex_coord;
    in vec3 color_tint;

    out vec3 f_tex_coord;
    out vec3 f_position;
    out vec3 f_normal;
    out vec3 f_color_tint;

    void main() {
        gl_Position = perspective * modelview * vec4(position, 1.0);

        f_tex_coord = tex_coord;
        f_position = position;
        f_normal = normal;
        f_color_tint = color_tint;
    }
"#;

static FRAGMENT_SHADER: &'static str = r#"
    #version 140

    in vec3 f_tex_coord;
    in vec3 f_position;
    in vec3 f_normal;
    in vec3 f_color_tint;

    uniform sampler2D tex;
    uniform vec3 player_pos;

    void main() {
        vec4 color = texture(tex, f_tex_coord.xy);
        if (f_position.y < player_pos.y && length(player_pos.xz - f_position.xz) <= 1.0)
            color.rgb = color.rgb * 0.4;

        gl_FragColor = color;
    }
"#;
static FRAGMENT_SHADER_TERRAIN: &'static str = r#"
    #version 140

    in vec3 f_tex_coord;
    in vec3 f_position;
    in vec3 f_normal;
    in vec3 f_color_tint;

    uniform sampler2D tex;
    uniform vec3 player_pos;

    void main() {
        if (f_position.y < 1.0) {
            discard;
        }
        vec4 color = texture(tex, f_tex_coord.xy);
        if (f_position.y < player_pos.y && length(player_pos.xz - f_position.xz) <= 1.0)
            color.rgb = color.rgb * 0.4;

        color.rgb += f_color_tint;
        color.rgb = clamp(color.rgb, 0.0, 1.0);
        color.rgb *= max(0.2, dot(f_normal, vec3(0,1,0)));
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
        vec4 color = texture(tex, f_tex_coord);
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

enum State {
    Menu(usize),
    Game,
}

fn run() -> Result<()> {
    use glium_sdl2::DisplayBuild;

    let mut gstate = State::Menu(0);

    unsafe {
        ode::dInitODE();
    }

    let settings = Settings::new("settings.txt").chain_err(|| "no settings file found")?;
    let sdl_ctx = sdl2::init().map_err(sdl_err).chain_err(|| "failed to initialize SDL")?;
    let sdl_video = sdl_ctx.video().map_err(sdl_err).chain_err(|| "failed to initialize video")?;
    let _sdl_audio = sdl_ctx.audio().map_err(sdl_err).chain_err(|| "failed to initialize audio")?;
    let sdl_glattr = sdl_video.gl_attr();
    sdl_glattr.set_context_profile(sdl2::video::GLProfile::Core);
    sdl_glattr.set_context_version(3, 3);

    let display_width = settings.get_u32("display_width");
    let display_height = settings.get_u32("display_height");
    let display = sdl_video.window("FGJ", display_width, display_height)
        .build_glium()
        .chain_err(|| "failed to initialize glium context")?;

    let nanovg = nanovg::Context::create_gl3(nanovg::ANTIALIAS | nanovg::STENCIL_STROKES);
    let _nanovg_font = nanovg.create_font("main", "liberationsans.ttf").unwrap();

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

    let buffer = glium::VertexBuffer::new(&display, &mesh)
        .chain_err(|| "failed to allocate GPU vertex buffer")?;
    let mut state = SampleModel {
        buffer: buffer,
        program: load_shader_prog(&display, "test").chain_err(|| "failed to load shader")?,
    };

    let mut last_t = sdl_timer.ticks();

    let scale = 1.0;
    let world = Rc::new(RefCell::new(world::World::new(scale)));
    let eh_texture = Rc::new(
        texture::load_texture(&display, "eh.png")
        .chain_err(|| "failed to load ball texture")?);
    let spin_texture = Rc::new(
        texture::load_texture(&display, "ruohe.png")
        .chain_err(|| "failed to load spin texture")?);
    let diam_texture = Rc::new(
        texture::load_texture(&display, "diamond.png")
        .chain_err(|| "failed to load diamond texture")?);
    let pup0_texture = Rc::new(
        texture::load_texture(&display, "powerup0.png")
        .chain_err(|| "failed to load powerup texture")?);

    let player = world.borrow_mut().add_body(
        Rc::new(RefCell::new(mesh::Mesh::from_obj(&display, "ballo.obj", false)
                .chain_err(|| "failed to load ball mesh")?)),
                eh_texture.clone(),
                Rc::new(body::BodyShape::Sphere{radius: 1.0}),
                body::BodyConfig{
                    friction: 0.4,
                    density: 0.1,
                    restitution: 0.0,
                    category_bits: body::BODY_CATEGORY_PLAYER_BIT,
                    collide_bits: body::BODY_COLLIDE_PLAYER,
                    ..body::BodyConfig::default() }
        );
    player.borrow_mut().set_finite_rotation_mode(true);

    let level_map = texture::load_image("level2.png").chain_err(|| "failed to load level")?;
    let level_body_id = {
        let landscape_texture = Rc::new(
        texture::load_texture(&display, "ruohe.png")
        .chain_err(|| "failed to load landscape texture")?);
        // do not move this. this installs a self pointer to a C callback that shouldn't change
        let body = world.borrow_mut().setup_heightfield(&display, &level_map, landscape_texture);
        let id = body.borrow().id;
        id
    };
    // set player position to 20, 20 and read height from heightfield
    // player.borrow_mut().set_position(settings.get_vec3("player"));
    {
        let reso = world.borrow_mut()
            .heightfield_resolution;

        player.borrow_mut()
            .set_position(Vec3::new(20.0 / scale - reso.0 as f32 * scale / 2.0,
                                    world.borrow_mut().heightfield[((20.0 / scale) * reso.0 as f32 + 20.0 / scale) as usize] +
                                    5.0,
                                    20.0 / scale - reso.1 as f32 * scale / 2.0));
    }

    let diamonds = Rc::new(RefCell::new(Vec::new()));
    let mut diams_tot = 0;
    let mut diams_got = 0;
    let diam_shape = Rc::new(
        body::BodyShape::from_obj("diamond.obj")
        .chain_err(|| "failed to load diamond mesh for phys")?);
    let diam_mesh = Rc::new(
        RefCell::new(mesh::Mesh::from_obj(&display, "diamond.obj", false)
                     .chain_err(|| "failed to load diamond mesh")?));
    let mut diamgears = Vec::new();
    {
        let (width, depth) = (level_map.width as i32, level_map.height as i32);

        for x in 0..width {
            for z in 0..depth {
                let hmp = (z * width + x) as usize;
                let r = level_map.data[hmp * 4 + 0] as f32 / 256.0;
                let g = level_map.data[hmp * 4 + 1] as f32 / 256.0;
                let _b = level_map.data[hmp * 4 + 2] as f32 / 256.0;
                let px = x as f32 * scale;
                let pz = z as f32 * scale;

                if g > 0.5 {
                    let p = Vec3::new((px + 0.5 * scale) - scale * 0.5 * width as f32,
                                      r * 8.0 * scale + 1.5,
                                      (pz + 0.5 * scale) - scale * 0.5 * depth as f32);
                    println!("{:?}", p);
                    let diamond = world.borrow_mut().add_body(diam_mesh.clone(),
                                                              diam_texture.clone(),
                                                              diam_shape.clone(),
                                                              body::BodyConfig {
                                                                  collide_sound: Some(0),
                                                                  ..Default::default()
                                                              });
                    diamond.borrow_mut().set_position(p);
                    diamonds.borrow_mut().push(diamond.borrow().id);
                    let mut gear = Gear::new(world.borrow_mut().ode_world(),
                                             diamond.clone(),
                                             dJointTypeHinge);
                    gear.set_hinge_axis(Vec3::new(0.0, 1.0, 0.0));
                    gear.set_hinge_param(dParamFMax, 1000.0);
                    gear.set_hinge_param(dParamVel, 1.0);
                    diamgears.push(gear);
                    diams_tot += 1;
                }
            }
        }
    }


    let spin_mesh = mesh::Mesh::from_obj(&display, "spinthing.obj", false)
        .chain_err(|| "failed to load spinthing mesh for draw")?;
    let spin_shape = Rc::new(
        body::BodyShape::from_obj("spinthing.obj")
        .chain_err(|| "failed to load spinthing mesh for phys")?);

    let spinthing = world.borrow_mut().add_body(Rc::new(RefCell::new(spin_mesh)),
                                                spin_texture.clone(),
                                                spin_shape,
                                                body::BodyConfig {
                                                    category_bits: body::BODY_CATEGORY_GEAR_BIT,
                                                    collide_bits: body::BODY_COLLIDE_GEAR,
                                                    ..Default::default()
                                                });
    spinthing.borrow_mut().set_position(settings.get_vec3("spinthing"));

    // this spins around y axis, i.e., on the ground
    let mut testgear = Gear::new(world.borrow_mut().ode_world(),
                                 spinthing.clone(),
                                 dJointTypeHinge);
    testgear.set_hinge_axis(Vec3::new(0.0, 1.0, 0.0));
    testgear.set_hinge_param(dParamFMax, 1000.0);
    testgear.set_hinge_param(dParamVel, 1.0);

    let mesh = mesh::Mesh::from_obj(&display, "gear.obj", false)
        .chain_err(|| "failed to load gear mesh for draw")?;
    let shape = Rc::new(
        body::BodyShape::from_obj("gear.obj")
        .chain_err(|| "failed to load gear mesh for phys")?);


    let body = world.borrow_mut().add_body(Rc::new(RefCell::new(mesh)),
                                           spin_texture.clone(),
                                           shape,
                                           body::BodyConfig {
                                               category_bits: body::BODY_CATEGORY_GEAR_BIT,
                                               collide_bits: body::BODY_COLLIDE_GEAR,
                                               ..Default::default()
                                           });

    body.borrow_mut().set_position(settings.get_vec3("liftgear"));
    // this spins around x axis, i.e., lifts things up
    let mut liftgear = Gear::new(world.borrow_mut().ode_world(),
                                 body.clone(),
                                 dJointTypeHinge);
    liftgear.set_hinge_axis(Vec3::new(1.0, 0.0, 0.0));
    liftgear.set_hinge_param(dParamFMax, 1000.0);
    liftgear.set_hinge_param(dParamVel, -1.0);

    let envmap = texture::load_texture_array(
        &display, &[
            "cubemap/negx.jpg",
            "cubemap/posx.jpg",
            "cubemap/negy.jpg",
            "cubemap/posy.jpg",
            "cubemap/negz.jpg",
            "cubemap/posz.jpg",
        ])
        .chain_err(|| "failed to load environment map")?;

    let cube = mesh::Mesh::for_cubemap(&display).unwrap();

    let mut particles = particle::Particles::new(
        &display, vec![texture::load_image("Smoke10.png")?], 100)
                    .chain_err(|| "failed to initialize particle engine")?;
    let new_particles = Rc::new(RefCell::new(vec![]));

    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();
    let program_terrain =
        glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER_TERRAIN, None)
            .unwrap();
    let program_array =
        glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER_ARRAY, None).unwrap();

    let mut ino = INotify::init().chain_err(|| "failed to initialize inotify")?;

    ino.add_watch(Path::new("src"), IN_MODIFY | IN_CREATE | IN_DELETE)
        .chain_err(|| "failed to add inotify watch")?;

    let mixer =
        Rc::new(AudioMixer::new("duunimusa2.ogg", "menu2.ogg").chain_err(|| "failed to initialize audio")?);
    let jump_sound = JumpSound::new().chain_err(|| "failed to load jump sound")?;
    let hit_sound = Rc::new(HitSound::new().chain_err(|| "failed to load hit sound")?);
    let diamond_sounds = vec![
        Rc::new(SimpleSound::new("sounds/elektro.wav")
                .chain_err(|| "failed to load elektro sound")?),
        Rc::new(SimpleSound::new("sounds/powerup1.wav")
                .chain_err(|| "failed to load powerup1 sound")?),
    ];
    let end_sound = Rc::new(SimpleSound::new("sounds/game_over.wav")
                .chain_err(|| "failed to load gameover sound")?);
    let win_sounds = vec![
        Rc::new(SimpleSound::new("sounds/great.wav")
                .chain_err(|| "failed to load great sound")?),
        Rc::new(SimpleSound::new("sounds/unbelievable.wav")
                .chain_err(|| "failed to load unbelievable sound")?),
    ];
    let on_ground = Rc::new(RefCell::new(false));
    {
        let plr_id = player.borrow_mut().id;
        let ground_id = level_body_id;
        let mixer = mixer.clone();
        let hit_sound = hit_sound.clone();
        let diamonds = diamonds.clone();
        let new_particles = new_particles.clone();
        let vol_scale = settings.get_f32("volume_scale");
        let on_ground = on_ground.clone();
        let landscape_sound_handler = move |o1: &mut Body,
                                            o2: &mut Body,
                                            contact: &mut ode::dContact| {
            // diamonds don't cause a sound here
            if !diamonds.borrow().contains(&o1.id) && !diamonds.borrow().contains(&o2.id) {
                if o1.id == plr_id || o2.id == plr_id {
                    let vel1 = o1.get_linear_velocity();
                    let vel2 = o2.get_linear_velocity();
                    let delta_vel = vel1 - vel2;
                    let normal = Vec3::new(contact.geom.normal[0] as f32,
                                           contact.geom.normal[1] as f32,
                                           contact.geom.normal[2] as f32);
                    let coincide_vel = na::dot(&normal, &delta_vel).abs();
                    let volume = (vol_scale * coincide_vel * coincide_vel).min(1.0);
                    // TODO: multiple different sounds for even more dramatic collisions
                    if volume > 0.01 {
                        // bleh, can't ".chain_err(foo)?" this result in a handler
                        mixer.play(&*hit_sound, (volume,)).expect("failed to play hit sound");

                        for a in 0..10 {
                            let angle = (a as f32 / 10.0) * 2.0 * 3.1416;
                            let part = particle::Particle {
                                position: o1.get_position().to_point() + Vec3::new(0.0, -1.0, 0.0),
                                scale: Vec2::new(volume, volume) * 2.5,
                                velocity: Vec3::new(angle.cos() * 3.0, 0.2, angle.sin() * 3.0),
                                lifetime: Some(0.75),
                                texture: 0,
                                ..Default::default()
                            };
                            new_particles.borrow_mut().push(part);
                        }
                    }
                    if o1.id == ground_id || o2.id == ground_id {
                        *on_ground.borrow_mut() = true;
                    }
                }
            }
            false
        };
        world.borrow_mut().add_contact_handler(Box::new(landscape_sound_handler));
    }
    let del_diamonds: Rc<RefCell<HashSet<u64>>> = Rc::new(RefCell::new(HashSet::new()));
    {
        let plr_id = player.borrow_mut().id;
        let del_diamonds = del_diamonds.clone();
        let diamonds = diamonds.clone();
        let diamond_collision_handler =
            move |o1: &mut Body, o2: &mut Body, _contact: &mut ode::dContact| {
                if o1.id == plr_id || o2.id == plr_id {
                    let (_player, diamond) = if o1.id == plr_id { (o1, o2) } else { (o2, o1) };
                    if diamonds.borrow().contains(&diamond.id) {
                        del_diamonds.borrow_mut().insert(diamond.id);
                        // don't cause physical collision
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
        world.borrow_mut().add_contact_handler(Box::new(diamond_collision_handler));
    }

    let body = world.borrow_mut().add_body(
        Rc::new(RefCell::new(mesh::Mesh::from_obj(&display, "powerup0.obj", false)
                             .chain_err(|| "failed to load powerup mesh")?)),
                             pup0_texture.clone(),
                             diam_shape.clone(),
                             body::BodyConfig { collide_sound: Some(1), ..Default::default() });
    body.borrow_mut().set_position(settings.get_vec3("pup0"));
    diamonds.borrow_mut().push(body.borrow().id);
    let ebin_powerup = body.borrow().id;

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

    let mut fov = PI / 2.0;

    let force_mag_duration = 10 * 1000;
    let mut force_mag_end = sdl_timer.ticks();

    let mut endtime = 0;

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

        let input = input_state.process_input(&mut event_pump);

        if input.quit {
            break 'mainloop;
        }

        let mut target = display.draw();

        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        gstate = match gstate {
            State::Game => {

                let mut force_x = 0.0;
                let mut force_y = 0.0;
                let mut force_z = 0.0;

                let force_mag = if last_t >= force_mag_end {
                    10.0 * 2.0
                } else {
                    31.4 * 2.0
                };

                if input.jump && allow_jump && *on_ground.borrow() {
                    force_y = 3.14 * GRAVITY * force_mag;
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

                {
                    let mut pp = new_particles.borrow_mut();
                    for part in pp.drain(..) {
                        particles.add(part);
                    }
                }

                // Step the world
                let player_position = player.borrow_mut().get_position();
                println!("{:?}", player_position);
                if player_position.y < -300.0 && endtime == 0 {
                    endtime = sdl_timer.ticks();
                    mixer.play(&*end_sound, ()).chain_err(|| "failed to play end sound")?;
                }
                *on_ground.borrow_mut() = false;
                world.borrow_mut().step(dt,
                                        player_position,
                                        input.action,
                                        (settings.get_f32("heightaction_power"),
                                         settings.get_f32("heightaction_damp"),
                                         settings.get_f32("heightaction_sin")));
                particles.step(dt);

                for &body_id in del_diamonds.borrow().iter() {
                    let mut w = world.borrow_mut();
                    {
                        let body = w.bodies().iter().find(|&x| x.borrow().id == body_id).unwrap();
                        if let Some(idx) = body.borrow().collide_sound {
                            mixer.play(&*diamond_sounds[idx], ())
                                .chain_err(|| "failed to play diamond sound")?;
                        }
                        if body.borrow().id == ebin_powerup {
                            force_mag_end = sdl_timer.ticks() + force_mag_duration;
                        } else {
                            // normal prize diamond
                            // TODO enum these
                            diams_got += 1;
                        }
                    }
                    w.del_body(body_id);
                    diamonds.borrow_mut().retain(|&x| x != body_id);
                    if diams_got == diams_tot {
                        endtime = sdl_timer.ticks();
                        let idx = if endtime % 1000 > 500 { 1 } else { 0 }; // random, lol
                        mixer.play(&*win_sounds[idx], ()).chain_err(|| "failed to play win sound")?;
                    }
                }
                del_diamonds.borrow_mut().clear();

                let camera_rot = Rotation3::new(Vec3::new(camera.pitch, 0.0, 0.0)) *
                                 Rotation3::new(Vec3::new(0.0, camera.yaw, 0.0));
                let camera_pos = player.borrow_mut().get_position() +
                                 Vec3::new(0.0, 3.0, 5.0) * camera_rot;

                let zfar = 5000.0f32;
                let znear_default = 0.01f32;
                // FIXME
                let znear = if true {
                    znear_default
                } else {
                    let cam = camera_pos.to_point();
                    let ball = player.borrow_mut().get_position().to_point();
                    let cam_to_ball = (ball - cam).normalize();
                    let maxdep = (ball - cam).norm() - 1.0; // radius

                    // welp. doesn't return the closest first.
                    // TODO: put the ground in its own space maybe
                    let maxhits = 100usize;

                    let mut contacts = Vec::<ode::dContactGeom>::with_capacity(maxhits);
                    unsafe {
                        let ray = ode::dCreateRay(std::ptr::null_mut(), zfar as f64);
                        ode::dGeomRaySet(ray,
                                         cam.x as f64,
                                         cam.y as f64,
                                         cam.z as f64,
                                         cam_to_ball.x as f64,
                                         cam_to_ball.y as f64,
                                         cam_to_ball.z as f64);
                        let found = ode::dCollide(ray,
                                          world.borrow_mut().ode_space() as ode::dGeomID,
                                          maxhits as i32,
                                          contacts.as_mut_ptr(),
                                          std::mem::size_of::<ode::dContactGeom>() as i32) as
                            usize;
                        contacts.set_len(found);
                        ode::dGeomDestroy(ray);
                    }

                    let eps = 0.001;

                    let mut dep = maxdep + eps;
                    for c in contacts {
                        if (c.depth as f32) < dep {
                            dep = c.depth as f32;
                        }
                    }
                    // closer than depth to the player ball surface? cut everything to be able to see when
                    // camera goes inside walls or other objects
                    if dep < maxdep - eps {
                        dep
                    } else {
                        znear_default
                    }
                };

                if *on_ground.borrow() {
                    force_x += force_mag * input.player.x;
                    force_z += force_mag * input.player.y;
                }

                // impulse based:
                player.borrow_mut().add_force(Vec3::new(0.0, force_y, 0.0) * camera_rot);

                // angular momentum based control:
                player.borrow_mut().add_torque(Vec3::new(force_z, 0.0, -force_x) * camera_rot);

                fov = (fov + input.zoom).max(PI / 8.0).min(7.0 / 8.0 * PI);

                let projection = na::Perspective3::new(display_width as f32 /
                                                       display_height as f32,
                                                       fov,
                                                       znear,
                                                       zfar)
                    .to_matrix();

                // iso is rotation followed by translation, can't use it directly just like that
                let cam_rotate = Iso3::from_rotation_matrix(na::zero(), camera_rot)
                    .to_homogeneous();
                let cam_translate = Iso3::new(-camera_pos, na::zero()).to_homogeneous();
                let cam_view = cam_rotate * cam_translate;

                cube.draw(&mut target,
                          &uniform! {
                    perspective: *projection.as_ref(),
                    modelview: *cam_rotate.as_ref(),
                    tex: &envmap
                  },
                          &program_array,
                          false,
                          false)
                    .chain_err(|| "failed to draw cubemap")?;

                let player_pos = player.borrow_mut().get_position();

                for body in world.borrow().bodies() {
                    let model = body.borrow_mut().get_posrot_homogeneous();
                    let modelview = cam_view * model;

                    let b = body.borrow_mut();
                    // i have no idea what i'm doing. this can't be right. thanks, compiler
                    if let (&Some(ref mesh), &Some(ref texture), ref shape) = (&b.mesh,
                                                                               &b.texture,
                                                                               &b.shape) {
                        let ref texture = **texture;

                        let prog = match ***shape {
                            body::BodyShape::HeightField => &program_terrain,
                            _ => {
                                match *texture {
                                    texture::Texture::Twod(_) => &program,
                                    texture::Texture::Array(_) => &program_array,
                                }
                            }
                        };

                        mesh
                .borrow_mut().draw(&mut target,
                      &uniform! {
                      perspective: *projection.as_ref(),
                      modelview: *modelview.as_ref(),
                      tex: &*texture,
                      player_pos: *player_pos.as_ref(),
                  },
                      prog,
                      true,
                      true) // FIXME only do alpha rendering for ball
                .chain_err(|| "failed to draw mesh")?;
                    }
                }

                particles.draw(&mut target, *projection.as_ref(), *cam_view.as_ref())
                    .chain_err(|| "failed to render particles")?;

                render(&mut target, &state, sdl_timer.ticks() as f32 / 1000.0);

                nanovg.begin_frame(800, 600, 1.0);
                nanovg.begin_path();
                nanovg.move_to(10.0, 50.0);
                nanovg.line_to(10.0, 100.0);
                nanovg.line_to(400.0, 100.0);
                nanovg.line_to(400.0, 50.0);
                nanovg.fill_color(nanovg::Color::rgba(0, 0, 0, 128));
                nanovg.fill();

                nanovg.font_size(32.0);
                nanovg.font_face("main");
                nanovg.stroke_color(nanovg::Color::rgba(255, 255, 255, 255));
                nanovg.fill_color(nanovg::Color::rgba(255, 255, 255, 255));
                let playtime = if endtime == 0 {
                    sdl_timer.ticks()
                } else {
                    endtime
                } as f32 / 1000.0;
                nanovg.text(20.0,
                            90.0,
                            &format!("diamonds {}/{} time {:.2} s",
                                     diams_got,
                                     diams_tot,
                                     playtime));

                nanovg.end_frame();

                State::Game

            }
            State::Menu(sel) => {
                nanovg.begin_frame(800, 600, 1.0);

                nanovg.begin_path();
                nanovg.move_to(10.0, [25.0, 75.0][sel]);
                nanovg.line_to(10.0, [60.0, 110.0][sel]);
                nanovg.line_to(200.0, [60.0, 110.0][sel]);
                nanovg.line_to(200.0, [25.0, 75.0][sel]);
                nanovg.fill_color(nanovg::Color::rgba(255, 0, 0, 128));
                nanovg.fill();

                nanovg.font_size(32.0);
                nanovg.font_face("main");
                nanovg.stroke_color(nanovg::Color::rgba(255, 255, 255, 255));
                nanovg.fill_color(nanovg::Color::rgba(255, 255, 255, 255));
                nanovg.text(20.0, 50.0, "Play");

                nanovg.text(20.0, 100.0, "Quit");

                nanovg.end_frame();

                if input.player.y < 0.0 {
                    State::Menu(0)
                } else if input.player.y > 0.0 {
                    State::Menu(1)
                } else if input.jump && sel == 0 {
                    mixer.play_music()?;
                    State::Game
                } else if input.jump && sel == 1 {
                    break 'mainloop;
                } else {
                    State::Menu(sel)
                }
            }
        };

        {
            use glium::backend::Facade;
            let gctx = display.get_context();
            let mut state = gctx.get_state().borrow_mut();
            *state = Default::default();
        }

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
