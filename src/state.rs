//! All of the visualiser's state, plus the actions that MIDI messages and key presses trigger.
//!
//! In the sketch these were globals and `action()` used Java reflection to call a function by the
//! name given in the mapping files. Here `State::action` matches on that same name, so the JSON
//! mapping files keep working unchanged; `ACTIONS` lists the names that exist so unknown ones in
//! the config can be reported at startup (`validateControls`).

use crate::audio::{AudioInput, BUFFER_SIZE};
use crate::bpm::Bpm;
use crate::config::{self, MainConfig};
use crate::controls::{java_key_code, Controls};
use crate::decks::{self, DeckSettings};
use crate::fft::FftAnalysis;
use crate::hotcues::HotcuePacks;
use crate::midi::{Midi, MidiEvent};
use crate::overlays::{BgPalette, CueInfo, WordPacks};
use crate::post::{
    BlurShader, Kaleidoscope, PostKind, PostProcessingShaders, ShaderTime, VisShader,
};
use crate::util::{map_midi, PNoise, MIDI_MAX};
use crate::video::LiveCapture;
use crate::vis::stick_figures::{DanceRoutines, VisStickFigures};
use crate::vis::waveform::VisWaveform;
use crate::vis::{self, Visualisers};
use nannou::wgpu;
use std::path::PathBuf;
use std::time::Instant;

/// Every action name that can be used in midiConfig.json / midiTraktorConfig.json /
/// keyboardConfig.json.
pub const ACTIONS: &[&str] = &[
    "setVolumeDeckA",
    "setVolumeDeckB",
    "setVolumeDeckC",
    "setVolumeDeckD",
    "setIsPlayingDeckA",
    "setIsPlayingDeckB",
    "setIsPlayingDeckC",
    "setIsPlayingDeckD",
    "readyHotCueDeckA",
    "readyHotCueDeckB",
    "readyHotCueDeckC",
    "readyHotCueDeckD",
    "setHotCuePackA",
    "setHotCuePackB",
    "setHotCuePackC",
    "setHotCuePackD",
    "hotCueClearCueInfo",
    "showImage0",
    "showImage1",
    "showImage2",
    "showImage3",
    "showImage4",
    "showImage5",
    "showImage6",
    "showImage7",
    "setFilterA",
    "setFilterB",
    "setFilterC",
    "setFilterD",
    "setDelayStatus",
    "setGlitchStatus",
    "selectVisualiser",
    "selectVisualiserBykeypress",
    "cueVisualiser",
    "toggleVisualiserButton1",
    "setVisualiserButton1On",
    "setVisualiserButton1Off",
    "toggleVisualiserButton2",
    "setVisualiserButton2On",
    "setVisualiserButton2Off",
    "toggleVisualiserButton3",
    "setVisualiserButton3On",
    "setVisualiserButton3Off",
    "setVisualiserFader1",
    "setVisualiserFader2",
    "setVisualiserFader3",
    "toggleWaveform",
    "setWaveformOn",
    "setWaveformScale",
    "toggleKaleidoscope",
    "setKaleidoscopeOn",
    "setKaleidoscopeRotation",
    "setKaleidoscopeViewAngle",
    "toggleAutoPlay",
    "triggerBeatActions",
    "resetBeatCounters",
    "setBeatCount",
    "switchBackground",
    "selectBackgroundPalette",
    "selectWordPack",
    "cueWordPack",
    "toggleWordDraw",
    "setWordAlpha",
    "selectPostShader",
    "togglePostShader",
    "setPostShaderOn",
    "setPostShaderValue1",
    "setPostShaderValue2",
    "setPostShaderAutoOn",
    "stickFiguresToggle",
    "stickFiguresScale",
    "stickFiguresPositionSet",
    "stickFiguresCrowdSize",
    "stickFiguresSelectDanceRoutine",
    "stickFiguresNextPositionSet",
    "toggleInfo",
    "toggleFPS",
    "toggleHelp",
    "toggleSpout",
    "videoToggle",
    "videoParam",
    "videoToggle2",
    "videoToggle3",
    "videoToggle4",
    "videoToggle5",
    "videoParam2",
    "videoParam3",
    "videoParam4",
    "videoParam5",
    "faderTest0",
    "faderTest1",
    "faderTest2",
    "faderTest3",
    "buttonTest0",
    "buttonTest1",
    "buttonTest2",
    "buttonTest3",
];

pub struct State {
    pub width: f32,
    pub height: f32,
    start: Instant,
    pub frame_count: u64,

    audio: AudioInput,
    pub audio_buf: Vec<f32>,
    pub fft: FftAnalysis,
    pub bpm: Bpm,
    pub noise: PNoise,

    midi: Midi,
    pub controls: Controls,

    pub decks: [DeckSettings; 4],
    pub filter_intensity: f32,
    pub hotcues: HotcuePacks,
    pub words: WordPacks,
    pub palette: BgPalette,
    pub cue_info: CueInfo,

    pub visualisers: Visualisers,
    pub waveform: VisWaveform,
    pub stick_figures: VisStickFigures,
    pub dance: DanceRoutines,

    pub post: PostProcessingShaders,
    pub kaleidoscope: Option<Kaleidoscope>,
    pub sobel: Option<VisShader>,
    pub vhs_glitch: Option<VisShader>,
    pub blur: Option<BlurShader>,

    pub live: LiveCapture,

    pub kaleidoscope_on: bool,
    pub waveform_on: bool,
    pub delay_on: bool,
    pub slice_on: bool,
    pub video_on: bool,
    pub info_on: bool,
    pub help_on: bool,
    pub fps_on: bool,
    pub auto_play: bool,

    pub fader_test: [f32; 4],
}

fn deck_index(letter: char) -> usize {
    match letter {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
    }
}

fn opt<T>(r: anyhow::Result<T>, what: &str) -> Option<T> {
    r.map_err(|e| log::error!("{what} disabled: {e}")).ok()
}

impl State {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: PathBuf,
        cfg: &MainConfig,
        size: [u32; 2],
    ) -> Self {
        let (w, h) = (size[0] as f32, size[1] as f32);
        let shader_dir = data.join("Shaders");

        let audio = AudioInput::new();
        let fft = FftAnalysis::new(BUFFER_SIZE, audio.sample_rate() as f32);

        // MIDI + mappings. As in the sketch, controller/Traktor mappings are only loaded when the
        // matching device was found; keyboard mappings are always loaded.
        let midi = Midi::open(&cfg.midi_devices);
        let mut controls = Controls::new(midi.controller_out.clone());
        if midi.controller_connected {
            match config::load_controller_mappings(&data) {
                Ok(m) => m.iter().for_each(|m| {
                    controls.add_midi(m.channel, m.control, &m.function_name, m.default_value)
                }),
                Err(e) => log::error!("{e:#}"),
            }
        }
        if midi.traktor_connected {
            match config::load_traktor_mappings(&data) {
                Ok(m) => m
                    .iter()
                    .for_each(|m| controls.add_midi(m.channel, m.control, &m.function_name, 0)),
                Err(e) => log::error!("{e:#}"),
            }
        }
        match config::load_key_mappings(&data) {
            Ok(m) => m
                .iter()
                .for_each(|k| controls.add_key(k.key_code, &k.function_name, k.value)),
            Err(e) => log::error!("{e:#}"),
        }
        for c in controls.all() {
            if !ACTIONS.contains(&c.function_name.as_str()) {
                log::warn!(
                    "WARNING: Config references unknown function: '{}'",
                    c.function_name
                );
            }
        }

        let kaleidoscope = opt(Kaleidoscope::new(device, &shader_dir), "Kaleidoscope");
        if kaleidoscope.is_some() && midi.controller_connected {
            controls.set_value_for("setKaleidoscopeViewAngle", Kaleidoscope::DEFAULT_VALUE);
        }

        let dance = DanceRoutines::load(&data).unwrap_or_else(|e| {
            log::error!("Error: can't load 'dancemoves.json' ({e:#}); figures will stand still");
            DanceRoutines::standing()
        });
        let frame_rate = cfg.screen.framerate as f32;

        log::info!(
            "Traktor MIDI channel (from config.json): {}",
            cfg.midi_channel + 1
        );

        State {
            width: w,
            height: h,
            start: Instant::now(),
            frame_count: 0,
            audio,
            audio_buf: vec![0.0; BUFFER_SIZE],
            fft,
            bpm: Bpm::new(),
            noise: PNoise::new(),
            midi,
            controls,
            decks: [
                DeckSettings::new("A"),
                DeckSettings::new("B"),
                DeckSettings::new("C"),
                DeckSettings::new("D"),
            ],
            filter_intensity: 0.0,
            hotcues: HotcuePacks::load(device, queue, &data),
            words: WordPacks::new(cfg.word_packs.clone()),
            palette: BgPalette::new(cfg.palettes.clone()),
            cue_info: CueInfo::default(),
            visualisers: Visualisers::new(vis::build_all(device, &data, w, h)),
            waveform: VisWaveform::new(),
            stick_figures: VisStickFigures::new(w, h, frame_rate, &dance),
            dance,
            post: PostProcessingShaders::new(device, &shader_dir, w, h),
            kaleidoscope,
            sobel: opt(
                VisShader::load(device, &shader_dir, PostKind::Sobel, w, h),
                "Sobel (echo)",
            ),
            vhs_glitch: opt(
                VisShader::load(device, &shader_dir, PostKind::VhsGlitch, w, h),
                "VHS glitch (gate/slice)",
            ),
            blur: opt(BlurShader::new(device, &shader_dir), "Blur (filter)"),
            live: LiveCapture::new(cfg.camera.as_ref()),
            kaleidoscope_on: false,
            waveform_on: false,
            delay_on: false,
            slice_on: false,
            video_on: false,
            info_on: false,
            help_on: false,
            fps_on: false,
            auto_play: false,
            fader_test: [0.0; 4],
        }
    }

    /// Use a synthetic signal instead of the line-in (`--test-signal`).
    pub fn use_test_signal(&mut self) {
        self.audio.use_test_signal();
        self.fft = FftAnalysis::new(BUFFER_SIZE, self.audio.sample_rate() as f32);
    }

    /// `millis()`.
    pub fn millis(&self) -> i64 {
        self.start.elapsed().as_millis() as i64
    }

    pub fn shader_time(&self) -> ShaderTime {
        ShaderTime {
            time: self.start.elapsed().as_secs_f32(),
            frame_count: self.frame_count,
            width: self.width,
            height: self.height,
        }
    }

    /// Per-frame updates that happen before drawing.
    pub fn update(&mut self) {
        while let Ok(ev) = self.midi.events.try_recv() {
            self.handle_midi(ev);
        }
        if decks::any_deck_playing(&self.decks) {
            let now = self.millis();
            self.bpm.calc_pulse(now);
        }
        self.audio.read(&mut self.audio_buf);
        self.fft.update(&self.audio_buf);
    }

    pub fn handle_midi(&mut self, ev: MidiEvent) {
        match ev {
            MidiEvent::ControlChange {
                channel,
                number,
                value,
            } => {
                let key = Controls::get_midi_key(channel, number);
                match self.controls.get_mut(key) {
                    None => self.controls.clear_current(),
                    Some(c) => {
                        c.value = value;
                        let name = c.function_name.clone();
                        self.controls.set_current(key);
                        self.action(&name, value);
                    }
                }
            }
            MidiEvent::NoteOn {
                channel,
                pitch,
                velocity,
            } => {
                log::debug!("Note On : Channel = {channel} Pitch = {pitch} Vel = {velocity}");
            }
            MidiEvent::NoteOff { .. } => {}
        }
    }

    pub fn handle_key(&mut self, key: nannou::event::Key) {
        let Some(code) = java_key_code(key) else {
            return;
        };
        log::debug!("Key = {key:?} KeyCode = {code}");
        let key_id = crate::controls::KEYBOARD_CHANNEL * 100 + code;
        match self.controls.get_mut(key_id) {
            None => self.controls.clear_current(),
            Some(c) => {
                let (name, value) = (c.function_name.clone(), c.value);
                log::debug!("Function = {name}");
                self.controls.set_current(key_id);
                self.action(&name, value);
            }
        }
    }

    /// Everything that happens on a beat from Traktor.
    fn on_beat(&mut self) {
        let now = self.millis();
        let ev = self.bpm.on_beat(now);
        if ev.quarter_beat {
            self.palette.inc_color();
            self.words.next_word();
            self.dance.on_beat();
            self.stick_figures.on_beat(&self.dance);
            self.visualisers.on_beat();
            self.post.beat_xy();
        }
        if ev.beat_count {
            self.visualisers.on_beat_count(self.auto_play);
        }
    }

    fn set_hotcue_pack(&mut self, deck: usize, value: i32) {
        let mut pack = DeckSettings::pack_from_value(value);
        if pack >= self.hotcues.len() {
            log::warn!("Warning - Pack number {pack} does not exist. Setting to pack 0");
            pack = 0;
        }
        let d = &mut self.decks[deck];
        d.hotcue_pack = pack;
        d.hotcue_index = 0;
        self.cue_info.set(format!("hc{pack}"));
        let name = format!("setHotCuePack{}", d.name);
        self.controls
            .write_colour_for(&name, pack, decks::NUM_OF_PACKS - 1);
    }

    fn ready_hotcue(&mut self, deck: usize, value: i32, frames: i32) {
        let d = &mut self.decks[deck];
        if d.loud_enough() {
            d.hotcue_index = DeckSettings::hotcue_from_value(value);
            self.hotcues
                .ready_hotcue(d.hotcue_pack, d.hotcue_index, frames);
        }
    }

    fn refresh_filter(&mut self) {
        self.filter_intensity = decks::filter_intensity(&self.decks);
    }

    /// Run the action called `name`. Returns false if there is no such action.
    pub fn action(&mut self, name: &str, value: i32) -> bool {
        let deck_suffix = name.chars().last().unwrap_or('A');
        match name {
            // Decks
            "setVolumeDeckA" | "setVolumeDeckB" | "setVolumeDeckC" | "setVolumeDeckD" => {
                self.decks[deck_index(deck_suffix)].set_volume(value);
                self.refresh_filter();
            }
            "setIsPlayingDeckA" | "setIsPlayingDeckB" | "setIsPlayingDeckC"
            | "setIsPlayingDeckD" => {
                self.decks[deck_index(deck_suffix)].playing = value == MIDI_MAX;
            }
            "readyHotCueDeckA" | "readyHotCueDeckB" | "readyHotCueDeckC" | "readyHotCueDeckD" => {
                if value > 0 {
                    self.ready_hotcue(deck_index(deck_suffix), value, 15);
                }
            }
            "setHotCuePackA" | "setHotCuePackB" | "setHotCuePackC" | "setHotCuePackD" => {
                if value > 0 {
                    self.set_hotcue_pack(deck_index(deck_suffix), value);
                }
            }
            "hotCueClearCueInfo" => self.cue_info.clear(),
            "showImage0" | "showImage1" | "showImage2" | "showImage3" | "showImage4"
            | "showImage5" | "showImage6" | "showImage7" => {
                if value > 0 {
                    let n = deck_suffix.to_digit(10).unwrap_or(0) as i32;
                    // the Traktor hotcue values: 15, 31, ... 127
                    self.ready_hotcue(0, 15 + 16 * n, 30);
                }
            }
            "setFilterA" | "setFilterB" | "setFilterC" | "setFilterD" => {
                self.decks[deck_index(deck_suffix)].set_filter(value);
                self.refresh_filter();
            }
            "setDelayStatus" => {
                if value > 100 {
                    self.delay_on = true;
                } else if value == 0 {
                    self.delay_on = false;
                }
            }
            "setGlitchStatus" => {
                if value > 100 {
                    self.slice_on = true;
                } else if value == 0 {
                    self.slice_on = false;
                }
            }

            // Visualiser selection and parameters
            "selectVisualiser" => {
                self.visualisers
                    .select_by_midi(value, &mut self.cue_info, &self.controls)
            }
            "selectVisualiserBykeypress" => self
                .visualisers
                .select_by_keypress(value, &mut self.cue_info),
            "cueVisualiser" => self.visualisers.cue(&mut self.cue_info),
            "toggleVisualiserButton1"
            | "setVisualiserButton1On"
            | "setVisualiserButton1Off"
            | "toggleVisualiserButton2"
            | "setVisualiserButton2On"
            | "setVisualiserButton2Off"
            | "toggleVisualiserButton3"
            | "setVisualiserButton3On"
            | "setVisualiserButton3Off"
            | "setVisualiserFader1"
            | "setVisualiserFader2"
            | "setVisualiserFader3" => {
                if let Some(v) = self.visualisers.current_mut() {
                    match name {
                        "toggleVisualiserButton1" => v.toggle_button1(),
                        "setVisualiserButton1On" => v.set_button1(true),
                        "setVisualiserButton1Off" => v.set_button1(false),
                        "toggleVisualiserButton2" => v.toggle_button2(),
                        "setVisualiserButton2On" => v.set_button2(true),
                        "setVisualiserButton2Off" => v.set_button2(false),
                        "toggleVisualiserButton3" => v.toggle_button3(),
                        "setVisualiserButton3On" => v.set_button3(true),
                        "setVisualiserButton3Off" => v.set_button3(false),
                        "setVisualiserFader1" => v.set_fader1(value),
                        "setVisualiserFader2" => v.set_fader2(value),
                        _ => v.set_fader3(value),
                    }
                }
            }

            // Overlays
            "toggleWaveform" => self.waveform_on = !self.waveform_on,
            "setWaveformOn" => self.waveform_on = value > 120,
            "setWaveformScale" => self.waveform.set_scale(value),
            "toggleKaleidoscope" => self.kaleidoscope_on = !self.kaleidoscope_on,
            "setKaleidoscopeOn" => self.kaleidoscope_on = value > 120,
            "setKaleidoscopeRotation" => {
                if let Some(k) = &mut self.kaleidoscope {
                    k.set_rotation(value);
                }
            }
            "setKaleidoscopeViewAngle" => {
                if let Some(k) = &mut self.kaleidoscope {
                    k.set_view_angle(value);
                }
            }
            "toggleAutoPlay" => self.auto_play = !self.auto_play,

            // Beats
            "triggerBeatActions" => {
                if value > 120 {
                    self.on_beat();
                }
            }
            "resetBeatCounters" => {
                self.bpm.reset_beat_counters();
                self.dance.reset();
            }
            "setBeatCount" => self.bpm.set_beat_count(value),

            // Background
            "switchBackground" => self.palette.inc_state(),
            "selectBackgroundPalette" => self.palette.select_palette(value),

            // Beat words
            "selectWordPack" => {
                let first = self.words.select(value);
                self.cue_info.set(first);
                self.controls.write_colour_for(
                    "selectWordPack",
                    self.words.cue_index,
                    self.words.packs.len() - 1,
                );
            }
            "cueWordPack" => {
                self.words.cue();
                self.cue_info.clear();
            }
            "toggleWordDraw" => self.words.on = !self.words.on,
            "setWordAlpha" => self.words.set_alpha(value),

            // Post-processing shaders
            "selectPostShader" => {
                let idx = self.post.select(value);
                self.controls
                    .write_colour_for("selectPostShader", idx, self.post.shaders.len());
            }
            "togglePostShader" => self.post.on = !self.post.on,
            "setPostShaderOn" => self.post.on = value > 120,
            "setPostShaderValue1" => {
                let w = self.width;
                if let Some(s) = self.post.current_mut() {
                    s.set_x(value, w);
                }
            }
            "setPostShaderValue2" => {
                let h = self.height;
                if let Some(s) = self.post.current_mut() {
                    s.set_y(value, h);
                }
            }
            "setPostShaderAutoOn" => self.post.auto_on = value > 120,

            // Stick figures
            "stickFiguresToggle" => self.stick_figures.toggle(),
            "stickFiguresScale" => self.stick_figures.set_scale(value),
            "stickFiguresPositionSet" | "stickFiguresNextPositionSet" => {
                self.stick_figures.next_position_set()
            }
            "stickFiguresCrowdSize" => self.stick_figures.set_crowd_size(value),
            "stickFiguresSelectDanceRoutine" => self.dance.select(value),

            // Info and debugging
            "toggleInfo" => self.info_on = !self.info_on,
            "toggleFPS" => self.fps_on = !self.fps_on,
            "toggleHelp" => self.help_on = !self.help_on,
            // Spout output is not part of the port (it was disabled in the sketch too, where this
            // key ended up toggling the help text). Kept so existing mappings still do something.
            "toggleSpout" => self.help_on = !self.help_on,

            // Video (camera capture is stubbed, see video.rs)
            "videoToggle" | "videoToggle2" | "videoToggle3" | "videoToggle4" | "videoToggle5" => {
                self.video_on = !self.video_on
            }
            "videoParam" => self.live.set_param1(value),
            "videoParam2" => self.live.set_param2(value),
            "videoParam3" => self.live.set_param3(value),
            "videoParam4" => self.live.set_param4(value),
            "videoParam5" => self.live.set_param5(value),

            // Test controls
            "faderTest0" | "faderTest1" | "faderTest2" | "faderTest3" => {
                let i = deck_suffix.to_digit(10).unwrap_or(0) as usize;
                self.fader_test[i] = map_midi(value, 0.0, 100.0);
            }
            "buttonTest0" | "buttonTest1" | "buttonTest2" | "buttonTest3" => {}

            _ => {
                log::warn!("Error in control change: unknown function '{name}' (value = {value})");
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action named in the shipped mapping files exists (apart from two the original
    /// sketch also lacked).
    #[test]
    fn mapping_files_reference_known_actions() {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
        let known_missing = ["toggleBackgroundBeatSync", "videoBlendMode"];
        let mut names: Vec<String> = Vec::new();
        names.extend(
            config::load_controller_mappings(&data)
                .unwrap()
                .into_iter()
                .map(|m| m.function_name),
        );
        names.extend(
            config::load_traktor_mappings(&data)
                .unwrap()
                .into_iter()
                .map(|m| m.function_name),
        );
        names.extend(
            config::load_key_mappings(&data)
                .unwrap()
                .into_iter()
                .map(|m| m.function_name),
        );
        let unknown: Vec<_> = names
            .iter()
            .filter(|n| !ACTIONS.contains(&n.as_str()) && !known_missing.contains(&n.as_str()))
            .collect();
        assert!(unknown.is_empty(), "unknown actions: {unknown:?}");
    }
}
