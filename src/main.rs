//! GiantSpaceRobot Traktor visualiser — Rust/nannou port of the Processing sketch.
//!
//! A visualiser for the audio input that is driven by Traktor (tempo, deck volumes, filters,
//! effects and hotcues arrive as MIDI) and by a MIDI controller and the keyboard. The JSON config
//! files, GLSL shaders, hotcue images and gradients in the sketch's `data/` directory are used
//! as-is. See README.md in this directory for setup and the list of differences.

mod audio;
mod bpm;
mod config;
mod controls;
mod decks;
mod fft;
mod gpu;
mod hotcues;
mod hud;
mod midi;
mod overlays;
mod post;
mod render;
mod shader;
mod state;
mod util;
mod video;
mod vis;

use nannou::prelude::*;
use nannou::wgpu;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Instant;

/// Command line options (beyond `--data <dir>`, handled in `config::find_data_dir`).
#[derive(Default, Debug)]
struct Options {
    /// `--set name=value`: run an action at startup (repeatable).
    set: Vec<(String, i32)>,
    /// `--demo-bpm <bpm>`: generate beats without Traktor.
    demo_bpm: Option<f32>,
    /// `--test-signal`: feed a synthetic beat into the analyser instead of the line-in.
    test_signal: bool,
    /// `--screenshot <file.png>`: save the frame after `--frames` frames and exit.
    screenshot: Option<PathBuf>,
    frames: u64,
    /// `--windowed`: ignore the fullscreen setting.
    windowed: bool,
}

fn parse_options() -> Options {
    let mut o = Options {
        frames: 60,
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--set" => {
                if let Some((k, v)) = args.next().as_deref().and_then(|s| s.split_once('=')) {
                    o.set.push((k.to_string(), v.trim().parse().unwrap_or(0)));
                }
            }
            "--demo-bpm" => o.demo_bpm = args.next().and_then(|v| v.parse().ok()),
            "--test-signal" => o.test_signal = true,
            "--screenshot" => o.screenshot = args.next().map(PathBuf::from),
            "--frames" => o.frames = args.next().and_then(|v| v.parse().ok()).unwrap_or(60),
            "--windowed" => o.windowed = true,
            "--data" => {
                args.next();
            }
            "-h" | "--help" => {
                println!(
                    "gsr_visualiser [--data <dir>] [--windowed] [--set action=value]... \
                     [--demo-bpm <bpm>] [--test-signal] [--screenshot <file.png> [--frames N]]"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    o
}

struct PendingCapture {
    buffer: wgpu::Buffer,
    size: [u32; 2],
    padded_bytes_per_row: u32,
    path: PathBuf,
}

struct Model {
    state: RefCell<state::State>,
    gpu: RefCell<render::Gpu>,
    options: Options,
    last_demo_beat: Instant,
    capture: RefCell<Option<PendingCapture>>,
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu=warn,naga=warn"),
    )
    .init();
    nannou::app(model).update(update).run();
}

fn find_font(data: &std::path::Path) -> Option<nannou::text::Font> {
    let mut files: Vec<_> = std::fs::read_dir(data)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for f in files {
        match nannou::text::font::from_file(&f) {
            Ok(font) => {
                log::info!("Using font {}", f.display());
                return Some(font);
            }
            Err(e) => log::warn!("Could not load font {}: {e}", f.display()),
        }
    }
    log::info!(
        "No .ttf/.otf font in the data directory (Processing's .vlw fonts can't be used); \
         using the built-in font"
    );
    None
}

fn model(app: &App) -> Model {
    let options = parse_options();
    let data = match config::find_data_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let cfg = match config::MainConfig::load(&data) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
    };
    log::info!("Data directory: {}", data.display());

    let s = &cfg.screen;
    let fullscreen = s.fullscreen && !options.windowed;
    let mut builder = app
        .new_window()
        .title("GiantSpaceRobot Visualiser")
        .msaa_samples(1)
        .view(view)
        .key_pressed(key_pressed);
    if fullscreen {
        let monitors = app.available_monitors();
        let monitor = monitors
            .get(s.display)
            .cloned()
            .or_else(|| app.primary_monitor());
        builder =
            builder.fullscreen_with(Some(nannou::winit::window::Fullscreen::Borderless(monitor)));
    } else {
        builder = builder
            .size(s.width, s.height)
            .resizable(s.resizable)
            .always_on_top(s.always_on_top);
    }
    let window_id = builder.build().expect("failed to create window");
    let window = app.window(window_id).unwrap();
    if !fullscreen {
        window.set_outer_position_pixels(s.location.0, s.location.1);
    }

    // Offscreen layers are the configured size (or the screen size when fullscreen), like
    // Processing's width/height; the result is scaled to the window when presenting.
    let size = if fullscreen {
        let (w, h) = window.inner_size_pixels();
        [w.max(64), h.max(64)]
    } else {
        [s.width, s.height]
    };

    let device = window.device();
    let queue = window.queue();
    let mut state = state::State::new(device, queue, data.clone(), &cfg, size);
    if options.test_signal {
        state.use_test_signal();
    }
    for (name, value) in &options.set {
        if !state.action(name, *value) {
            log::warn!("--set: unknown action '{name}'");
        }
    }
    let gpu = render::Gpu::new(device, size, find_font(&data));

    Model {
        state: RefCell::new(state),
        gpu: RefCell::new(gpu),
        options,
        last_demo_beat: Instant::now(),
        capture: RefCell::new(None),
    }
}

fn update(app: &App, model: &mut Model, _update: Update) {
    if let Some(bpm) = model.options.demo_bpm {
        let interval = 60.0 / bpm.max(1.0);
        if model.last_demo_beat.elapsed().as_secs_f32() >= interval {
            model.last_demo_beat = Instant::now();
            model.state.get_mut().action("triggerBeatActions", 127);
        }
    }
    model.state.get_mut().update();

    if let Some(c) = model.capture.get_mut().take() {
        let window = app.main_window();
        match save_capture(window.device(), &c) {
            Ok(()) => log::info!("Saved screenshot to {}", c.path.display()),
            Err(e) => log::error!("Screenshot failed: {e:#}"),
        }
        std::process::exit(0);
    }
}

fn key_pressed(_app: &App, model: &mut Model, key: Key) {
    model.state.get_mut().handle_key(key);
}

fn view(app: &App, model: &Model, frame: Frame) {
    let mut state = model.state.borrow_mut();
    let mut gpu = model.gpu.borrow_mut();
    let dq = frame.device_queue_pair().clone();
    let (device, queue) = (dq.device(), dq.queue());
    let mut encoder = frame.command_encoder();
    gpu.render(
        &mut state,
        device,
        queue,
        &mut encoder,
        frame.texture_view(),
        frame.texture_format(),
        app.fps(),
    );

    if let Some(path) = &model.options.screenshot {
        if state.frame_count == model.options.frames {
            *model.capture.borrow_mut() =
                Some(queue_capture(device, &mut encoder, gpu.scene(), path));
        }
    }
}

fn queue_capture(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target: &gpu::RenderTarget,
    path: &std::path::Path,
) -> PendingCapture {
    let [w, h] = target.size;
    let padded_bytes_per_row = (w * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot"),
        size: (padded_bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    PendingCapture {
        buffer,
        size: [w, h],
        padded_bytes_per_row,
        path: path.to_path_buf(),
    }
}

fn save_capture(device: &wgpu::Device, c: &PendingCapture) -> anyhow::Result<()> {
    let slice = c.buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.recv()??;
    let data = slice.get_mapped_range();
    let [w, h] = c.size;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let start = (row * c.padded_bytes_per_row) as usize;
        rgba.extend_from_slice(&data[start..start + (w * 4) as usize]);
    }
    for px in rgba.chunks_mut(4) {
        px[3] = 255;
    }
    nannou::image::save_buffer(&c.path, &rgba, w, h, nannou::image::ColorType::Rgba8)?;
    Ok(())
}
