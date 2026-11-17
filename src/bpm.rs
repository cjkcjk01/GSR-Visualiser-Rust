//! Beat and tempo tracking driven by Traktor's beat MIDI messages (the `BPM` class).

use crate::util::{map_midi, MIDI_MAX};

pub struct Bpm {
    pub bpm: f32,
    /// Running average, in milliseconds, of the time between beats.
    pub beat_duration: i64,
    pub bar: i32,
    pub beat: i32,
    beat_times: [i64; 4],
    current_index: usize,
    previous_time: i64,
    /// 0..1..0 pulse across a beat, smoothed with a sine. Drives bounces in a few visuals.
    pub quarter_note_pulse: f32,
    time_at_start_of_beat: i64,
    beat_count: usize,
    beat_count_values: [usize; 5],
    beat_count_index: usize,
}

/// What `on_beat` asks the rest of the app to do.
#[derive(Default, Debug, PartialEq)]
pub struct BeatEvents {
    /// Every beat: palette/word/dance/visualiser/post-shader beat actions.
    pub quarter_beat: bool,
    /// Every `beat_count_values[beat_count_index]` beats: auto-play stepping.
    pub beat_count: bool,
}

impl Bpm {
    pub fn new() -> Self {
        // In the sketch `bpm` starts at 0, so `int(60000/bpm)` is Integer.MAX_VALUE and the pulse
        // stays near zero until real beats arrive.
        let quarter_note = i32::MAX as i64;
        Bpm {
            bpm: 0.0,
            beat_duration: quarter_note / 2,
            bar: 0,
            beat: 0,
            beat_times: [0; 4],
            current_index: 0,
            previous_time: 0,
            quarter_note_pulse: 0.0,
            time_at_start_of_beat: 0,
            beat_count: 0,
            beat_count_values: [1, 4, 8, 16, 32],
            beat_count_index: 1,
        }
    }

    pub fn on_beat(&mut self, now_ms: i64) -> BeatEvents {
        self.beat_calc(now_ms);
        self.beat += 1;
        if self.beat > 3 {
            self.bar += 1;
            self.beat = 0;
        }
        self.beat_count = (self.beat_count + 1) % self.beat_count_values[self.beat_count_index];
        let events = BeatEvents {
            quarter_beat: true,
            beat_count: self.beat_count == 0,
        };
        self.reset_pulse(now_ms);
        events
    }

    fn reset_pulse(&mut self, now_ms: i64) {
        self.quarter_note_pulse = 0.0;
        self.time_at_start_of_beat = now_ms;
    }

    pub fn calc_pulse(&mut self, now_ms: i64) {
        let time_in_beat = now_ms - self.time_at_start_of_beat;
        let t = time_in_beat as f32 / self.beat_duration.max(1) as f32;
        let tri = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
        self.quarter_note_pulse = 0.5 - (std::f32::consts::PI * tri).cos() / 2.0;
    }

    fn beat_calc(&mut self, now_ms: i64) {
        let duration = now_ms - self.previous_time;
        self.previous_time = now_ms;
        self.beat_times[self.current_index] = duration;
        self.current_index = (self.current_index + 1) % self.beat_times.len();
        let sum: i64 = self.beat_times.iter().sum();
        self.beat_duration = (sum / self.beat_times.len() as i64).max(1);
        self.bpm = (60_000 / self.beat_duration) as f32;
    }

    pub fn reset_beat_counters(&mut self) {
        self.bar = 0;
        self.beat = 0;
    }

    /// Number of beats between "beat count" actions (auto-play), chosen with a knob.
    pub fn set_beat_count(&mut self, v: i32) {
        let max = (self.beat_count_values.len() - 1) as f32;
        self.beat_count_index = (map_midi(v.clamp(0, MIDI_MAX), 0.0, max) as usize).min(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_from_beats() {
        let mut b = Bpm::new();
        let mut t = 10_000;
        for _ in 0..8 {
            t += 500;
            b.on_beat(t);
        }
        assert_eq!(b.beat_duration, 500);
        assert_eq!(b.bpm, 120.0);
        b.calc_pulse(t + 250);
        assert!((b.quarter_note_pulse - 1.0).abs() < 1e-5);
    }

    #[test]
    fn beat_count_every_four() {
        let mut b = Bpm::new();
        let fired: Vec<bool> = (1..=8).map(|i| b.on_beat(i * 500).beat_count).collect();
        assert_eq!(
            fired,
            [false, false, false, true, false, false, false, true]
        );
    }
}
