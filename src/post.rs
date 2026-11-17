//! Post-processing shaders (Shaders.pde): the selectable effects, the shaders that represent
//! Traktor's effects (filter -> blur, echo -> sobel, gater/slicer/masher -> VHS glitch) and the
//! kaleidoscope.

use crate::gpu::ShaderPass;
use crate::util::{map, map_midi, pround, random, MIDI_MAX};
use nannou::wgpu;
use std::f32::consts::TAU;
use std::path::Path;

/// Values that change every frame and that most shaders need.
#[derive(Copy, Clone, Debug)]
pub struct ShaderTime {
    /// Seconds since start (`frameTimeSecs`).
    pub time: f32,
    pub frame_count: u64,
    pub width: f32,
    pub height: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PostKind {
    Brcosa,
    Hue,
    Pixelate,
    Channels,
    Threshold,
    Neon,
    Deform,
    PixelRolls,
    Modcolor,
    Halftone,
    Invert,
    Strobe,
    NeonSpark,
    VoronoiSlice,
    VhsGlitch,
    Sobel,
}

impl PostKind {
    fn info(self) -> (&'static str, &'static str) {
        match self {
            PostKind::Brcosa => ("brcosa", "brcosa.glsl"),
            PostKind::Hue => ("hue", "hue.glsl"),
            PostKind::Pixelate => ("pixelate", "pixelate.glsl"),
            PostKind::Channels => ("channels", "channels.glsl"),
            PostKind::Threshold => ("threshold", "threshold.glsl"),
            PostKind::Neon => ("neon", "neon.glsl"),
            PostKind::Deform => ("deform", "deform.glsl"),
            PostKind::PixelRolls => ("pixelRolls", "pixelrolls.glsl"),
            PostKind::Modcolor => ("modcolor", "modcolor.glsl"),
            PostKind::Halftone => ("halftone", "halftone.glsl"),
            PostKind::Invert => ("inversion", "invert.glsl"),
            PostKind::Strobe => ("strobe", "strobe.glsl"),
            PostKind::NeonSpark => ("neonSpark", "neonSpark.glsl"),
            PostKind::VoronoiSlice => ("voronoiSlice", "voronoiSlice.glsl"),
            PostKind::VhsGlitch => ("VHS Glitch", "vhs_glitch.glsl"),
            PostKind::Sobel => ("Sobel", "sobel.glsl"),
        }
    }
}

/// One post-processing shader with its two parameters (`x`, `y`, both 0..127).
pub struct VisShader {
    pub name: &'static str,
    pub kind: PostKind,
    pub pass: ShaderPass,
    pub x: i32,
    pub y: i32,
}

impl VisShader {
    pub fn load(
        device: &wgpu::Device,
        dir: &Path,
        kind: PostKind,
        w: f32,
        h: f32,
    ) -> anyhow::Result<Self> {
        let (name, file) = kind.info();
        let mut s = VisShader {
            name,
            kind,
            pass: ShaderPass::load(device, dir, file)?,
            x: MIDI_MAX / 2,
            y: MIDI_MAX / 2,
        };
        let p = &mut s.pass;
        match kind {
            PostKind::Brcosa => {
                s.x = 0;
                s.y = 0;
                p.set_f32("brightness", 1.0);
            }
            PostKind::VhsGlitch | PostKind::Sobel => p.set_vec2("iResolution", w, h),
            PostKind::NeonSpark => {
                p.set_vec2("iResolution", w, h);
                p.set_f32("intensity", 10.0);
                p.set_f32("windSpeed", 0.1);
                p.set_f32("windStrength", 0.15);
                p.set_f32("windAngle", 0.0);
                p.set_f32("r", 1.0);
                p.set_f32("g", 0.4);
                p.set_f32("b", 0.64);
                p.set_bool("useRainbow", false);
                p.set_f32("hueSpeed", 0.5);
            }
            PostKind::VoronoiSlice => {
                p.set_vec2("iResolution", w, h);
                p.set_f32("gridLineWidth", 0.02);
                p.set_f32("speed", 1.0);
                p.set_vec2("centerPoint", 0.5, 0.5);
            }
            _ => {}
        }
        Ok(s)
    }

    pub fn set_x(&mut self, i: i32, width: f32) {
        self.x = i;
        if self.kind == PostKind::Deform {
            // Snap to the middle (kept from the sketch, which compares against width/2)
            let mid = (width / 2.0) as i32;
            if self.x < mid + 10 && self.x > mid - 10 {
                self.x = mid;
            }
        }
    }

    pub fn set_y(&mut self, i: i32, height: f32) {
        self.y = i;
        if self.kind == PostKind::Deform {
            let mid = (height / 2.0) as i32;
            if self.y < mid + 10 && self.y > mid - 10 {
                self.y = mid;
            }
        }
    }

    /// Parameter automation on the beat (only deform reacts).
    pub fn beat_xy(&mut self) {
        if self.kind == PostKind::Deform {
            self.x += random(-50.0, 50.0) as i32;
            self.y += random(-50.0, 50.0) as i32;
        }
    }

    /// Set the per-frame uniforms, as each class's `draw()` did before `filter()`.
    pub fn prepare(&mut self, t: ShaderTime) {
        let (x, y) = (self.x, self.y);
        let p = &mut self.pass;
        match self.kind {
            PostKind::Brcosa => {
                p.set_f32("contrast", map_midi(x, -5.0, 5.0));
                p.set_f32("saturation", map_midi(y, -5.0, 5.0));
            }
            PostKind::Hue => p.set_f32("hue", map_midi(x, 0.0, TAU)),
            PostKind::Pixelate => {
                let px = map((MIDI_MAX - x) as f32, 0.0, MIDI_MAX as f32, 0.0, t.width);
                p.set_vec2("pixels", 0.1 * px, 0.1 * px);
            }
            PostKind::Channels => {
                p.set_vec2("rbias", map_midi(x, -0.2, 0.2), 0.0);
                p.set_vec2("gbias", map_midi(MIDI_MAX - y, -0.4, 0.0), 0.0);
                p.set_vec2("bbias", 0.0, 0.0);
                p.set_vec2("rmult", 1.0, 1.0);
                p.set_vec2("gmult", 1.0, 1.0);
                p.set_vec2("bmult", 1.0, 1.0);
            }
            PostKind::Threshold => p.set_f32("threshold", map_midi(x, 0.0, 1.0)),
            PostKind::Neon => {
                p.set_f32("brt", map_midi(x, 0.0, 0.5));
                p.set_i32("rad", map_midi(y, 0.0, 3.0) as i32);
            }
            PostKind::Deform => {
                p.set_f32("time", t.time);
                p.set_vec2(
                    "mouse",
                    map_midi(x, 0.0, t.width) / t.width,
                    map_midi(y, 0.0, t.width) / t.height,
                );
                p.set_f32(
                    "turns",
                    map((0.01 * t.frame_count as f32).sin(), -1.0, 1.0, 2.0, 10.0),
                );
            }
            PostKind::PixelRolls => {
                p.set_f32("time", t.time);
                p.set_vec2("pixels", (x / 5) as f32, 150.0);
                p.set_f32("rollRate", map_midi(y, -0.5, 0.5));
                p.set_f32("rollAmount", 0.25);
            }
            PostKind::Modcolor => {
                p.set_f32("modr", map_midi(x, 0.0, 0.5));
                p.set_f32("modg", 0.3);
                p.set_f32("modb", map_midi(y, 0.0, 0.5));
            }
            PostKind::Halftone => {
                p.set_i32("pixelsPerRow", map_midi(MIDI_MAX - x, 2.0, 100.0) as i32)
            }
            PostKind::Invert | PostKind::Sobel => {}
            PostKind::Strobe => {
                p.set_f32("time", t.time);
                p.set_f32("freq", map_midi(x, 0.01, 0.1));
            }
            PostKind::NeonSpark => {
                p.set_f32("time", t.time);
                p.set_f32("dissolve", map_midi(x, 0.0, 1.0));
                p.set_f32("pixelSize", map_midi(y, 0.003, 0.03));
            }
            PostKind::VoronoiSlice => {
                p.set_f32("time", t.time);
                p.set_f32("randomness", map_midi(x, 0.0, 1.0));
                p.set_f32("cellScale", map_midi(y, 2.0, 30.0));
            }
            PostKind::VhsGlitch => p.set_f32("iGlobalTime", t.time),
        }
    }
}

/// The list of selectable post-processing shaders.
pub struct PostProcessingShaders {
    pub shaders: Vec<VisShader>,
    pub current: usize,
    pub on: bool,
    pub auto_on: bool,
    x: i32,
    y: i32,
}

impl PostProcessingShaders {
    pub const KINDS: [PostKind; 14] = [
        PostKind::Brcosa,
        PostKind::Hue,
        PostKind::Pixelate,
        PostKind::Channels,
        PostKind::Threshold,
        PostKind::Neon,
        PostKind::Deform,
        PostKind::PixelRolls,
        PostKind::Modcolor,
        PostKind::Halftone,
        PostKind::Invert,
        PostKind::Strobe,
        PostKind::NeonSpark,
        PostKind::VoronoiSlice,
    ];

    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> Self {
        let shaders = Self::KINDS
            .iter()
            .filter_map(|k| match VisShader::load(device, dir, *k, w, h) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::error!("Skipping post shader: {e}");
                    None
                }
            })
            .collect();
        PostProcessingShaders {
            shaders,
            current: 0,
            on: false,
            auto_on: false,
            x: MIDI_MAX / 2,
            y: MIDI_MAX / 2,
        }
    }

    pub fn current_mut(&mut self) -> Option<&mut VisShader> {
        self.shaders.get_mut(self.current)
    }

    /// Returns the selected index so the caller can light up the controller.
    pub fn select(&mut self, value: i32) -> usize {
        let n = self.shaders.len().max(1);
        let v = map_midi(value, 0.0, (n - 1) as f32) as i32;
        self.current = v.clamp(0, n as i32 - 1) as usize;
        self.current
    }

    pub fn info(&self) -> String {
        match self.shaders.get(self.current) {
            Some(s) => format!("{} X = {} Y = {}", s.name, self.x, self.y),
            None => "none".into(),
        }
    }

    pub fn beat_xy(&mut self) {
        if self.auto_on {
            if let Some(s) = self.current_mut() {
                s.beat_xy();
            }
        }
    }
}

/// Blur used to show Traktor's filter knob.
pub struct BlurShader {
    pub pass: ShaderPass,
}

impl BlurShader {
    pub fn new(device: &wgpu::Device, dir: &Path) -> anyhow::Result<Self> {
        let mut pass = ShaderPass::load(device, dir, "blurFrag.glsl")?;
        pass.set_f32("blurDegree", 0.0);
        Ok(BlurShader { pass })
    }
}

/// Kaleidoscope applied to the visualiser, hotcue images and waveform.
pub struct Kaleidoscope {
    pub pass: ShaderPass,
    pub rotation: f32,
    pub view_angle: f32,
}

impl Kaleidoscope {
    pub const DEFAULT_VALUE: i32 = 60;

    pub fn new(device: &wgpu::Device, dir: &Path) -> anyhow::Result<Self> {
        let mut pass = ShaderPass::load(device, dir, "kaleidoscope.glsl")?;
        let view_angle = (Self::DEFAULT_VALUE as f32).to_radians();
        pass.set_f32("rotation", 0.0);
        pass.set_f32("viewAngle", view_angle);
        Ok(Kaleidoscope {
            pass,
            rotation: 0.0,
            view_angle,
        })
    }

    pub fn set_rotation(&mut self, value: i32) {
        self.rotation = map_midi(value, TAU, 0.0);
        self.pass.set_f32("rotation", self.rotation);
    }

    pub fn set_view_angle(&mut self, value: i32) {
        self.view_angle = snap_view_angle(map_midi(value, TAU, 0.0));
        self.pass.set_f32("viewAngle", self.view_angle);
    }
}

/// Snap the kaleidoscope angle so TWO_PI/angle lands on whole numbers when it is close, which
/// makes it easy to dial in symmetric patterns.
pub fn snap_view_angle(view_angle: f32) -> f32 {
    let snap_range = 0.2;
    let mut x = TAU / view_angle;
    let frac = x % 1.0;
    if frac < snap_range || frac > 1.0 - snap_range {
        x = pround(x) as f32;
        return TAU / x;
    }
    view_angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_angle_snaps_to_whole_segments() {
        let a = snap_view_angle(TAU / 5.9);
        assert!((TAU / a - 6.0).abs() < 1e-4);
        let b = snap_view_angle(TAU / 5.5);
        assert!((TAU / b - 5.5).abs() < 1e-4);
    }
}
