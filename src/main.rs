#[macro_use]
extern crate glium;
extern crate glium_sdl2;
extern crate sdl2;

extern crate image;

extern crate nalgebra as na;

mod types {
    use na;
    pub type Vec2 = na::Vector2<f32>;
    pub type Vec3 = na::Vector3<f32>;
    pub type Pnt3 = na::Point3<f32>;
    pub type Iso3 = na::Isometry3<f32>;
    pub type Mat4 = na::Matrix4<f32>;
}

use glium::Surface;
use na::{Transformation, ToHomogeneous, Transform, Translation, Norm};
use types::*;

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
                                           3.1416 / 2.0, 0.01, 50.0f32).to_matrix();
    let modelview = Iso3::look_at_rh(&Pnt3::new(0.0, 0.0, 0.0), &Pnt3::new(0.0, 0.0, -20.0),
                                     &Vec3::new(0.0, 1.0, 0.0)).to_homogeneous();

    let mut last_t = sdl_timer.ticks();

    'mainloop: loop {
        for ev in event_pump.poll_iter() {
            use sdl2::event::Event;

            match ev {
                Event::Quit { .. } => break 'mainloop,
                _ => ()
            }
        }

        let dt = (sdl_timer.ticks() - last_t) as f32 / 1000.0;
        last_t = sdl_timer.ticks();

        let mut target = display.draw();

        target.clear_color_and_depth((0.0, 0.0, 0.0, 1.0), 1.0);

        target.finish().unwrap();

        std::thread::sleep_ms(1);
    }
}
