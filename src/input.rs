use sdl2;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Scancode};
use math::*;
use na;

#[derive(Debug)]
pub struct Input {
    pub quit: bool,
    pub jump: bool,
    pub reset_camera: bool,
    pub stop: bool,

    pub camera: Vec2,
    pub player: Vec2,

    pub action: bool,

    pub zoom: f32,
}

impl Default for Input {
    fn default() -> Input {
        Input {
            quit: false,
            jump: false,
            reset_camera: false,
            stop: false,

            camera: na::zero(),
            player: na::zero(),

            action: false,

            zoom: 0.0,
        }
    }
}

pub struct InputState {
    controller: Option<sdl2::controller::GameController>,
}

impl InputState {
    pub fn new(controller: Option<sdl2::controller::GameController>) -> InputState {
        InputState { controller: controller }
    }

    pub fn process_input(&mut self, pump: &mut sdl2::EventPump) -> Input {
        let mut input: Input = Default::default();

        for ev in pump.poll_iter() {
            match ev {
                Event::Quit { .. } => input.quit = true,
                Event::KeyDown { keycode, .. } => {
                    match keycode {
                        Some(Keycode::Escape) => input.quit = true,
                        Some(Keycode::Space) => input.jump = true,
                        Some(Keycode::R) => input.reset_camera = true,
                        Some(Keycode::S) => input.stop = true,
                        Some(Keycode::A) => input.action = true,
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        if let Some(ref ctrl) = self.controller {
            use sdl2::controller::Axis::*;
            use sdl2::controller::Button::*;
            use std::cmp::max;

            let deadzone = 4000;
            let clamp_deadzone = |f: i16| max(f.abs() - deadzone, 0) * f.signum();

            let scale = 1.0 / i16::max_value() as f32;
            input.camera = Vec2::new(clamp_deadzone(ctrl.axis(RightX)) as f32 * scale,
                                     clamp_deadzone(ctrl.axis(RightY)) as f32 * scale);
            input.player = Vec2::new(clamp_deadzone(ctrl.axis(LeftX)) as f32 * scale,
                                     clamp_deadzone(ctrl.axis(LeftY)) as f32 * scale);

            if ctrl.button(LeftShoulder) {
                input.zoom += 0.05;
            }
            if ctrl.button(RightShoulder) {
                input.zoom -= 0.05;
            }

            if ctrl.button(Back) {
                input.quit = true;
            }
            if ctrl.button(A) {
                input.jump = true;
            }
            if ctrl.button(X) {
                input.reset_camera = true;
            }
            if ctrl.button(Y) {
                input.stop = true;
            }
            if ctrl.button(B) {
                input.action = true;
            }
        } else {
            let m = pump.relative_mouse_state();
            let scale = 1.0 / 10.0;
            if m.left() {
                input.camera = Vec2::new(m.x() as f32 * scale, m.y() as f32 * scale);
            }
        }

        let kb = pump.keyboard_state();
        if kb.is_scancode_pressed(Scancode::Left) {
            input.player.x -= 1.0;
        }
        if kb.is_scancode_pressed(Scancode::Right) {
            input.player.x += 1.0;
        }
        if kb.is_scancode_pressed(Scancode::Up) {
            input.player.y -= 1.0;
        }
        if kb.is_scancode_pressed(Scancode::Down) {
            input.player.y += 1.0;
        }

        input
    }
}
