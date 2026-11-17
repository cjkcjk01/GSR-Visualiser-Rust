//! Hotcue image packs: `data/hotcue_pack<N>/` folders of eight PNGs, flashed full screen when a
//! hotcue is pressed in Traktor.

use crate::gpu::RenderTarget;
use nannou::wgpu;
use std::path::{Path, PathBuf};

pub struct HotcuePacks {
    packs: Vec<Vec<RenderTarget>>,
    frames: i32,
    frame_count: i32,
    ready: bool,
    current_pack: usize,
    current_hotcue: usize,
    alpha: i32,
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn is_png(p: &Path) -> bool {
    p.extension()
        .map(|e| e.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}

/// Load an image as raw RGBA8 (no colour management, like Processing).
pub fn load_rgba(path: &Path) -> anyhow::Result<([u32; 2], Vec<u8>)> {
    let img = nannou::image::open(path)?.to_rgba8();
    Ok(([img.width(), img.height()], img.into_raw()))
}

impl HotcuePacks {
    pub fn load(device: &wgpu::Device, queue: &wgpu::Queue, data: &Path) -> Self {
        let mut packs = Vec::new();
        let dirs: Vec<PathBuf> = sorted_entries(data)
            .into_iter()
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("hotcue"))
                        .unwrap_or(false)
            })
            .collect();
        if dirs.is_empty() {
            log::warn!(
                "No hotcue directory found. Make sure that you have at least one directory, \
                 inside the data directory, named \"hotcue_pack<x>\""
            );
        }
        for dir in dirs {
            let mut images = Vec::new();
            for file in sorted_entries(&dir)
                .into_iter()
                .filter(|p| is_png(p))
                .take(8)
            {
                match load_rgba(&file) {
                    Ok((size, rgba)) => images.push(RenderTarget::from_rgba(
                        device, queue, size, &rgba, "hotcue",
                    )),
                    Err(e) => log::warn!("Could not load {}: {e}", file.display()),
                }
            }
            if images.len() < 8 {
                log::warn!("Warning - Less than 8 images found in {}", dir.display());
            } else {
                packs.push(images);
            }
        }
        log::info!("Loaded {} hotcue packs", packs.len());
        HotcuePacks {
            packs,
            frames: 15,
            frame_count: 0,
            ready: false,
            current_pack: 0,
            current_hotcue: 0,
            alpha: 255,
        }
    }

    pub fn len(&self) -> usize {
        self.packs.len()
    }

    /// Show hotcue `hc` of pack `pack` for `frames` frames.
    pub fn ready_hotcue(&mut self, pack: usize, hc: usize, frames: i32) {
        if self.packs.get(pack).and_then(|p| p.get(hc)).is_none() {
            return;
        }
        self.current_pack = pack;
        self.current_hotcue = hc;
        self.frames = frames.max(1);
        self.frame_count = 0;
        self.alpha = 255;
        self.ready = true;
    }

    /// Advance the fade and return the image to draw this frame with its alpha (0..1).
    pub fn frame(&mut self) -> Option<(&RenderTarget, f32)> {
        if !self.ready {
            return None;
        }
        // Same integer fade as the sketch: each frame loses alpha/frames, so it eases out.
        let alpha_dec = self.alpha / self.frames;
        let alpha = self.alpha.max(0) as f32 / 255.0;
        self.frame_count += 1;
        if self.frame_count > self.frames {
            self.frame_count = 0;
            self.ready = false;
        }
        self.alpha -= alpha_dec;
        let img = &self.packs[self.current_pack][self.current_hotcue];
        Some((img, alpha))
    }
}
