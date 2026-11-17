//! The per-frame render, in the same order as the sketch's `draw()`:
//!
//! 1. background colour
//! 2. visualiser, hotcue image and waveform, composited into one layer that the kaleidoscope
//!    (when on) is applied to — in the sketch `shader(kaleidoscope)` affected exactly these
//! 3. beat words and stick figures
//! 4. post-processing shaders, then the Traktor effect shaders (echo, gate/slice, filter blur)
//! 5. info / FPS / help / cue-info text
//! 6. present to the window

use crate::gpu::{Blend, Blitter, DrawLayer, GpuFrame, PingPong, RenderTarget, LAYER_FORMAT};
use crate::hud::{self, Hud};
use crate::state::State;
use crate::vis::{processing_draw, FrameCtx};
use nannou::color::LinSrgba;
use nannou::draw::{self, Draw};
use nannou::geom::vec3;
use nannou::text::Font;
use nannou::wgpu;

pub struct Gpu {
    pub size: [u32; 2],
    draw_renderer: draw::Renderer,
    blitter: Blitter,
    scene: PingPong,
    kaleido_layer: RenderTarget,
    overlay: DrawLayer,
    font: Option<Font>,
}

const TRANSPARENT: [f64; 4] = [0.0, 0.0, 0.0, 0.0];

impl Gpu {
    pub fn new(device: &wgpu::Device, size: [u32; 2], font: Option<Font>) -> Self {
        let draw_renderer = draw::RendererBuilder::new()
            .glyph_cache_size([2048, 2048])
            .build(
                device,
                size,
                1.0,
                crate::gpu::DRAW_MSAA_SAMPLES,
                LAYER_FORMAT,
            );
        Gpu {
            size,
            draw_renderer,
            blitter: Blitter::new(device),
            scene: PingPong::new(device, size, "scene"),
            kaleido_layer: RenderTarget::new(device, size, "kaleidoscope layer"),
            overlay: DrawLayer::new(device, size, "overlay"),
            font,
        }
    }

    /// Render one frame of `state` into the window's frame texture.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        state: &mut State,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        output_format: wgpu::TextureFormat,
        fps: f32,
    ) {
        state.frame_count += 1;
        let (w, h) = (state.width, state.height);
        let t = state.shader_time();
        self.blitter.begin_frame();
        let mut g = GpuFrame {
            device,
            queue,
            encoder,
            draw_renderer: &mut self.draw_renderer,
            blitter: &mut self.blitter,
            size: self.size,
        };

        let ctx = FrameCtx {
            t,
            fft: &state.fft,
            pulse: state.bpm.quarter_note_pulse,
            black_or_white: state.palette.black_or_white(),
            audio: &state.audio_buf,
            noise: &state.noise,
        };

        // 1. background
        let bg = state.palette.background();
        self.scene
            .current()
            .clear(g.encoder, [bg.r as f64, bg.g as f64, bg.b as f64, 1.0]);

        // 2. visualiser + hotcue + waveform into the kaleidoscope layer
        self.kaleido_layer.clear(g.encoder, TRANSPARENT);
        // (live camera would be drawn here when `state.live.available() && state.video_on`)
        if let Some(v) = state.visualisers.list.get_mut(state.visualisers.current) {
            let (layer, blend) = v.render(&mut g, &ctx);
            g.blit(&layer.view, &self.kaleido_layer.view, [1.0; 4], blend);
        }
        if let Some((img, alpha)) = state.hotcues.frame() {
            g.blit(
                &img.view,
                &self.kaleido_layer.view,
                [1.0, 1.0, 1.0, alpha],
                Blend::Alpha,
            );
        }
        if state.waveform_on {
            let draw = processing_draw(w, h);
            draw.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
            state.waveform.draw(&draw, &ctx);
            self.overlay.render(&mut g, &draw);
            g.blit(
                &self.overlay.resolved.view,
                &self.kaleido_layer.view,
                [1.0; 4],
                Blend::Premultiplied,
            );
        }
        match (&mut state.kaleidoscope, state.kaleidoscope_on) {
            (Some(k), true) => {
                let dst = self.scene.current();
                k.pass.encode(
                    &mut g,
                    Some(&self.kaleido_layer),
                    &dst.view,
                    dst.size,
                    Blend::Premultiplied,
                );
            }
            _ => g.blit(
                &self.kaleido_layer.view,
                &self.scene.current().view,
                [1.0; 4],
                Blend::Premultiplied,
            ),
        }

        // 3. beat words + stick figures
        if state.words.on || state.stick_figures.on {
            let base = Draw::new();
            base.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
            let hud = Hud {
                draw: &base,
                w,
                h,
                font: self.font.as_ref(),
            };
            hud::draw_words(&hud, state);
            let pd = base.translate(vec3(-w / 2.0, h / 2.0, 0.0)).scale_y(-1.0);
            state.stick_figures.draw(&pd, &ctx);
            self.overlay.render(&mut g, &base);
            g.blit(
                &self.overlay.resolved.view,
                &self.scene.current().view,
                [1.0; 4],
                Blend::Premultiplied,
            );
        }

        // 4. post processing
        if state.post.on {
            if let Some(s) = state.post.current_mut() {
                s.prepare(t);
                self.scene.apply(&mut g, &mut s.pass);
            }
        }
        if state.delay_on {
            if let Some(s) = &mut state.sobel {
                s.prepare(t);
                self.scene.apply(&mut g, &mut s.pass);
            }
        }
        if state.slice_on {
            if let Some(s) = &mut state.vhs_glitch {
                s.prepare(t);
                self.scene.apply(&mut g, &mut s.pass);
            }
        }
        if state.filter_intensity > 0.0 {
            if let Some(b) = &mut state.blur {
                b.pass.set_f32("blurDegree", state.filter_intensity);
                self.scene.apply(&mut g, &mut b.pass);
            }
        }

        // 5. text overlays
        if state.info_on || state.fps_on || state.help_on || !state.cue_info.text.is_empty() {
            let base = Draw::new();
            base.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
            let hud = Hud {
                draw: &base,
                w,
                h,
                font: self.font.as_ref(),
            };
            if state.info_on {
                hud::draw_info(&hud, state);
            }
            if state.fps_on {
                hud::draw_fps(&hud, fps);
            }
            if state.help_on {
                hud::draw_help(&hud);
            }
            hud::draw_cue_info(&hud, state);
            self.overlay.render(&mut g, &base);
            g.blit(
                &self.overlay.resolved.view,
                &self.scene.current().view,
                [1.0; 4],
                Blend::Premultiplied,
            );
        }

        // 6. present. Our layers hold raw (gamma-encoded) values; a linear or sRGB-encoding
        // output needs them decoded first so they come out looking the same.
        let decode = !matches!(
            output_format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
        );
        g.blitter.blit(
            device,
            queue,
            g.encoder,
            &self.scene.current().view,
            output,
            output_format,
            [1.0; 4],
            Blend::Replace,
            decode,
        );
    }

    /// The final scene texture (for screenshots).
    pub fn scene(&self) -> &RenderTarget {
        self.scene.current()
    }
}
