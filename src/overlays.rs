//! Simple overlay state: beat words, the background palette and the cue-info box.

use crate::util::{map_midi, pround, PColor};

/// A list of words shown one per beat.
#[derive(Clone, Debug)]
pub struct WordPack {
    pub words: Vec<String>,
    pub current: usize,
}

impl WordPack {
    pub fn cue_word(&mut self) {
        self.current = (self.current + 1) % self.words.len().max(1);
    }
    pub fn current_word(&self) -> &str {
        self.words
            .get(self.current)
            .map(String::as_str)
            .unwrap_or("")
    }
}

pub struct WordPacks {
    pub packs: Vec<WordPack>,
    /// The pack in use.
    pub pack_index: usize,
    /// The pack that will be used next time it is cued.
    pub cue_index: usize,
    pub on: bool,
    pub alpha: i32,
}

impl WordPacks {
    pub fn new(packs: Vec<Vec<String>>) -> Self {
        let mut packs: Vec<WordPack> = packs
            .into_iter()
            .map(|words| WordPack { words, current: 0 })
            .collect();
        if packs.is_empty() {
            packs.push(WordPack {
                words: vec![String::new()],
                current: 0,
            });
        }
        WordPacks {
            packs,
            pack_index: 0,
            cue_index: 0,
            on: false,
            alpha: 150,
        }
    }

    /// Select (but don't yet use) a pack; returns its first word for the cue box.
    pub fn select(&mut self, v: i32) -> String {
        let max = (self.packs.len() - 1) as f32;
        self.cue_index = (map_midi(v, 0.0, max) as usize).min(self.packs.len() - 1);
        self.packs[self.cue_index]
            .words
            .first()
            .cloned()
            .unwrap_or_default()
    }

    pub fn cue(&mut self) {
        self.pack_index = self.cue_index;
        self.packs[self.pack_index].current = 0;
    }

    pub fn next_word(&mut self) {
        self.packs[self.pack_index].cue_word();
    }

    pub fn set_alpha(&mut self, a: i32) {
        self.alpha = pround(map_midi(a, 0.0, 255.0));
    }

    pub fn current_word(&self) -> &str {
        self.packs[self.pack_index].current_word()
    }
}

/// Background colour: black, white, or cycling through a palette on the beat.
pub struct BgPalette {
    pub palettes: Vec<Vec<PColor>>,
    pub palette_index: usize,
    pub color_index: usize,
    pub color: PColor,
    /// 0 = black, 1 = white, 2 = colour cycling.
    pub state: u8,
}

impl BgPalette {
    pub fn new(mut palettes: Vec<Vec<PColor>>) -> Self {
        if palettes.is_empty() {
            palettes.push(vec![
                PColor::rgba8(0, 0, 0, 255),
                PColor::rgba8(255, 255, 255, 255),
            ]);
        }
        let color = *palettes[0].get(1).unwrap_or(&palettes[0][0]);
        BgPalette {
            palettes,
            palette_index: 0,
            color_index: 0,
            color,
            state: 0,
        }
    }

    pub fn inc_state(&mut self) {
        self.state = (self.state + 1) % 3;
    }

    pub fn select_palette(&mut self, v: i32) {
        let max = (self.palettes.len() - 1) as f32;
        self.palette_index = (map_midi(v, 0.0, max) as usize).min(self.palettes.len() - 1);
        self.color_index = self
            .color_index
            .min(self.palettes[self.palette_index].len() - 1);
    }

    pub fn inc_color(&mut self) {
        let colors = &self.palettes[self.palette_index];
        self.color_index = (self.color_index + 1) % colors.len();
        self.color = colors[self.color_index];
    }

    /// True on a white background (things drawn on top switch to dark colours).
    pub fn black_or_white(&self) -> bool {
        self.state == 1
    }

    pub fn background(&self) -> PColor {
        match self.state {
            0 => PColor::rgba8(0, 0, 0, 255),
            1 => PColor::rgba8(255, 255, 255, 255),
            _ => self.color,
        }
    }
}

/// Small box in the top-right corner showing what has been cued up (e.g. the next visualiser).
#[derive(Default)]
pub struct CueInfo {
    pub text: String,
}

impl CueInfo {
    pub fn set(&mut self, t: impl Into<String>) {
        self.text = t.into();
    }
    pub fn clear(&mut self) {
        self.text.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_cycle_and_cue() {
        let mut w = WordPacks::new(vec![
            vec!["a".into(), "b".into()],
            vec!["x".into(), "y".into(), "z".into()],
        ]);
        w.next_word();
        assert_eq!(w.current_word(), "b");
        w.next_word();
        assert_eq!(w.current_word(), "a");
        assert_eq!(w.select(127), "x");
        assert_eq!(w.current_word(), "a", "selecting doesn't switch until cued");
        w.cue();
        assert_eq!(w.current_word(), "x");
    }

    #[test]
    fn palette_states() {
        let mut p = BgPalette::new(vec![vec![
            PColor::rgba8(255, 0, 0, 255),
            PColor::rgba8(0, 255, 0, 255),
        ]]);
        assert_eq!(p.background(), PColor::rgba8(0, 0, 0, 255));
        p.inc_state();
        assert!(p.black_or_white());
        p.inc_state();
        p.inc_color();
        assert_eq!(p.background(), PColor::rgba8(0, 255, 0, 255));
        p.inc_state();
        assert_eq!(p.state, 0);
    }
}
