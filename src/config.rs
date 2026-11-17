//! Loading of the JSON config files in the data directory.
//!
//! The files are shared with the Processing sketch, so this accepts what Processing's JSON parser
//! accepts: numbers and booleans written as strings (`"width": "1200"`) and trailing commas.

use crate::util::PColor;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Read a JSON file, tolerating trailing commas.
pub fn load_json(path: &Path) -> Result<Value> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_lenient(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn parse_lenient(text: &str) -> Result<Value> {
    match serde_json::from_str(text) {
        Ok(v) => Ok(v),
        Err(_) => {
            let trailing = regex::Regex::new(r",(\s*[\]}])").unwrap();
            let fixed = trailing.replace_all(text, "$1");
            Ok(serde_json::from_str(&fixed)?)
        }
    }
}

/// Integer value that may be stored as a number or a numeric string.
pub fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|f| f as i64),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

pub fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_i64().map(|i| i != 0),
        _ => None,
    }
}

pub fn get_i64(obj: &Value, key: &str) -> Option<i64> {
    obj.get(key).and_then(as_i64)
}

pub fn get_bool(obj: &Value, key: &str) -> Option<bool> {
    obj.get(key).and_then(as_bool)
}

pub fn get_str<'a>(obj: &'a Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}

#[derive(Clone, Debug)]
pub struct ScreenSettings {
    pub fullscreen: bool,
    pub display: usize,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub always_on_top: bool,
    pub framerate: u32,
    pub location: (i32, i32),
}

#[derive(Clone, Debug)]
pub struct CameraSettings {
    pub index: usize,
    pub width: u32,
    pub height: u32,
}

/// A mapping from a MIDI CC (or a key) to an action, as read from the mapping files.
#[derive(Clone, Debug)]
pub struct MidiMapping {
    pub channel: i32,
    pub control: i32,
    pub function_name: String,
    pub default_value: i32,
}

#[derive(Clone, Debug)]
pub struct KeyMapping {
    pub key_code: i32,
    pub function_name: String,
    pub value: i32,
}

/// Everything read from `config.json`.
#[derive(Clone, Debug)]
pub struct MainConfig {
    pub screen: ScreenSettings,
    pub midi_devices: Vec<String>,
    pub midi_channel: i32,
    pub camera: Option<CameraSettings>,
    pub word_packs: Vec<Vec<String>>,
    pub palettes: Vec<Vec<PColor>>,
}

fn first_entry<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    root.get(key)?.as_array()?.first()
}

impl MainConfig {
    pub fn load(data: &Path) -> Result<Self> {
        let root = load_json(&data.join("config.json"))?;

        let s = first_entry(&root, "Screensize").ok_or_else(|| {
            anyhow!("Can't find screensize definition in config file, please check this")
        })?;
        let screen = ScreenSettings {
            fullscreen: get_bool(s, "fullscreen").unwrap_or(false),
            display: get_i64(s, "displaynumber").unwrap_or(0).max(0) as usize,
            width: get_i64(s, "width").unwrap_or(1200).max(64) as u32,
            height: get_i64(s, "height").unwrap_or(1200).max(64) as u32,
            resizable: get_bool(s, "resizable").unwrap_or(true),
            always_on_top: get_bool(s, "alwaysOnTop").unwrap_or(false),
            framerate: get_i64(s, "framerate").unwrap_or(60).clamp(1, 1000) as u32,
            location: (
                get_i64(s, "locationX").unwrap_or(0) as i32,
                get_i64(s, "locationY").unwrap_or(0) as i32,
            ),
        };

        let midi_devices = root
            .get("MIDIdevice")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|d| get_str(d, "device").map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        // Traktor numbers channels 1-16 while MIDI bytes use 0-15.
        let midi_channel =
            match first_entry(&root, "MIDIchannel").and_then(|c| get_i64(c, "channel")) {
                Some(c) => c as i32 - 1,
                None => {
                    log::warn!("No Midi channel definition found in config.json file, assuming 11");
                    10
                }
            };

        let camera = first_entry(&root, "Camera").map(|c| CameraSettings {
            index: get_i64(c, "index").unwrap_or(0).max(0) as usize,
            width: get_i64(c, "width").unwrap_or(640) as u32,
            height: get_i64(c, "height").unwrap_or(480) as u32,
        });

        let word_packs = root
            .get("Wordpacks")
            .and_then(Value::as_array)
            .map(|packs| {
                packs
                    .iter()
                    .filter_map(|p| p.get("words")?.as_array().cloned())
                    .map(|words| {
                        words
                            .iter()
                            .map(|w| w.as_str().unwrap_or_default().to_string())
                            .collect::<Vec<_>>()
                    })
                    .filter(|w| !w.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let palettes = root
            .get("Palettes")
            .and_then(Value::as_array)
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| p.get("colours")?.as_array().cloned())
                    .map(|cols| {
                        cols.iter()
                            .filter_map(|c| {
                                let s = c.as_str()?;
                                let col = PColor::from_hex(s);
                                if col.is_none() {
                                    log::warn!("Ignoring bad palette colour '{s}'");
                                }
                                col
                            })
                            .collect::<Vec<_>>()
                    })
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Ok(MainConfig {
            screen,
            midi_devices,
            midi_channel,
            camera,
            word_packs,
            palettes,
        })
    }
}

fn load_midi_mappings(path: &Path) -> Result<Vec<MidiMapping>> {
    let root = load_json(path)?;
    let list = root
        .get("MIDIControls")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("no MIDIControls array in {}", path.display()))?;
    Ok(list
        .iter()
        .filter_map(|m| {
            Some(MidiMapping {
                channel: get_i64(m, "channel")? as i32,
                control: get_i64(m, "control")? as i32,
                function_name: get_str(m, "functionName")?.to_string(),
                default_value: get_i64(m, "default").unwrap_or(0) as i32,
            })
        })
        .collect())
}

/// `midiConfig.json`: the controller's knobs and buttons.
pub fn load_controller_mappings(data: &Path) -> Result<Vec<MidiMapping>> {
    load_midi_mappings(&data.join("midiConfig.json"))
}

/// `midiTraktorConfig.json`: the messages Traktor sends.
pub fn load_traktor_mappings(data: &Path) -> Result<Vec<MidiMapping>> {
    load_midi_mappings(&data.join("midiTraktorConfig.json"))
}

/// `keyboardConfig.json`.
pub fn load_key_mappings(data: &Path) -> Result<Vec<KeyMapping>> {
    let path = data.join("keyboardConfig.json");
    let root = load_json(&path)?;
    let list = root
        .get("KeyboardControls")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("no KeyboardControls array in {}", path.display()))?;
    Ok(list
        .iter()
        .filter_map(|k| {
            Some(KeyMapping {
                key_code: get_i64(k, "keyCode")? as i32,
                function_name: get_str(k, "functionName")?.to_string(),
                value: get_i64(k, "value").unwrap_or(0) as i32,
            })
        })
        .collect())
}

/// Find the shared `data` directory: `--data <dir>`, `$GSR_DATA`, or a `data` folder next to / above
/// the working directory or the executable.
pub fn find_data_dir() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--data" {
            if let Some(p) = args.next() {
                return Ok(PathBuf::from(p));
            }
        } else if let Some(p) = a.strip_prefix("--data=") {
            return Ok(PathBuf::from(p));
        }
    }
    if let Ok(p) = std::env::var("GSR_DATA") {
        return Ok(PathBuf::from(p));
    }
    let mut candidates = vec![PathBuf::from("data")];
    if let Ok(exe) = std::env::current_exe() {
        for anc in exe.ancestors().skip(1).take(5) {
            candidates.push(anc.join("data"));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.join("config.json").is_file())
        .ok_or_else(|| {
            anyhow!("could not find the data directory (pass --data <dir> or set GSR_DATA)")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    #[test]
    fn lenient_json() {
        let v = parse_lenient(r#"{"a": [1, 2, ], "b": {"c": "3",},}"#).unwrap();
        assert_eq!(as_i64(&v["a"][1]), Some(2));
        assert_eq!(get_i64(&v["b"], "c"), Some(3));
    }

    #[test]
    fn loads_repo_config() {
        let c = MainConfig::load(&data()).unwrap();
        assert!(c.screen.width > 0);
        assert!(!c.word_packs.is_empty());
        assert!(!c.palettes.is_empty());
        assert_eq!(c.midi_channel, 10);
        assert!(!load_key_mappings(&data()).unwrap().is_empty());
        assert!(!load_controller_mappings(&data()).unwrap().is_empty());
        assert!(!load_traktor_mappings(&data()).unwrap().is_empty());
    }
}
