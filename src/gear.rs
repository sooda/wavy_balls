use body::Body;
use math::*;

use ode;
pub use ode::dJointType;
pub use ode::dJointTypeHinge;

pub use ode::dParam;
pub use ode::dParamFMax;
pub use ode::dParamVel;

use std;
use std::rc::Rc;
use std::cell::RefCell;

pub struct Gear {
    pub body: Rc<RefCell<Body>>,
    pub ode_joint: ode::dJointID,
}

impl Gear {
    pub fn new(world: ode::dWorldID, body: Rc<RefCell<Body>>, joint_type: ode::dJointType) -> Self {
        // because:
        // error: constant in pattern `dJointTypeHinge` should have an upper case name such as `D_JOINT_TYPE_HINGE`
#[allow(warnings)]
        let joint = match joint_type {
            dJointTypeHinge => unsafe { ode::dJointCreateHinge(world, std::ptr::null_mut()) },
            _ => unreachable!(),
        };

        let anchor = body.borrow().get_position();
        unsafe {
            ode::dJointAttach(joint, body.borrow().ode_body, std::ptr::null_mut());
        }

        let mut g = Gear {
            body: body,
            ode_joint: joint,
        };
        g.set_hinge_anchor(anchor);
        g
    }

    pub fn set_hinge_anchor(&mut self, anchor: Vec3) {
        unsafe {
            ode::dJointSetHingeAnchor(self.ode_joint,
                                      anchor.x as f64,
                                      anchor.y as f64,
                                      anchor.z as f64);
        }
    }

    pub fn set_hinge_axis(&mut self, axis: Vec3) {
        unsafe {
            ode::dJointSetHingeAxis(self.ode_joint, axis.x as f64, axis.y as f64, axis.z as f64);
        }
    }

    // XXX: or a pub fn for each param separately
    pub fn set_hinge_param(&mut self, param: dParam, value: f32) {
        unsafe {
            ode::dJointSetHingeParam(self.ode_joint, param as i32, value as f64);
        }
    }
}

impl Drop for Gear {
    fn drop(&mut self) {
        unsafe {
            ode::dJointDestroy(self.ode_joint);
        }
    }
}
