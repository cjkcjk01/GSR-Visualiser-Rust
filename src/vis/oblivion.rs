//! Oblivion (after Tobias Wehrum's "Into Oblivion", MIT licence).
//!
//! Button1: keep trails (don't clear), Button2: extra dots, Button3: next colour gradient,
//! Fader1: rotation speed, Fader2: opacity, Fader3: sensitivity.

use super::{processing_draw, FrameCtx, LazyDrawLayer, VisBase, Visualiser};
use crate::gpu::{Blend, GpuFrame, RenderTarget};
use crate::hotcues::load_rgba;
use crate::util::{lerp, map_midi, pround, PColor};
use nannou::color::LinSrgba;
use nannou::geom::{vec2, vec3, Vec2};
use std::f32::consts::PI;
use std::path::Path;

const COUNT: usize = 20;

pub struct VisOblivion {
    base: VisBase,
    layer: LazyDrawLayer,
    gradients: Vec<Vec<PColor>>,
    gradient_index: usize,
    opacity: i32,
    position_radius: f32,
    previous_values: Vec<f32>,
    prev_pos: Vec<[Option<Vec2>; COUNT]>,
    rotation_angle: f32,
    rotation_speed: f32,
    scaling: f32,
    half: Vec2,
}

/// Each PNG in `data/gradients` becomes a palette: the colours along its top row.
fn load_gradients(data: &Path) -> Vec<Vec<PColor>> {
    let mut files: Vec<_> = std::fs::read_dir(data.join("gradients"))
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.sort();
    let mut out = Vec::new();
    for f in files {
        if !f
            .extension()
            .map(|e| e.eq_ignore_ascii_case("png"))
            .unwrap_or(false)
        {
            continue;
        }
        match load_rgba(&f) {
            Ok(([w, _], px)) => out.push(
                (0..w as usize)
                    .map(|x| PColor::rgba8(px[x * 4], px[x * 4 + 1], px[x * 4 + 2], 255))
                    .collect(),
            ),
            Err(e) => log::warn!("Could not load gradient {}: {e}", f.display()),
        }
    }
    if out.is_empty() {
        log::warn!("No gradients found in data/gradients, using white");
        out.push(vec![PColor::rgba8(255, 255, 255, 255)]);
    }
    out
}

impl VisOblivion {
    pub fn new(data: &Path, w: f32, h: f32) -> Self {
        // fftAnalysis.specSize / 10 with a 1024 sample buffer
        let n = (crate::audio::BUFFER_SIZE / 2 + 1) / 10;
        VisOblivion {
            base: VisBase::default(),
            layer: LazyDrawLayer::default(),
            gradients: load_gradients(data),
            gradient_index: 0,
            opacity: 100,
            position_radius: (h * 0.3) * 1.35,
            previous_values: vec![0.0; n],
            prev_pos: vec![[None; COUNT]; n],
            rotation_angle: 0.0,
            rotation_speed: 0.0,
            scaling: 5.0,
            half: vec2(w / 2.0, h / 2.0),
        }
    }

    fn calculate_fft_values(&mut self, ctx: &FrameCtx) {
        let size = 10;
        let spec = ctx.fft.spec_size;
        let mut n = 0;
        while n + size < spec {
            // The sketch computes this with integer division, so it is always 0 and the lerp
            // below is always 4. Kept as-is so the look matches.
            let percent = (n / (spec - size)) as f32;
            let mut avg: f32 = (n..n + size).map(|i| ctx.fft.fft.get_band(i)).sum();
            avg = avg * lerp(4.0, 8.0, percent) * self.scaling / size as f32;
            let idx = n / size;
            if let Some(prev) = self.previous_values.get_mut(idx) {
                *prev = (*prev * 0.9).max(avg);
            }
            n += size;
        }
    }
}

impl Visualiser for VisOblivion {
    fn name(&self) -> &str {
        "Oblivion"
    }
    fn base(&mut self) -> &mut VisBase {
        &mut self.base
    }
    fn set_fader1(&mut self, v: i32) {
        self.rotation_speed = map_midi(v, -0.05, 0.05);
        if self.rotation_speed > -0.01 && self.rotation_speed < 0.01 {
            self.rotation_speed = 0.0;
        }
    }
    fn set_fader2(&mut self, v: i32) {
        self.opacity = pround(map_midi(v, 0.0, 255.0));
    }
    fn set_fader3(&mut self, v: i32) {
        self.scaling = map_midi(v, 0.0, 20.0);
    }
    fn toggle_button3(&mut self) {
        self.gradient_index = (self.gradient_index + 1) % self.gradients.len();
    }

    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        self.calculate_fft_values(ctx);

        let draw = processing_draw(ctx.t.width, ctx.t.height);
        if !self.base.button1 {
            draw.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
        }

        let half = self.half;
        self.rotation_angle += self.rotation_speed;
        let d = draw
            .translate(vec3(half.x, half.y, 0.0))
            .rotate(self.rotation_angle)
            .translate(vec3(-half.x, -half.y, 0.0));

        let gradient = &self.gradients[self.gradient_index];
        let len = self.previous_values.len();
        for i in 0..len {
            let start_angle = i as f32 * PI / 100.0;
            let delta_angle = PI * 2.0 / COUNT as f32;
            let value = self.previous_values[i];
            let percent = i as f32 / len as f32;
            let col = gradient
                [((gradient.len() as f32 * percent) as usize).min(gradient.len() - 1)]
            .with_alpha(self.opacity as f32)
            .lin();

            let s = (value * 0.5 * self.position_radius / 360.0).max(2.0);
            let distance = (self.position_radius - percent * self.position_radius * value / 40.0)
                .max(-self.position_radius);

            for j in 0..COUNT {
                let mut a = start_angle + delta_angle * j as f32;
                if j % 2 == 0 {
                    a -= start_angle * 2.0;
                }
                let cur = vec2(half.x + a.cos() * distance, half.y + a.sin() * distance);

                if self.base.button2 {
                    d.ellipse()
                        .x_y(cur.x, cur.y)
                        .w_h(s / 2.0, s / 2.0)
                        .color(col);
                }

                if let Some(prev) = self.prev_pos[i][j] {
                    let delta = prev - cur;
                    let dist = delta.length();
                    d.translate(vec3(cur.x, cur.y, 0.0))
                        .rotate(delta.y.atan2(delta.x))
                        .rect()
                        .x_y(dist / 2.0, 0.0)
                        .w_h(dist, s)
                        .color(col);
                }
                self.prev_pos[i][j] = Some(cur);
            }
        }

        (
            self.layer.render(g, &draw, "Oblivion"),
            Blend::Premultiplied,
        )
    }
}
