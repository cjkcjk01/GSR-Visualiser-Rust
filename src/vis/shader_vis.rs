//! Visualisers that are a single full-screen GLSL shader with audio-driven uniforms.

use super::{FrameCtx, ShaderLayer, VisBase, Visualiser};
use crate::gpu::{Blend, GpuFrame, RenderTarget};
use crate::util::{map, map_midi, PColor, MIDI_MAX};
use nannou::wgpu;
use std::f32::consts::{PI, TAU};
use std::path::Path;

/// Boilerplate shared by every shader visualiser.
macro_rules! shader_vis_common {
    () => {
        fn name(&self) -> &str {
            self.name
        }
        fn base(&mut self) -> &mut VisBase {
            &mut self.base
        }
    };
}

fn finish<'a>(layer: &'a mut ShaderLayer, g: &mut GpuFrame) -> (&'a RenderTarget, Blend) {
    (layer.render(g), Blend::Alpha)
}

// ---------------------------------------------------------------------------------------------
/// CandyWarp. Fader1: cycle, Fader2: warp, Button3: response type.
pub struct VisCandyWarp {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    cycle: f32,
    warp: f32,
    response_type_index: usize,
}

impl VisCandyWarp {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visCandywarp.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("thickness", 0.1);
        p.set_f32("loops", 61.0);
        p.set_f32("tint", 0.1);
        p.set_f32("rate", 1.3);
        p.set_f32("hue", 0.33);
        p.set_f32("cycle", 0.2);
        p.set_f32("warp", 2.5);
        p.set_f32("scale", 84.0);
        Ok(VisCandyWarp {
            name: "CandyWarp",
            base: VisBase::default(),
            layer,
            cycle: 0.2,
            warp: 2.5,
            response_type_index: 0,
        })
    }
}

impl Visualiser for VisCandyWarp {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.cycle = map_midi(v, 0.01, 0.4);
    }
    fn set_fader2(&mut self, v: i32) {
        self.warp = map_midi(v, -5.0, 5.0);
    }
    fn toggle_button3(&mut self) {
        self.response_type_index = (self.response_type_index + 1) % 2;
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32("warp", self.warp);
        p.set_f32("cycle", self.cycle);
        let scale = ctx.fft.choose_response_type(self.response_type_index);
        p.set_f32("scale", map(scale, 0.0, 100.0, 20.0, 50.0));
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// JazzUniverse. Fader1: colour hue, Fader2: "drie", Button3: response type.
pub struct VisJazzUniverse {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    twee: f32,
    drie: f32,
    color: PColor,
    response_type_index: usize,
}

impl VisJazzUniverse {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visJazzUniverse.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w / 2.0, h / 2.0);
        p.set_f32("thickness", 0.0);
        p.set_f32("vijf", 1.0);
        p.set_f32("twee", 1.0);
        p.set_f32("drie", 1.0);
        p.set_f32("vier", 0.0);
        p.set_vec3("color3", 0.0, 1.0, 0.1);
        Ok(VisJazzUniverse {
            name: "JazzUniverse",
            base: VisBase::default(),
            layer,
            twee: 1.0,
            drie: 1.0,
            color: PColor::hsb(1.0, 1.0, 1.0),
            response_type_index: 0,
        })
    }
}

impl Visualiser for VisJazzUniverse {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.color = PColor::hsb(map_midi(v, 0.0, 1.0), 1.0, 1.0);
    }
    fn set_fader2(&mut self, v: i32) {
        self.drie = map_midi(v, -5.0, 5.0);
    }
    fn toggle_button3(&mut self) {
        self.response_type_index = (self.response_type_index + 1) % 2;
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32("twee", self.twee);
        p.set_f32("drie", self.drie);
        let level = ctx.fft.choose_response_type(self.response_type_index);
        p.set_f32("vier", map(level, 0.0, 100.0, 0.0, 1.0));
        p.set_vec3("color3", self.color.r, self.color.g, self.color.b);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// CircleTunnel. The centre orbits, kicked on each beat. Fader1: orbit radius, Fader2: friction.
pub struct VisCircleTunnel {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    radius: f32,
    angle: f32,
    rotation_speed: f32,
    rotation_friction: f32,
    rotation_impetus: f32,
}

impl VisCircleTunnel {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visCircleTunnel.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("speed", 0.2);
        p.set_f32("circleSize", 200.0);
        p.set_f32("moveX", 0.0);
        p.set_f32("moveY", 0.0);
        p.set_vec4("pointColorA", 0.6, 0.2, 0.2, 1.0);
        p.set_vec4("pointColorB", 0.8, 0.4, 0.4, 1.0);
        Ok(VisCircleTunnel {
            name: "CircleTunnel",
            base: VisBase::default(),
            layer,
            radius: 0.1,
            angle: 0.0,
            rotation_speed: 0.0,
            rotation_friction: 0.05,
            rotation_impetus: 0.1,
        })
    }
}

impl Visualiser for VisCircleTunnel {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.radius = map_midi(v, 0.0, 1.0);
    }
    fn set_fader2(&mut self, v: i32) {
        self.rotation_friction = map_midi(v, 0.005, 1.0);
    }
    fn on_beat(&mut self) {
        self.rotation_speed += self.rotation_impetus;
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        self.angle = (self.angle + self.rotation_speed) % TAU;
        self.rotation_speed = (self.rotation_speed - self.rotation_friction).max(0.0);
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32("moveX", self.radius * self.angle.cos());
        p.set_f32("moveY", self.radius * self.angle.sin());
        let hue1 = map(ctx.fft.slow_levels[0], 0.0, 100.0, 0.0, 0.3);
        let hue2 = map(ctx.fft.slow_levels[2], 0.0, 100.0, 0.3, 0.9);
        let c1 = PColor::hsb(hue1, 1.0, 1.0);
        let c2 = PColor::hsb(hue2, 1.0, 1.0);
        p.set_vec4("pointColorA", c1.r, c1.g, c1.b, 1.0);
        p.set_vec4("pointColorB", c2.r, c2.g, c2.b, 1.0);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// SpirographSpectrum. Fader1: radius scale, Fader2: loops, Fader3: thickness,
/// Button1: step frequency, Button2: reset loops, Button3: reset thickness.
pub struct VisSpirographSpectrum {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    loops: f32,
    thickness: f32,
    freq_values: [f32; 5],
    freq_index: usize,
    radius_scale: f32,
}

impl VisSpirographSpectrum {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visSpirographSpectrum.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("freq", 12.32);
        p.set_f32("radius1", 0.1);
        p.set_f32("radius2", 0.67);
        p.set_f32("rate", 0.01);
        p.set_f32("loops", 58.93);
        p.set_f32("thickness", 0.005);
        p.set_f32("nudge", 0.875);
        p.set_f32("hue", 0.875);
        Ok(VisSpirographSpectrum {
            name: "SpirographSpectrumBeatLoop",
            base: VisBase::default(),
            layer,
            loops: 58.93,
            thickness: 0.005,
            freq_values: [0.0, 28.0, 70.0, 98.0, 127.0],
            freq_index: 3,
            radius_scale: 1.0,
        })
    }
}

impl Visualiser for VisSpirographSpectrum {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.radius_scale = map_midi(v, 0.5, 2.0);
    }
    fn set_fader2(&mut self, v: i32) {
        self.loops = map_midi(v, 1.0, 200.0);
    }
    fn set_fader3(&mut self, v: i32) {
        self.thickness = map_midi(v, 0.005, 0.1);
    }
    fn toggle_button1(&mut self) {
        self.freq_index = (self.freq_index + 1) % self.freq_values.len();
    }
    fn toggle_button2(&mut self) {
        self.loops = 58.93;
    }
    fn toggle_button3(&mut self) {
        self.thickness = 0.005;
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let rs = self.radius_scale;
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32(
            "radius1",
            map(ctx.fft.slow_levels[0], 0.0, 100.0, 0.1 * rs, 0.3 * rs),
        );
        p.set_f32(
            "radius2",
            map(ctx.fft.slow_levels[1], 0.0, 100.0, 0.5 * rs, 1.0 * rs),
        );
        p.set_f32("hue", map(ctx.fft.traktor_eq[0], 0.0, 100.0, 1.0, 10.0));
        p.set_f32("loops", self.loops);
        p.set_f32(
            "freq",
            map(
                self.freq_values[self.freq_index],
                0.0,
                MIDI_MAX as f32,
                1.0,
                16.0,
            ),
        );
        p.set_f32("thickness", self.thickness);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// Vis1 (waves). Line thickness pulses with the bass. Fader1: line count, Fader2: amplitude,
/// Fader3: colour mix.
pub struct Vis1 {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    line_count: f32,
    wave_amplitude: f32,
    line_thickness: f32,
    color_mix: f32,
    freq_index: usize,
}

impl Vis1 {
    const MIN_LINE_THICKNESS: f32 = 0.001;
    const THICKNESS_DECAY: f32 = 1.0;

    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "vis1 - waves.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("speed", 0.5);
        p.set_f32("lineCount", 8.0);
        p.set_f32("waveAmplitude", 0.3);
        p.set_f32("lineThickness", 0.05);
        p.set_f32("glowIntensity", 1.0);
        p.set_f32("colorMix", 0.5);
        p.set_vec4("color1", 0.0, 0.8, 1.0, 1.0);
        p.set_vec4("color2", 0.0, 1.0, 0.6, 1.0);
        p.set_vec4("color3", 0.9, 0.9, 0.1, 1.0);
        Ok(Vis1 {
            name: "Vis1",
            base: VisBase::default(),
            layer,
            line_count: 8.0,
            wave_amplitude: 0.3,
            line_thickness: 0.05,
            color_mix: 0.5,
            freq_index: 3,
        })
    }
}

impl Visualiser for Vis1 {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.line_count = map_midi(v, 1.0, 15.0).trunc();
    }
    fn set_fader2(&mut self, v: i32) {
        self.wave_amplitude = map_midi(v, 0.1, 1.0);
    }
    fn set_fader3(&mut self, v: i32) {
        self.color_mix = map_midi(v, 0.0, 1.0);
    }
    fn toggle_button1(&mut self) {
        self.freq_index = (self.freq_index + 1) % 5;
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        // Bass level sets the thickness, which then decays for a pulsing look.
        let level = ctx.fft.traktor_eq[0];
        if level > self.line_thickness {
            self.line_thickness = level;
        }
        self.line_thickness =
            (self.line_thickness - Self::THICKNESS_DECAY).max(Self::MIN_LINE_THICKNESS);
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32(
            "lineThickness",
            map(self.line_thickness, 0.0, 30.0, 0.001, 0.1),
        );
        p.set_f32("lineCount", self.line_count);
        p.set_f32("waveAmplitude", self.wave_amplitude);
        p.set_f32("colorMix", self.color_mix);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// Bubbles. dotSize follows the bass, rimThickness the treble. Fader1: speed, Fader2: canvas
/// rotation, Fader3: bubble count.
pub struct VisBubbles {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    iteration: f32,
    speed: f32,
    rotate_canvas: f32,
}

impl VisBubbles {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visBubbles.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("xAmp", 0.3);
        p.set_f32("yAmp", 0.1);
        p.set_f32("xFactor", 0.2);
        p.set_f32("yFactor", 0.2);
        p.set_vec2("pos", 0.5, 0.5);
        p.set_f32("dotSize", 0.02);
        p.set_f32("rimThickness", 0.15);
        Ok(VisBubbles {
            name: "Bubbles",
            base: VisBase::default(),
            layer,
            iteration: 60.0,
            speed: 0.05,
            rotate_canvas: 0.0,
        })
    }
}

impl Visualiser for VisBubbles {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.speed = map_midi(v, 0.0, 0.1);
    }
    fn set_fader2(&mut self, v: i32) {
        self.rotate_canvas = map_midi(v, -PI, PI);
    }
    fn set_fader3(&mut self, v: i32) {
        self.iteration = map_midi(v, 10.0, 100.0);
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32(
            "dotSize",
            map(ctx.fft.slow_levels[0], 0.0, 100.0, 0.01, 0.06),
        );
        p.set_f32(
            "rimThickness",
            map(ctx.fft.slow_levels[2], 0.0, 100.0, 0.05, 0.4),
        );
        p.set_f32("iteration", self.iteration);
        p.set_f32("speed", self.speed);
        p.set_f32("rotateCanvas", self.rotate_canvas);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// IsoHatching. Grid scale follows the bass, cluster size the mid+treble. Fader1: line density,
/// Fader3: randomness.
pub struct VisIsoHatching {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    line_density: f32,
    randomness: f32,
}

impl VisIsoHatching {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visIsoHatching.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("borderThickness", 0.01);
        p.set_vec4("backgroundColor", 0.95, 0.96, 0.97, 1.0);
        p.set_vec4("lineColor", 0.08, 0.12, 0.20, 1.0);
        p.set_f32("scale", 4.3);
        p.set_f32("clusterSize", 0.8);
        Ok(VisIsoHatching {
            name: "IsoHatching",
            base: VisBase::default(),
            layer,
            line_density: 18.0,
            randomness: 0.7,
        })
    }
}

impl Visualiser for VisIsoHatching {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.line_density = map_midi(v, 5.0, 50.0);
    }
    fn set_fader3(&mut self, v: i32) {
        self.randomness = map_midi(v, 0.0, 1.0);
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let s = &ctx.fft.slow_levels;
        let p = &mut self.layer.pass;
        p.set_f32("scale", map(s[0], 0.0, 100.0, 3.0, 9.0));
        p.set_f32(
            "clusterSize",
            map((s[1] + s[2]) / 2.0, 0.0, 100.0, 0.0, 2.0),
        );
        p.set_f32("lineDensity", self.line_density);
        p.set_f32("randomness", self.randomness);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// OpArt. Line spacing follows the bass; the two line colours follow treble and mid. The
/// background is transparent. Fader2: speed, Fader3: distortion.
pub struct VisOpArt {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    speed: f32,
    distortion_amount: f32,
}

impl VisOpArt {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visOpArt.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_vec4("bgColor", 0.0, 0.0, 0.0, 0.0);
        p.set_vec4("color1", 0.0, 0.0, 0.0, 1.0);
        p.set_vec4("color2", 1.0, 1.0, 1.0, 1.0);
        Ok(VisOpArt {
            name: "OpArt",
            base: VisBase::default(),
            layer,
            speed: 1.0,
            distortion_amount: 0.15,
        })
    }
}

impl Visualiser for VisOpArt {
    shader_vis_common!();
    fn set_fader2(&mut self, v: i32) {
        self.speed = map_midi(v, 0.0, 5.0);
    }
    fn set_fader3(&mut self, v: i32) {
        self.distortion_amount = map_midi(v, 0.0, 1.0);
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let s = &ctx.fft.slow_levels;
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32("lineSpacing", map(s[0], 0.0, 100.0, 40.0, 160.0));
        p.set_f32("speed", self.speed);
        p.set_f32("distortionAmount", self.distortion_amount);
        let c1 = PColor::hsb(map(s[2], 0.0, 100.0, 0.0, 1.0), 1.0, 1.0);
        let c2 = PColor::hsb(map(s[1], 0.0, 100.0, 0.0, 1.0), 1.0, 1.0);
        p.set_vec4("color1", c1.r, c1.g, c1.b, 1.0);
        p.set_vec4("color2", c2.r, c2.g, c2.b, 1.0);
        finish(&mut self.layer, g)
    }
}

// ---------------------------------------------------------------------------------------------
/// Orbs. Radius follows the bass, colour shift the mid+treble. Fader1: xMul, Fader2: yMul,
/// Fader3: contrast.
pub struct VisOrbs {
    name: &'static str,
    base: VisBase,
    layer: ShaderLayer,
    x_mul: f32,
    y_mul: f32,
    contrast: f32,
}

impl VisOrbs {
    pub fn new(device: &wgpu::Device, dir: &Path, w: f32, h: f32) -> anyhow::Result<Self> {
        let mut layer = ShaderLayer::load(device, dir, "visOrbs.glsl")?;
        let p = &mut layer.pass;
        p.set_vec2("iResolution", w, h);
        p.set_f32("zoom", 0.07);
        p.set_f32("rotation", 1.0);
        p.set_f32("sinMul", 0.0);
        p.set_f32("cosMul", 2.38);
        p.set_f32("xSpeed", 0.0);
        p.set_f32("ySpeed", 0.0);
        p.set_f32("yDivide", 4.99);
        p.set_f32("xDivide", 6.27);
        Ok(VisOrbs {
            name: "Orbs",
            base: VisBase::default(),
            layer,
            x_mul: 0.28,
            y_mul: 0.0,
            contrast: 0.13,
        })
    }
}

impl Visualiser for VisOrbs {
    shader_vis_common!();
    fn set_fader1(&mut self, v: i32) {
        self.x_mul = map_midi(v, 0.0, 1.0);
    }
    fn set_fader2(&mut self, v: i32) {
        self.y_mul = map_midi(v, 0.0, 1.0);
    }
    fn set_fader3(&mut self, v: i32) {
        self.contrast = map_midi(v, 0.08, 0.16);
    }
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let s = &ctx.fft.slow_levels;
        let p = &mut self.layer.pass;
        p.set_f32("time", ctx.t.time);
        p.set_f32("radius", map(s[0], 0.0, 100.0, 6.0, 18.0));
        p.set_f32(
            "colorShift",
            map((s[1] + s[2]) / 2.0, 0.0, 100.0, 4.0, 20.0),
        );
        p.set_f32("xMul", self.x_mul);
        p.set_f32("yMul", self.y_mul);
        p.set_f32("contrast", self.contrast);
        finish(&mut self.layer, g)
    }
}
