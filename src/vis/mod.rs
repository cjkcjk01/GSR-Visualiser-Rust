//! The main visualisers (Visualisers.pde) and the overlays that share their machinery.
//!
//! Each visualiser renders into its own layer (Processing's per-visualiser `PGraphics`) which the
//! frame then composites. Shader visualisers run a GLSL file full-screen; the others draw shapes
//! with nannou's `Draw` API in Processing's coordinate system (origin top-left, y down).

pub mod beatloop;
pub mod oblivion;
pub mod shader_vis;
pub mod spectrum;
pub mod sprocket;
pub mod stick_figures;
pub mod waveform;

use crate::controls::Controls;
use crate::fft::FftAnalysis;
use crate::gpu::{Blend, DrawLayer, GpuFrame, PingPong, RenderTarget, ShaderPass};
use crate::overlays::CueInfo;
use crate::post::ShaderTime;
use crate::util::{map_midi, PNoise};
use nannou::draw::Draw;
use nannou::geom::vec3;
use nannou::wgpu;
use std::path::Path;

/// Read-only data a visualiser needs for one frame.
pub struct FrameCtx<'a> {
    pub t: ShaderTime,
    pub fft: &'a FftAnalysis,
    /// `bpm.quarterNotePulse`.
    pub pulse: f32,
    /// True when the background is white (draw dark lines instead of light ones).
    pub black_or_white: bool,
    /// Latest audio samples (Minim's `input.left` / `input.mix`).
    pub audio: &'a [f32],
    pub noise: &'a PNoise,
}

/// Buttons and faders every visualiser receives from the controller.
#[derive(Default, Clone, Debug)]
pub struct VisBase {
    pub button1: bool,
    pub button2: bool,
    pub button3: bool,
    pub fader1: i32,
    pub fader2: i32,
    pub fader3: i32,
}

pub trait Visualiser {
    fn name(&self) -> &str;
    fn base(&mut self) -> &mut VisBase;

    fn toggle_button1(&mut self) {
        let b = self.base();
        b.button1 = !b.button1;
    }
    fn set_button1(&mut self, v: bool) {
        self.base().button1 = v;
    }
    fn toggle_button2(&mut self) {
        let b = self.base();
        b.button2 = !b.button2;
    }
    fn set_button2(&mut self, v: bool) {
        self.base().button2 = v;
    }
    fn toggle_button3(&mut self) {
        let b = self.base();
        b.button3 = !b.button3;
    }
    fn set_button3(&mut self, v: bool) {
        self.base().button3 = v;
    }
    fn set_fader1(&mut self, v: i32) {
        self.base().fader1 = v;
    }
    fn set_fader2(&mut self, v: i32) {
        self.base().fader2 = v;
    }
    fn set_fader3(&mut self, v: i32) {
        self.base().fader3 = v;
    }
    fn on_beat(&mut self) {}

    /// Render this frame. Returns the layer to composite and how its alpha is stored.
    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend);
}

/// A `Draw` set up like a Processing sketch: origin top-left, y pointing down.
pub fn processing_draw(w: f32, h: f32) -> Draw {
    Draw::new()
        .translate(vec3(-w / 2.0, h / 2.0, 0.0))
        .scale_y(-1.0)
}

/// Lazily created draw layer, so visualisers that are never shown cost no GPU memory.
#[derive(Default)]
pub struct LazyDrawLayer(Option<DrawLayer>);

impl LazyDrawLayer {
    pub fn render(&mut self, g: &mut GpuFrame, draw: &Draw, label: &str) -> &RenderTarget {
        let layer = self
            .0
            .get_or_insert_with(|| DrawLayer::new(g.device, g.size, label));
        layer.render(g, draw);
        &layer.resolved
    }
}

/// A shader visualiser's pass plus the two textures it ping-pongs between.
pub struct ShaderLayer {
    pub pass: ShaderPass,
    targets: Option<PingPong>,
}

impl ShaderLayer {
    pub fn load(device: &wgpu::Device, dir: &Path, file: &str) -> anyhow::Result<Self> {
        Ok(ShaderLayer {
            pass: ShaderPass::load(device, dir, file)?,
            targets: None,
        })
    }

    pub fn render(&mut self, g: &mut GpuFrame) -> &RenderTarget {
        let targets = self
            .targets
            .get_or_insert_with(|| PingPong::new(g.device, g.size, "shader visualiser"));
        targets.apply(g, &mut self.pass);
        targets.current()
    }
}

/// The list of main visualisers. One is current; another can be cued up and switched to later.
pub struct Visualisers {
    pub list: Vec<Box<dyn Visualiser>>,
    /// Index of the cued visualiser.
    pub vis_index: usize,
    /// Index of the one being drawn.
    pub current: usize,
}

impl Visualisers {
    pub fn new(list: Vec<Box<dyn Visualiser>>) -> Self {
        Visualisers {
            list,
            vis_index: 0,
            current: 0,
        }
    }

    pub fn current_mut(&mut self) -> Option<&mut Box<dyn Visualiser>> {
        self.list.get_mut(self.current)
    }

    pub fn name(&self) -> &str {
        self.list
            .get(self.current)
            .map(|v| v.name())
            .unwrap_or("none")
    }

    fn count(&self) -> usize {
        self.list.len()
    }

    /// Switch to the cued visualiser.
    pub fn cue(&mut self, cue_info: &mut CueInfo) {
        if self.vis_index < self.count() {
            self.current = self.vis_index;
        }
        cue_info.clear();
    }

    /// Pick (cue) a visualiser with a knob.
    pub fn select_by_midi(&mut self, v: i32, cue_info: &mut CueInfo, controls: &Controls) {
        if self.count() == 0 {
            return;
        }
        let max = self.count() as i32 - 1;
        self.vis_index = (map_midi(v, 0.0, max as f32) as i32).clamp(0, max) as usize;
        cue_info.set(self.vis_index.to_string());
        controls.write_colour_for("selectVisualiser", self.vis_index, self.count());
    }

    /// Previous (1) or next (2) visualiser from the keyboard; switches straight away.
    pub fn select_by_keypress(&mut self, v: i32, cue_info: &mut CueInfo) {
        let n = self.count();
        if n == 0 {
            return;
        }
        if v == 1 {
            self.vis_index = if self.vis_index == 0 {
                n - 1
            } else {
                self.vis_index - 1
            };
        } else if v == 2 {
            self.vis_index = (self.vis_index + 1) % n;
        }
        cue_info.set(self.vis_index.to_string());
        self.cue(cue_info);
    }

    pub fn on_beat(&mut self) {
        if let Some(v) = self.current_mut() {
            v.on_beat();
        }
    }

    /// Auto-play: step to the next visualiser every N beats.
    pub fn on_beat_count(&mut self, auto_play: bool) {
        if auto_play && self.count() > 0 {
            self.vis_index = (self.vis_index + 1) % self.count();
            self.current = self.vis_index;
        }
    }
}

/// Build the visualiser list in the sketch's order. Shader visualisers whose GLSL fails to load
/// are skipped with an error in the log rather than aborting.
pub fn build_all(device: &wgpu::Device, data: &Path, w: f32, h: f32) -> Vec<Box<dyn Visualiser>> {
    use shader_vis::*;
    let dir = data.join("Shaders");
    let mut list: Vec<Box<dyn Visualiser>> = Vec::new();

    fn push<T: Visualiser + 'static>(list: &mut Vec<Box<dyn Visualiser>>, v: anyhow::Result<T>) {
        match v {
            Ok(v) => list.push(Box::new(v)),
            Err(e) => log::error!("Skipping visualiser: {e}"),
        }
    }

    push(&mut list, Ok(oblivion::VisOblivion::new(data, w, h)));
    push(&mut list, Ok(sprocket::VisSprocket::new()));
    push(&mut list, VisCandyWarp::new(device, &dir, w, h));
    push(&mut list, VisJazzUniverse::new(device, &dir, w, h));
    push(&mut list, VisCircleTunnel::new(device, &dir, w, h));
    push(&mut list, VisSpirographSpectrum::new(device, &dir, w, h));
    push(&mut list, Ok(beatloop::VisBeatLoop::new()));
    push(&mut list, Ok(spectrum::VisSpectrum::new()));
    push(&mut list, Vis1::new(device, &dir, w, h));
    push(&mut list, VisBubbles::new(device, &dir, w, h));
    push(&mut list, VisIsoHatching::new(device, &dir, w, h));
    push(&mut list, VisOpArt::new(device, &dir, w, h));
    push(&mut list, VisOrbs::new(device, &dir, w, h));
    list
}
