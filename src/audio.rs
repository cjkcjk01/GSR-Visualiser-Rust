//! Line-in capture (replaces Minim's `getLineIn(Minim.MONO, 1024, 48000, 16)`).
//!
//! A cpal input stream mixes every channel down to mono and keeps the most recent `BUFFER_SIZE`
//! samples in a ring. If no input device is available the visualiser keeps running on silence.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub const BUFFER_SIZE: usize = 1024;
pub const PREFERRED_SAMPLE_RATE: u32 = 48_000;

struct Ring {
    data: Vec<f32>,
    pos: usize,
}

pub struct AudioInput {
    ring: Arc<Mutex<Ring>>,
    sample_rate: u32,
    _stream: Option<cpal::Stream>,
    synthetic: Option<std::time::Instant>,
}

impl AudioInput {
    pub fn new() -> Self {
        let ring = Arc::new(Mutex::new(Ring {
            data: vec![0.0; BUFFER_SIZE],
            pos: 0,
        }));
        match Self::open(ring.clone()) {
            Ok((stream, rate)) => {
                log::info!("Audio input open at {rate} Hz");
                AudioInput {
                    ring,
                    sample_rate: rate,
                    _stream: Some(stream),
                    synthetic: None,
                }
            }
            Err(e) => {
                log::warn!("No audio input available ({e}); running on silence");
                AudioInput {
                    ring,
                    sample_rate: PREFERRED_SAMPLE_RATE,
                    _stream: None,
                    synthetic: None,
                }
            }
        }
    }

    fn open(ring: Arc<Mutex<Ring>>) -> anyhow::Result<(cpal::Stream, u32)> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no default input device"))?;
        log::info!(
            "Audio input device: {}",
            device.name().unwrap_or_else(|_| "?".into())
        );

        // Prefer an f32 config at 48 kHz, otherwise take the device default.
        let config = device
            .supported_input_configs()?
            .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
            .find(|c| {
                c.min_sample_rate().0 <= PREFERRED_SAMPLE_RATE
                    && c.max_sample_rate().0 >= PREFERRED_SAMPLE_RATE
            })
            .map(|c| c.with_sample_rate(cpal::SampleRate(PREFERRED_SAMPLE_RATE)))
            .map_or_else(|| device.default_input_config(), Ok)?;

        let channels = config.channels() as usize;
        let rate = config.sample_rate().0;
        let err_fn = |e| log::error!("audio stream error: {e}");

        fn push(ring: &Mutex<Ring>, mono: impl Iterator<Item = f32>) {
            if let Ok(mut r) = ring.lock() {
                for s in mono {
                    let p = r.pos;
                    r.data[p] = s;
                    r.pos = (p + 1) % BUFFER_SIZE;
                }
            }
        }

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |d: &[f32], _: &_| {
                    push(
                        &ring,
                        d.chunks(channels)
                            .map(|f| f.iter().sum::<f32>() / channels as f32),
                    )
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |d: &[i16], _: &_| {
                    push(
                        &ring,
                        d.chunks(channels).map(|f| {
                            f.iter().map(|s| *s as f32 / 32768.0).sum::<f32>() / channels as f32
                        }),
                    )
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |d: &[u16], _: &_| {
                    push(
                        &ring,
                        d.chunks(channels).map(|f| {
                            f.iter()
                                .map(|s| (*s as f32 - 32768.0) / 32768.0)
                                .sum::<f32>()
                                / channels as f32
                        }),
                    )
                },
                err_fn,
                None,
            )?,
            other => anyhow::bail!("unsupported sample format {other:?}"),
        };
        stream.play()?;
        Ok((stream, rate))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Replace the line-in with a synthetic 120 BPM kick + bass + hi-hat pattern (for testing
    /// without an audio interface).
    pub fn use_test_signal(&mut self) {
        self._stream = None;
        self.synthetic = Some(std::time::Instant::now());
        self.sample_rate = PREFERRED_SAMPLE_RATE;
    }

    fn synth(start: std::time::Instant, rate: u32, out: &mut Vec<f32>) {
        use std::f32::consts::TAU;
        let now = start.elapsed().as_secs_f32();
        let rate = rate as f32;
        for i in 0..BUFFER_SIZE {
            let t = now - (BUFFER_SIZE - i) as f32 / rate;
            let beat = t.rem_euclid(0.5);
            let kick = (TAU * 55.0 * beat).sin() * (-beat * 12.0).exp();
            let bass = 0.3 * (TAU * 110.0 * t).sin() * (0.5 + 0.5 * (TAU * 0.5 * t).sin());
            let off = (t + 0.25).rem_euclid(0.5);
            let hat_noise = ((t * 92821.7).sin() * 43758.547).fract() * 2.0 - 1.0;
            let hat = 0.3 * hat_noise * (-off * 40.0).exp();
            let lead = 0.2 * (TAU * 880.0 * t).sin() * (0.5 + 0.5 * (TAU * 0.25 * t).sin());
            out.push((kick + bass + hat + lead) * 0.6);
        }
    }

    /// Copy the latest `BUFFER_SIZE` samples, oldest first (Minim's `input.mix`).
    pub fn read(&self, out: &mut Vec<f32>) {
        out.clear();
        if let Some(start) = self.synthetic {
            Self::synth(start, self.sample_rate, out);
            return;
        }
        if let Ok(r) = self.ring.lock() {
            out.extend_from_slice(&r.data[r.pos..]);
            out.extend_from_slice(&r.data[..r.pos]);
        } else {
            out.resize(BUFFER_SIZE, 0.0);
        }
    }
}
