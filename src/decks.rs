//! Per-deck state from Traktor (volume, play state, filter, hotcue pack) and the mixer summary.

use crate::util::{map, map_midi, MIDI_MAX};

pub const NUM_OF_PACKS: usize = 8;
/// Fader level above which a deck is considered loud enough to trigger visual effects.
pub const FADER_THRESHOLD: i32 = 25;

#[derive(Clone, Debug)]
pub struct DeckSettings {
    pub name: &'static str,
    pub hotcue_pack: usize,
    pub hotcue_index: usize,
    pub fader_value: i32,
    pub filter_value: f32,
    pub blur_on: bool,
    pub playing: bool,
}

impl DeckSettings {
    pub fn new(name: &'static str) -> Self {
        DeckSettings {
            name,
            hotcue_pack: 0,
            hotcue_index: 0,
            fader_value: 0,
            filter_value: 0.0,
            blur_on: false,
            playing: false,
        }
    }

    /// Map a "set hotcue pack" CC to a pack number.
    pub fn pack_from_value(value: i32) -> usize {
        map_midi(value, 0.0, (NUM_OF_PACKS - 1) as f32) as usize
    }

    /// Which of the eight hotcues a Traktor hotcue CC value (15, 31, ... 127) refers to.
    pub fn hotcue_from_value(value: i32) -> usize {
        map(
            value as f32,
            0.0,
            MIDI_MAX as f32,
            0.0,
            (NUM_OF_PACKS - 1) as f32,
        ) as usize
    }

    pub fn set_volume(&mut self, v: i32) {
        self.fader_value = v;
        self.update_blur();
    }

    pub fn set_filter(&mut self, v: i32) {
        self.filter_value = map(v as f32, 1.0, MIDI_MAX as f32, -8.0, 8.0);
        if self.filter_value > -0.15 && self.filter_value < 0.15 {
            self.filter_value = 0.0;
        }
        self.update_blur();
    }

    /// The blur is shown when the deck is playing, audible and its filter is engaged.
    fn update_blur(&mut self) {
        self.blur_on =
            self.playing && self.fader_value > FADER_THRESHOLD && self.filter_value.abs() > 0.0;
    }

    pub fn loud_enough(&self) -> bool {
        self.fader_value > FADER_THRESHOLD
    }
}

/// `MixerState.checkForBlurOn()`: 0 when no deck's blur is on, otherwise the strongest filter.
pub fn filter_intensity(decks: &[DeckSettings; 4]) -> f32 {
    if decks.iter().any(|d| d.blur_on) {
        decks
            .iter()
            .map(|d| d.filter_value.abs())
            .fold(0.0, f32::max)
    } else {
        0.0
    }
}

pub fn any_deck_playing(decks: &[DeckSettings; 4]) -> bool {
    decks.iter().any(|d| d.playing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotcue_values_map_to_eight_cues() {
        let cues: Vec<usize> = [15, 31, 47, 63, 79, 95, 111, 127]
            .iter()
            .map(|v| DeckSettings::hotcue_from_value(*v))
            .collect();
        assert_eq!(cues, [0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn blur_needs_play_volume_and_filter() {
        let mut d = DeckSettings::new("A");
        d.playing = true;
        d.set_volume(100);
        d.set_filter(64);
        assert!(!d.blur_on, "centre filter position is a dead zone");
        d.set_filter(127);
        assert!(d.blur_on);
        let decks = [
            d.clone(),
            DeckSettings::new("B"),
            DeckSettings::new("C"),
            DeckSettings::new("D"),
        ];
        assert_eq!(filter_intensity(&decks), 8.0);
    }
}
