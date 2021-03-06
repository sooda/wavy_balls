#![allow(dead_code)]

use na;

pub type Vec2 = na::Vector2<f32>;
pub type Vec3 = na::Vector3<f32>;
pub type Vec4 = na::Vector4<f32>;
pub type Pnt2 = na::Point2<f32>;
pub type Pnt3 = na::Point3<f32>;
pub type Iso3 = na::Isometry3<f32>;
pub type Mat4 = na::Matrix4<f32>;

pub const GRAVITY: f32 = 9.80665;
pub const PHYS_DT: f32 = 0.01;
pub const PI: f32 = 3.1416;
