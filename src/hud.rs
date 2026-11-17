//! On-screen text: beat words, info/FPS/help screens and the cue-info box.
//!
//! These draw on an un-flipped nannou `Draw` (text would be mirrored on the y-down one), so this
//! module converts from Processing's top-left coordinates itself.

use crate::controls::ControlKind;
use crate::decks::DeckSettings;
use crate::state::State;
use crate::util::{map, PColor, MIDI_MAX};
use nannou::draw::Draw;
use nannou::geom::vec2;
use nannou::text::Font;

#[derive(Copy, Clone)]
pub enum Align {
    Left,
    Center,
}

/// Draws text at Processing coordinates.
pub struct Hud<'a> {
    pub draw: &'a Draw,
    pub w: f32,
    pub h: f32,
    pub font: Option<&'a Font>,
}

impl Hud<'_> {
    fn to_nannou(&self, x: f32, y: f32) -> (f32, f32) {
        (x - self.w / 2.0, self.h / 2.0 - y)
    }

    /// `text(s, x, y)` with the baseline of the first line at `y`.
    pub fn text(&self, s: &str, x: f32, y: f32, size: u32, color: PColor, align: Align) {
        if s.is_empty() {
            return;
        }
        let lines = s.lines().count().max(1) as f32;
        let line_h = size as f32 * 1.25;
        let box_w = self.w * 2.0;
        let box_h = lines * line_h + size as f32;
        let top = y - size as f32 * 0.9;
        let (cx, cy) = match align {
            Align::Left => self.to_nannou(x + box_w / 2.0, top + box_h / 2.0),
            Align::Center => self.to_nannou(x, top + box_h / 2.0),
        };
        let t = self
            .draw
            .text(s)
            .x_y(cx, cy)
            .w_h(box_w, box_h)
            .font_size(size)
            .no_line_wrap()
            .align_text_top()
            .color(color.lin());
        let t = match align {
            Align::Left => t.left_justify(),
            Align::Center => t.center_justify(),
        };
        if let Some(f) = self.font {
            t.font(f.clone());
        }
    }

    /// Text centred on a point both ways (`textAlign(CENTER, CENTER)`).
    pub fn text_centered(&self, s: &str, x: f32, y: f32, size: u32, color: PColor) {
        if s.is_empty() {
            return;
        }
        let (cx, cy) = self.to_nannou(x, y);
        let t = self
            .draw
            .text(s)
            .x_y(cx, cy)
            .w_h(self.w * 2.0, size as f32 * 2.0)
            .font_size(size)
            .no_line_wrap()
            .center_justify()
            .align_text_middle_y()
            .color(color.lin());
        if let Some(f) = self.font {
            t.font(f.clone());
        }
    }

    /// `rect(x, y, w, h)` in corner mode (negative sizes allowed, as in Processing).
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, fill: PColor, stroke: Option<PColor>) {
        let (cx, cy) = self.to_nannou(x + w / 2.0, y + h / 2.0);
        let r = self
            .draw
            .rect()
            .x_y(cx, cy)
            .w_h(w.abs(), h.abs())
            .color(fill.lin());
        if let Some(s) = stroke {
            r.stroke(s.lin()).stroke_weight(1.0);
        }
    }

    pub fn triangle(&self, a: (f32, f32), b: (f32, f32), c: (f32, f32), fill: PColor) {
        let p = |(x, y): (f32, f32)| {
            let (x, y) = self.to_nannou(x, y);
            vec2(x, y)
        };
        self.draw.tri().points(p(a), p(b), p(c)).color(fill.lin());
    }
}

/// The beat word, big and centred.
pub fn draw_words(hud: &Hud, state: &State) {
    if !state.words.on {
        return;
    }
    let v = if state.palette.black_or_white() {
        0.0
    } else {
        255.0
    };
    hud.text_centered(
        state.words.current_word(),
        hud.w / 2.0,
        hud.h / 2.0,
        200,
        PColor::gray(v, state.words.alpha as f32),
    );
}

pub fn draw_fps(hud: &Hud, fps: f32) {
    hud.text_centered(
        &format!("FPS = {}", fps.round() as i32),
        hud.w / 2.0,
        hud.h / 8.0,
        52,
        PColor::gray(150.0, 255.0),
    );
}

pub fn draw_help(hud: &Hud) {
    hud.text(
        "Help\n------------\ni - Info toggle\nf - FPS toggle\nw - Waveform toggle\n\
         d - Next Visualiser\na - Prev Visualiser\np - SPOUT toggle",
        hud.w - 200.0,
        30.0,
        12,
        PColor::gray(150.0, 255.0),
        Align::Left,
    );
}

pub fn draw_cue_info(hud: &Hud, state: &State) {
    if state.cue_info.text.is_empty() {
        return;
    }
    let box_size = 18.0;
    let margin = 10.0;
    let x = hud.w - box_size - margin;
    let y = margin;
    hud.rect(
        x,
        y,
        box_size,
        box_size,
        PColor::gray(50.0, 255.0),
        Some(PColor::gray(50.0, 255.0)),
    );
    hud.text_centered(
        &state.cue_info.text,
        x + box_size / 2.0,
        y + box_size / 2.0,
        18,
        PColor::gray(220.0, 255.0),
    );
}

fn draw_deck(hud: &Hud, deck: &DeckSettings, x: f32, y: f32) {
    let white = PColor::gray(255.0, 255.0);
    let grey = PColor::gray(100.0, 255.0);
    let blue = PColor::rgb(50.0, 50.0, 200.0);
    let max = MIDI_MAX as f32;
    let fill = PColor::gray(150.0, 255.0);
    hud.text(deck.name, x, y, 72, fill, Align::Left);

    // play / stop
    let size = 40.0;
    let (px, py) = (x, y + 15.0);
    if deck.playing {
        hud.triangle(
            (px, py),
            (px, py + size),
            (px + size, py + size / 2.0),
            fill,
        );
    } else {
        hud.rect(px, py, size, size, fill, None);
    }

    // fader
    let (fx, fy) = (x, y + 60.0);
    hud.rect(fx, fy, 25.0, max, grey, None);
    hud.rect(
        fx,
        fy + max,
        25.0,
        -(deck.fader_value as f32),
        blue,
        Some(white),
    );
    hud.text("Fader", fx, fy + max + 17.0, 12, white, Align::Left);

    // filter
    let fv = map(deck.filter_value, -8.0, 8.0, 0.0, max);
    let middle = (MIDI_MAX / 2) as f32;
    let (gx, gy) = (x + 45.0, y + 60.0);
    hud.rect(gx, gy, 25.0, max, grey, None);
    hud.rect(gx, gy + middle, 25.0, middle - fv, blue, Some(white));
    hud.text(
        "Filter",
        gx,
        gy + 2.0 * middle + 17.0,
        12,
        white,
        Align::Left,
    );
}

/// The "i" info screen.
pub fn draw_info(hud: &Hud, state: &State) {
    let grey = PColor::gray(150.0, 255.0);
    hud.text(
        &format!(
            "Info\n------------\nVisualiser = {}\nwaveform = {}\nStickFigures = {}\n\
             Post Shader = {}\n",
            state.visualisers.name(),
            state.waveform_on,
            state.stick_figures.on,
            state.post.info()
        ),
        10.0,
        30.0,
        16,
        grey,
        Align::Left,
    );

    // decks in mixer order: C A B D
    let w = (hud.w / 5.0).floor();
    let w2 = (w / 2.0).floor();
    let h = (hud.h / 4.0).floor();
    for (col, deck) in [2usize, 0, 1, 3].iter().enumerate() {
        draw_deck(hud, &state.decks[*deck], w * col as f32 + w2, h);
    }

    // the control that last fired
    let (cx, mut cy) = (hud.w / 2.0, hud.h / 2.0);
    let light = PColor::gray(200.0, 255.0);
    match state.controls.current() {
        Some(c) => {
            if let ControlKind::Midi {
                channel,
                control_num,
            } = c.kind
            {
                for line in [
                    format!("Channel = {channel}"),
                    format!("Control Number = {control_num}"),
                    format!("Value = {}", c.value),
                ] {
                    hud.text(&line, cx, cy, 33, light, Align::Center);
                    cy += 40.0;
                }
                cy += 40.0;
                hud.text(&c.function_name, cx, cy, 33, light, Align::Center);
            }
        }
        None => hud.text("Undefined", cx, cy, 33, light, Align::Center),
    }

    // tempo
    let bpm = &state.bpm;
    let (bx, by) = (hud.w / 2.0, hud.h / 5.0 * 4.0);
    hud.text(
        &format!("BPM = {} beat duration = {}ms", bpm.bpm, bpm.beat_duration),
        bx,
        by,
        50,
        grey,
        Align::Center,
    );
    hud.text(
        &format!("Beat = {} : {}", bpm.bar, bpm.beat + 1),
        bx,
        by + 40.0,
        50,
        grey,
        Align::Center,
    );
    hud.text(
        "Quarter beat pulse = ",
        bx,
        by + 75.0,
        25,
        grey,
        Align::Center,
    );
    hud.rect(
        bx + 120.0,
        by + 75.0,
        20.0,
        -25.0 * bpm.quarter_note_pulse,
        grey,
        None,
    );
}
