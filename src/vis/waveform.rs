//! Oscilloscope-style waveform overlay.

use super::FrameCtx;
use crate::util::{map_midi, pround, PColor};
use nannou::draw::Draw;
use nannou::geom::vec2;

pub struct VisWaveform {
    pub scale: i32,
}

impl VisWaveform {
    pub fn new() -> Self {
        VisWaveform { scale: 100 }
    }

    pub fn set_scale(&mut self, v: i32) {
        self.scale = pround(map_midi(v, 100.0, 1000.0));
    }

    /// Draw onto a Processing-oriented `Draw`.
    pub fn draw(&self, draw: &Draw, ctx: &FrameCtx) {
        let n = ctx.audio.len();
        if n < 2 {
            return;
        }
        let col = if ctx.black_or_white {
            PColor::gray(10.0, 255.0)
        } else {
            PColor::gray(250.0, 255.0)
        };
        let dist = ctx.t.width / n as f32;
        let mid = (ctx.t.height / 2.0).floor();
        let scale = self.scale as f32;
        draw.polyline()
            .weight(2.0)
            .points(
                ctx.audio
                    .iter()
                    .enumerate()
                    .map(|(i, s)| vec2(dist * i as f32, mid + s * scale)),
            )
            .color(col.lin());
    }
}
