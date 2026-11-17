//! Live camera capture and video playback (Video.pde) — not yet ported.
//!
//! The Processing sketch used `processing.video` (GStreamer) for both. There is no
//! dependency-light equivalent in Rust yet, so these types keep the sketch's interface and
//! parameter handling but report the camera as unavailable. A real implementation can be dropped
//! in behind the same methods (e.g. with `nokhwa` for cameras or `gstreamer` for files) and its
//! frames drawn where `State::render` composites the visualiser.

use crate::config::CameraSettings;
use crate::util::map_midi;

pub struct LiveCapture {
    available: bool,
    pub param1: f32,
    pub param2: f32,
    pub param3: f32,
    pub param4: f32,
    pub param5: f32,
}

impl LiveCapture {
    pub fn new(camera: Option<&CameraSettings>) -> Self {
        if let Some(c) = camera {
            log::info!(
                "Video camera {} ({}x{}) requested, but camera capture is not supported in the \
                 Rust port yet; video functions are turned off",
                c.index,
                c.width,
                c.height
            );
        }
        LiveCapture {
            available: false,
            param1: 0.5,
            param2: 0.0,
            param3: 0.5,
            param4: 0.0,
            param5: 0.0,
        }
    }

    #[allow(dead_code)] // used once capture is implemented
    pub fn available(&self) -> bool {
        self.available
    }

    pub fn set_param1(&mut self, v: i32) {
        self.param1 = map_midi(v, 0.0, 1.0);
    }
    pub fn set_param2(&mut self, v: i32) {
        self.param2 = map_midi(v, 0.0, 20.0).trunc();
    }
    pub fn set_param3(&mut self, v: i32) {
        self.param3 = map_midi(v, 0.0, 1.0);
    }
    pub fn set_param4(&mut self, v: i32) {
        self.param4 = map_midi(v, 2.0, 50.0).trunc();
    }
    pub fn set_param5(&mut self, v: i32) {
        self.param5 = map_midi(v, 2.0, 50.0).trunc();
    }
}
