//! BeatLoop: a dot orbiting a circle, kicked forward on every beat.
//!
//! Button1: dot size follows the bass, Fader1: friction.

use super::{processing_draw, FrameCtx, LazyDrawLayer, VisBase, Visualiser};
use crate::gpu::{Blend, GpuFrame, RenderTarget};
use crate::util::{map, map_midi, PColor};
use nannou::color::LinSrgba;

pub struct VisBeatLoop {
    base: VisBase,
    layer: LazyDrawLayer,
    radius: f32,
    point_speed: f32,
    point_impetus: f32,
    point_friction: f32,
    angle: f32,
}

impl VisBeatLoop {
    pub fn new() -> Self {
        VisBeatLoop {
            base: VisBase::default(),
            layer: LazyDrawLayer::default(),
            radius: 200.0,
            point_speed: 0.0,
            point_impetus: 0.2,
            point_friction: 0.66,
            angle: 0.0,
        }
    }
}

impl Visualiser for VisBeatLoop {
    fn name(&self) -> &str {
        "BeatLoop"
    }
    fn base(&mut self) -> &mut VisBase {
        &mut self.base
    }
    fn set_fader1(&mut self, v: i32) {
        self.point_friction = map_midi(v, 0.5, 0.99);
    }
    fn on_beat(&mut self) {
        self.point_speed += self.point_impetus;
    }

    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let (w, h) = (ctx.t.width, ctx.t.height);
        let draw = processing_draw(w, h);
        draw.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
        let col = if ctx.black_or_white {
            PColor::gray(10.0, 255.0)
        } else {
            PColor::gray(250.0, 255.0)
        }
        .lin();
        let d = draw.x_y((w / 2.0).floor(), (h / 2.0).floor());

        d.ellipse()
            .x_y(0.0, 0.0)
            .radius(self.radius)
            .no_fill()
            .stroke(col)
            .stroke_weight(4.0);

        self.angle += self.point_speed;
        let x = self.radius * self.angle.cos();
        let y = self.radius * self.angle.sin();
        let point_radius = if self.base.button1 {
            map(ctx.fft.normalised_levels[0], 0.0, 100.0, 10.0, 50.0)
        } else {
            10.0
        };
        d.ellipse()
            .x_y(x, y)
            .radius(point_radius)
            .color(col)
            .stroke(col)
            .stroke_weight(4.0);

        self.point_speed *= self.point_friction;
        (
            self.layer.render(g, &draw, "BeatLoop"),
            Blend::Premultiplied,
        )
    }
}
