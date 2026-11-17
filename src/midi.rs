//! MIDI I/O (replaces themidibus).
//!
//! Two devices are used, as named in `config.json`'s `MIDIdevice` list: the first is Traktor's
//! output (through a virtual port such as Bome's "BMT 1" or loopMIDI), the second is the hardware
//! controller (e.g. a Midi Fighter Twister). Incoming messages are forwarded over a channel to the
//! main thread; CC messages can be sent back to the controller to update its LEDs.

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEvent {
    ControlChange {
        channel: i32,
        number: i32,
        value: i32,
    },
    NoteOn {
        channel: i32,
        pitch: i32,
        velocity: i32,
    },
    NoteOff {
        channel: i32,
        pitch: i32,
        velocity: i32,
    },
}

impl MidiEvent {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let (&status, rest) = bytes.split_first()?;
        let channel = (status & 0x0F) as i32;
        let d1 = *rest.first()? as i32;
        let d2 = rest.get(1).copied().unwrap_or(0) as i32;
        match status & 0xF0 {
            0xB0 => Some(MidiEvent::ControlChange {
                channel,
                number: d1,
                value: d2,
            }),
            0x90 if d2 > 0 => Some(MidiEvent::NoteOn {
                channel,
                pitch: d1,
                velocity: d2,
            }),
            0x90 | 0x80 => Some(MidiEvent::NoteOff {
                channel,
                pitch: d1,
                velocity: d2,
            }),
            _ => None,
        }
    }
}

/// Shared handle used to send CCs to the controller (LED feedback). Cheap to clone; does nothing
/// when no controller is connected.
#[derive(Clone, Default)]
pub struct MidiOut(Arc<Mutex<Option<MidiOutputConnection>>>);

impl MidiOut {
    pub fn send_controller_change(&self, channel: i32, number: i32, value: i32) {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(conn) = guard.as_mut() {
                let msg = [
                    0xB0 | (channel.clamp(0, 15) as u8),
                    number.clamp(0, 127) as u8,
                    value.clamp(0, 127) as u8,
                ];
                if let Err(e) = conn.send(&msg) {
                    log::warn!("MIDI send failed: {e}");
                }
            }
        }
    }
}

pub struct Midi {
    pub events: Receiver<MidiEvent>,
    pub controller_out: MidiOut,
    pub traktor_connected: bool,
    pub controller_connected: bool,
    _inputs: Vec<MidiInputConnection<()>>,
}

fn list_inputs() -> Vec<String> {
    match MidiInput::new("gsr-list") {
        Ok(mi) => mi
            .ports()
            .iter()
            .filter_map(|p| mi.port_name(p).ok())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn connect_input(name: &str, tx: Sender<MidiEvent>) -> Option<MidiInputConnection<()>> {
    let mi = MidiInput::new("gsr-visualiser").ok()?;
    let port = mi
        .ports()
        .into_iter()
        .find(|p| mi.port_name(p).map(|n| n.contains(name)).unwrap_or(false))?;
    match mi.connect(
        &port,
        "gsr-in",
        move |_stamp, bytes, _| {
            if let Some(ev) = MidiEvent::parse(bytes) {
                let _ = tx.send(ev);
            }
        },
        (),
    ) {
        Ok(c) => Some(c),
        Err(e) => {
            log::warn!("Could not open MIDI input '{name}': {e}");
            None
        }
    }
}

fn connect_output(name: &str) -> Option<MidiOutputConnection> {
    let mo = MidiOutput::new("gsr-visualiser").ok()?;
    let port = mo
        .ports()
        .into_iter()
        .find(|p| mo.port_name(p).map(|n| n.contains(name)).unwrap_or(false))?;
    match mo.connect(&port, "gsr-out") {
        Ok(c) => Some(c),
        Err(e) => {
            log::warn!("Could not open MIDI output '{name}': {e}");
            None
        }
    }
}

impl Midi {
    pub fn open(devices: &[String]) -> Self {
        let (tx, rx) = channel();
        let mut midi = Midi {
            events: rx,
            controller_out: MidiOut::default(),
            traktor_connected: false,
            controller_connected: false,
            _inputs: Vec::new(),
        };

        let available = list_inputs();
        log::info!("Available MIDI inputs: {available:?}");

        if devices.is_empty() {
            log::warn!("No Midi device name found in config.json file. Use in non-Midi mode.");
            return midi;
        }
        if devices.len() > 2 {
            log::warn!(
                "Too many Midi devices, there should only be two; Traktor's and the controller's. \
                 Use in non-Midi mode."
            );
            return midi;
        }

        if let Some(traktor) = devices.first() {
            if let Some(conn) = connect_input(traktor, tx.clone()) {
                log::info!("Added Midi Traktor device - {traktor}");
                midi._inputs.push(conn);
                midi.traktor_connected = true;
            } else {
                log::warn!("Traktor MIDI device '{traktor}' not found");
            }
        }
        if let Some(controller) = devices.get(1) {
            if let Some(conn) = connect_input(controller, tx) {
                log::info!("Added Midi Controller device - {controller}");
                midi._inputs.push(conn);
                midi.controller_connected = true;
                if let Some(out) = connect_output(controller) {
                    *midi.controller_out.0.lock().unwrap() = Some(out);
                }
            } else {
                log::warn!("Controller MIDI device '{controller}' not found");
            }
        }
        midi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cc() {
        assert_eq!(
            MidiEvent::parse(&[0xBA, 100, 64]),
            Some(MidiEvent::ControlChange {
                channel: 10,
                number: 100,
                value: 64
            })
        );
        assert_eq!(
            MidiEvent::parse(&[0x91, 60, 0]),
            Some(MidiEvent::NoteOff {
                channel: 1,
                pitch: 60,
                velocity: 0
            })
        );
        assert_eq!(MidiEvent::parse(&[0xF8]), None);
    }
}
