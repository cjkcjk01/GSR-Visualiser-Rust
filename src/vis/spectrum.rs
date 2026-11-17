//! Classic bar spectrum analyser with log-spaced bands, fast attack / slow decay and peak caps.
//!
//! Button1: peak-hold caps, Button2: VU colours (green to red) instead of a hue ramp,
//! Button3: mirror around the centre, Fader1: sensitivity, Fader2: decay, Fader3: hue offset.

use super::{processing_draw, FrameCtx, LazyDrawLayer, VisBase, Visualiser};
use crate::gpu::{Blend, GpuFrame, RenderTarget};
use crate::util::{constrain, map, map_midi, PColor};
use nannou::color::LinSrgba;

const NUM_BARS: usize = 24;
const MAX_FREQ: f32 = 4000.0;
const NOISE_FLOOR: f32 = 0.15;
const MAX_DECAY: f32 = 0.999;
const PEAK_DECAY_RATE: f32 = 0.05;

pub struct VisSpectrum {
    base: VisBase,
    layer: LazyDrawLayer,
    band_low: [f32; NUM_BARS],
    band_high: [f32; NUM_BARS],
    bar_levels: [f32; NUM_BARS],
    peak_levels: [f32; NUM_BARS],
    running_max: [f32; NUM_BARS],
    sensitivity: f32,
    decay_rate: f32,
    hue_offset: f32,
    peak_hold_on: bool,
    vu_colour_mode: bool,
    mirror_on: bool,
}

impl VisSpectrum {
    pub fn new() -> Self {
        let lowest = 20.0;
        let mut band_low = [0.0; NUM_BARS];
        let mut band_high = [0.0; NUM_BARS];
        for i in 0..NUM_BARS {
            band_low[i] = lowest * (MAX_FREQ / lowest).powf(i as f32 / NUM_BARS as f32);
            band_high[i] = lowest * (MAX_FREQ / lowest).powf((i + 1) as f32 / NUM_BARS as f32);
        }
        VisSpectrum {
            base: VisBase::default(),
            layer: LazyDrawLayer::default(),
            band_low,
            band_high,
            bar_levels: [0.0; NUM_BARS],
            peak_levels: [0.0; NUM_BARS],
            running_max: [NOISE_FLOOR; NUM_BARS],
            sensitivity: 1.0,
            decay_rate: 0.04,
            hue_offset: 200.0,
            peak_hold_on: false,
            vu_colour_mode: false,
            mirror_on: true,
        }
    }

    fn update_bands(&mut self, ctx: &FrameCtx) {
        for i in 0..NUM_BARS {
            let raw = ctx
                .fft
                .fft
                .calc_avg(self.band_low[i], self.band_high[i])
                .max(NOISE_FLOOR);
            // Auto gain: track the loudest level per band, relaxing slowly.
            if raw > self.running_max[i] {
                self.running_max[i] = raw;
            } else {
                self.running_max[i] = (self.running_max[i] * MAX_DECAY).max(NOISE_FLOOR);
            }
            let normalised = constrain(raw / self.running_max[i] * self.sensitivity, 0.0, 1.0);
            if normalised > self.bar_levels[i] {
                self.bar_levels[i] = normalised;
            } else {
                self.bar_levels[i] = (self.bar_levels[i] - self.decay_rate).max(0.0);
            }
            if self.bar_levels[i] >= self.peak_levels[i] {
                self.peak_levels[i] = self.bar_levels[i];
            } else {
                self.peak_levels[i] = (self.peak_levels[i] - PEAK_DECAY_RATE).max(0.0);
            }
        }
    }

    fn bar_colour(&self, i: usize, level: f32) -> LinSrgba {
        if self.vu_colour_mode {
            let hue = map(level, 0.0, 1.0, 120.0, 0.0);
            PColor::hsb(hue / 360.0, 0.9, 0.9).lin()
        } else {
            let hue = (map(i as f32, 0.0, NUM_BARS as f32, 0.0, 300.0) + self.hue_offset) % 360.0;
            PColor::hsb(hue / 360.0, 0.85, 0.9).lin()
        }
    }
}

impl Visualiser for VisSpectrum {
    fn name(&self) -> &str {
        "Spectrum"
    }
    fn base(&mut self) -> &mut VisBase {
        &mut self.base
    }
    fn set_fader1(&mut self, v: i32) {
        self.sensitivity = map_midi(v, 0.3, 3.0);
    }
    fn set_fader2(&mut self, v: i32) {
        self.decay_rate = map_midi(v, 0.01, 0.15);
    }
    fn set_fader3(&mut self, v: i32) {
        self.hue_offset = map_midi(v, 0.0, 360.0);
    }
    fn toggle_button1(&mut self) {
        self.peak_hold_on = !self.peak_hold_on;
    }
    fn toggle_button2(&mut self) {
        self.vu_colour_mode = !self.vu_colour_mode;
    }
    fn toggle_button3(&mut self) {
        self.mirror_on = !self.mirror_on;
    }

    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        self.update_bands(ctx);
        let (w, h) = (ctx.t.width, ctx.t.height);
        let half_h = (h / 2.0).floor();
        let draw = processing_draw(w, h);
        draw.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));

        let bar_w = w / NUM_BARS as f32;
        let gap = bar_w * 0.15;
        let max_h = if self.mirror_on { half_h } else { h };
        // corner-mode rect helper
        let rect = |x: f32, y: f32, rw: f32, rh: f32, c: LinSrgba| {
            draw.rect()
                .x_y(x + rw / 2.0, y + rh / 2.0)
                .w_h(rw, rh)
                .color(c);
        };

        for i in 0..NUM_BARS {
            let x = i as f32 * bar_w + gap / 2.0;
            let bw = bar_w - gap;
            let bar_h = self.bar_levels[i] * max_h;
            let peak_h = self.peak_levels[i] * max_h;
            let c = self.bar_colour(i, self.bar_levels[i]);
            if self.mirror_on {
                rect(x, half_h - bar_h, bw, bar_h, c);
                rect(x, half_h, bw, bar_h, c);
            } else {
                rect(x, h - bar_h, bw, bar_h, c);
            }
            if self.peak_hold_on {
                let pc = self.bar_colour(i, self.peak_levels[i]);
                if self.mirror_on {
                    rect(x, half_h - peak_h - 2.0, bw, 2.0, pc);
                    rect(x, half_h + peak_h, bw, 2.0, pc);
                } else {
                    rect(x, h - peak_h - 2.0, bw, 2.0, pc);
                }
            }
        }
        (
            self.layer.render(g, &draw, "Spectrum"),
            Blend::Premultiplied,
        )
    }
}
