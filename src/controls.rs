//! MIDI and keyboard controls: which incoming CC / key press triggers which action, plus LED
//! feedback to the controller.

use crate::midi::MidiOut;
use crate::util::map;
use std::collections::HashMap;

// MIDI channels used by the Midi Fighter Twister (0-based, as on the wire).
/// Midi Out: sets the colour of the large LED.
pub const CHANNEL_CONTROL_COLOR: i32 = 1;
/// Midi Out: sets the animation state of the large LED.
pub const CHANNEL_CONTROL_ANI: i32 = 3;
/// Midi Out: sets the value of the knob's LED ring.
pub const CHANNEL_CONTROL_CC_OUT: i32 = 0;
/// "Dummy" channel used to key keyboard controls in the same map as MIDI ones.
pub const KEYBOARD_CHANNEL: i32 = 20;

#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    Midi { channel: i32, control_num: i32 },
    Key { key_number: i32 },
}

#[derive(Clone, Debug)]
pub struct Control {
    pub kind: ControlKind,
    pub function_name: String,
    pub value: i32,
}

impl Control {
    pub fn function_key(&self) -> i32 {
        match self.kind {
            ControlKind::Midi {
                channel,
                control_num,
            } => channel * 100 + control_num,
            ControlKind::Key { key_number } => KEYBOARD_CHANNEL * 100 + key_number,
        }
    }
}

pub struct Controls {
    controls: HashMap<i32, Control>,
    current: Option<i32>,
    out: MidiOut,
}

impl Controls {
    pub fn new(out: MidiOut) -> Self {
        Controls {
            controls: HashMap::new(),
            current: None,
            out,
        }
    }

    /// Add a MIDI control and push its default value to the controller.
    pub fn add_midi(&mut self, channel: i32, control_num: i32, function_name: &str, value: i32) {
        let c = Control {
            kind: ControlKind::Midi {
                channel,
                control_num,
            },
            function_name: function_name.to_string(),
            value,
        };
        self.out.send_controller_change(channel, control_num, value);
        self.controls.insert(c.function_key(), c);
    }

    pub fn add_key(&mut self, key_number: i32, function_name: &str, value: i32) {
        let c = Control {
            kind: ControlKind::Key { key_number },
            function_name: function_name.to_string(),
            value,
        };
        self.controls.insert(c.function_key(), c);
    }

    pub fn get_midi_key(channel: i32, number: i32) -> i32 {
        channel * 100 + number
    }

    pub fn get_mut(&mut self, function_key: i32) -> Option<&mut Control> {
        self.controls.get_mut(&function_key)
    }

    pub fn all(&self) -> impl Iterator<Item = &Control> {
        self.controls.values()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.controls.len()
    }

    pub fn set_current(&mut self, function_key: i32) {
        self.current = Some(function_key);
    }

    pub fn clear_current(&mut self) {
        self.current = None;
    }

    pub fn current(&self) -> Option<&Control> {
        self.current.and_then(|k| self.controls.get(&k))
    }

    fn by_function_name(&self, name: &str) -> Option<&Control> {
        self.controls.values().find(|c| c.function_name == name)
    }

    /// `writeControlColour`: show a selection index on the knob's LED colour.
    pub fn write_colour_for(&self, function_name: &str, index: usize, size: usize) {
        if let Some(Control {
            kind: ControlKind::Midi { control_num, .. },
            ..
        }) = self.by_function_name(function_name)
        {
            let v = if size <= 1 {
                1
            } else {
                map(index as f32, 0.0, (size - 1) as f32, 1.0, 126.0) as i32
            };
            self.out
                .send_controller_change(CHANNEL_CONTROL_COLOR, *control_num, v);
        }
    }

    /// `writeControlAni`: set the knob's LED animation state.
    #[allow(dead_code)]
    pub fn write_animation_for(&self, function_name: &str, index: usize, size: usize) {
        if let Some(Control {
            kind: ControlKind::Midi { control_num, .. },
            ..
        }) = self.by_function_name(function_name)
        {
            let v = if size <= 1 {
                1
            } else {
                map(index as f32, 0.0, (size - 1) as f32, 1.0, 126.0) as i32
            };
            self.out
                .send_controller_change(CHANNEL_CONTROL_ANI, *control_num, v);
        }
    }

    /// `MidiControl.setValue`: store a value and, for knobs, move the LED ring to match.
    pub fn set_value_for(&mut self, function_name: &str, v: i32) {
        let out = self.out.clone();
        if let Some(c) = self
            .controls
            .values_mut()
            .find(|c| c.function_name == function_name)
        {
            c.value = v;
            if let ControlKind::Midi {
                channel: 0,
                control_num,
            } = c.kind
            {
                out.send_controller_change(CHANNEL_CONTROL_CC_OUT, control_num, v);
            }
        }
    }
}

/// Map a nannou key to the Java/Processing `keyCode` used in `keyboardConfig.json`.
pub fn java_key_code(key: nannou::event::Key) -> Option<i32> {
    use nannou::event::Key::*;
    let letters = [
        A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    ];
    if let Some(i) = letters.iter().position(|k| *k == key) {
        return Some(65 + i as i32);
    }
    let digits = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
    if let Some(i) = digits.iter().position(|k| *k == key) {
        return Some(48 + i as i32);
    }
    let numpad = [
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    ];
    if let Some(i) = numpad.iter().position(|k| *k == key) {
        return Some(96 + i as i32);
    }
    let fkeys = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    if let Some(i) = fkeys.iter().position(|k| *k == key) {
        return Some(112 + i as i32);
    }
    Some(match key {
        Space => 32,
        Return => 10,
        Back => 8,
        Tab => 9,
        Left => 37,
        Up => 38,
        Right => 39,
        Down => 40,
        Comma => 44,
        Minus => 45,
        Period => 46,
        Slash => 47,
        Semicolon => 59,
        Equals => 61,
        LBracket => 91,
        Backslash => 92,
        RBracket => 93,
        Delete => 127,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_midi_do_not_collide() {
        let mut c = Controls::new(MidiOut::default());
        c.add_midi(1, 0, "cueVisualiser", 0);
        c.add_key(65, "selectVisualiserBykeypress", 1);
        assert_eq!(c.len(), 2);
        assert_eq!(
            c.get_mut(Controls::get_midi_key(1, 0))
                .unwrap()
                .function_name,
            "cueVisualiser"
        );
        assert_eq!(c.get_mut(2065).unwrap().value, 1);
    }

    #[test]
    fn java_key_codes() {
        use nannou::event::Key;
        assert_eq!(java_key_code(Key::A), Some(65));
        assert_eq!(java_key_code(Key::Key9), Some(57));
        assert_eq!(java_key_code(Key::Space), Some(32));
    }
}
