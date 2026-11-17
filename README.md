# GiantSpaceRobot Visualiser — Rust port

A visualiser for DJs that responds to the audio and to Traktor (tempo, deck volumes, filters,
effects and hotcues arrive as MIDI), controlled from a MIDI controller and the keyboard. Built in
Rust on [nannou](https://nannou.cc) 0.19 (wgpu).

This is a port of the Processing version,
[GiantSpaceRobotVisualiser](https://github.com/cjkcjk01/GiantSpaceRobotVisualiser), and is kept
as a separate project. The files in `data/` use the same formats as the Processing version:
`config.json`, the MIDI and keyboard mapping files, `dancemoves.json`, the hotcue packs, the
gradients and the GLSL shaders can be copied between the two.

## Building and running

You need a Rust toolchain (https://rustup.rs). On Linux you also need the ALSA headers for MIDI
and audio (`sudo apt install libasound2-dev pkg-config`).

```bash
cargo run --release
```

The `data/` directory is found automatically when you run from the repository root (or when it
sits next to, or a few folders above, the executable). Otherwise pass `--data /path/to/data` or
set `GSR_DATA`.

Useful options:

| Option | What it does |
| --- | --- |
| `--data <dir>` | Use a different data directory |
| `--windowed` | Ignore `"fullscreen": "true"` in config.json |
| `--set <action>=<value>` | Run an action at startup, e.g. `--set toggleWaveform=0` (repeatable) |
| `--demo-bpm <bpm>` | Generate beats without Traktor |
| `--test-signal` | Use a synthetic drum/bass signal instead of the line-in |
| `--screenshot <file.png> [--frames N]` | Save frame N (default 60) and exit |

For example, to try it with no Traktor, controller or audio interface:

```bash
cargo run --release -- --windowed --test-signal --demo-bpm 120
```

Then use the keyboard (`d`/`a` next/previous visualiser, `w` waveform, `4` kaleidoscope, `i` info,
`h` help and the rest of `keyboardConfig.json`). Set `RUST_LOG=debug` to log every key and
MIDI message.

## Setup

Setup is the same as for the Processing version: a virtual MIDI port for Traktor (e.g. Bome MIDI
Translator or loopMIDI), the Traktor TSI files from
https://maps.djtechtools.com/mappings/6883, and the device names in `config.json`'s `MIDIdevice` list (first
Traktor's port, then the controller). A device matches if its port name contains the configured
name. Audio comes from the system's default input device.

## What's ported

Everything that the sketch's `draw()` does:

* **Visualisers** (in the same order): Oblivion, Sprocket, CandyWarp, JazzUniverse, CircleTunnel,
  SpirographSpectrum, BeatLoop, Spectrum, Vis1 (waves), Bubbles, IsoHatching, OpArt and Orbs,
  with the same buttons/faders and audio reactions
* **Overlays**: waveform, kaleidoscope, hotcue image packs, beat words, background colour/palette
  cycling, dancing stick figures (all four formations, dance routines from `dancemoves.json`)
* **Post-processing shaders** (all 14) with their X/Y parameters and beat automation
* **Traktor effect shaders**: filter → blur, echo → Sobel, gater/slicer/masher → VHS glitch
* **Beat/tempo tracking**, auto-play, cue-up-then-switch for visualisers and word packs
* **MIDI** in from Traktor and the controller, LED colour/value feedback to the controller
* **Info, FPS, help and cue-info** screens

### Shaders

The GLSL files are not rewritten by hand. At load time `src/shader/glsl.rs` converts the
Processing-style GLSL (loose uniforms, `texture2D(texture, …)`, `gl_FragColor`, `varying`) into
Vulkan-style GLSL, which naga turns into WGSL. It also keeps OpenGL's bottom-left origin for
`gl_FragCoord` and texture coordinates, so shaders behave as they did in Processing. New shaders
can be dropped into `data/Shaders` exactly as before. A shader that fails to translate is logged
and skipped instead of stopping the program. `cargo test` checks that every shader the
visualiser uses translates.

## Differences from the Processing version

* **Live camera and video playback are not ported yet.** `src/video.rs` keeps the same controls
  (`videoToggle`, `videoParam*`), but reports the camera as unavailable. In the sketch these
  used GStreamer through `processing.video`; a Rust version could use `nokhwa` or `gstreamer`
  behind the same interface.
* **Spout output is not included.** It was commented out in the sketch as well, and its key
  (`toggleSpout`) toggled the help text there too, which is kept.
* **Fonts**: Processing's `.vlw` bitmap font can't be read. Put a `.ttf` or `.otf` file (e.g.
  Franklin Gothic Heavy) in `data/` and it is used; otherwise nannou's built-in Noto Sans is used.
* **Frame rate** follows the display's refresh rate (vsync) rather than `framerate` in
  config.json. Several animations advance per frame, as in the sketch, so on a 60 Hz display they
  run at the same speed as before.
* `Vis2` and `Vis3` were created in the sketch but never added to the visualiser list (they were
  copies of SpirographSpectrum), so they are not included.
* The config file's `toggleBackgroundBeatSync` and `videoBlendMode` key mappings refer to
  functions that don't exist in the sketch either; they are reported at startup and ignored.
* Sprocket's 3D boxes and spheres are lit and projected in software (matching Processing's
  default camera and `lights()`), then drawn as one depth-sorted mesh.

Some quirks of the original were kept on purpose so the visuals match. For example, Oblivion's
frequency weighting uses integer division and is always 4, Sprocket reads `getFreq(i)` with
the band index treated as Hz, and the stick figures ease toward each pose from their current
position. They are noted in comments where they occur.

## Code layout

| File | Processing equivalent |
| --- | --- |
| `main.rs` | `settings()`, `setup()`, window and CLI |
| `state.rs` | the globals, `Actions.pde`, `Triggers.pde` |
| `render.rs` | `draw()` |
| `config.rs` | `Config.pde` |
| `audio.rs`, `fft.rs`, `bpm.rs` | Minim line-in, `FFTAnalysis`, `BPM` |
| `midi.rs`, `controls.rs` | themidibus, `Controls.pde` |
| `decks.rs`, `hotcues.rs`, `overlays.rs` | `Decks.pde`, `HotCues.pde`, `Wordpacks.pde`, `bgPalette.pde`, `CueInfo.pde` |
| `post.rs` | `Shaders.pde` |
| `vis/` | `Visualisers.pde`, `StickFigures.pde` |
| `hud.rs` | `displayInfo()`, `displayHelp()`, `displayFPS()` |
| `gpu.rs`, `shader/glsl.rs` | `PGraphics`, `PShader`, `filter()` |
| `video.rs` | `Video.pde` (stub) |

## Controls

MIDI and keyboard controls are defined in `data/midiConfig.json` (controller),
`data/midiTraktorConfig.json` (messages from Traktor) and `data/keyboardConfig.json`. Each entry
names an action; the list of actions is `ACTIONS` in `src/state.rs`.

## Acknowledgements

### Oblivion

Copyright (c) 2016 Tobias Wehrum <Tobias.Wehrum@dragonlab.de>
Distributed under the MIT License. (See accompanying file LICENSE or copy at http://opensource.org/licenses/MIT)
This notice shall be included in all copies or substantial portions of the Software.
[https://blog.dragonlab.de/2016/04/into-oblivion-music-visualizer/](https://blog.dragonlab.de/2016/04/into-oblivion-music-visualizer/)
[https://github.com/TobiasWehrum/GenerativeArt/tree/master/s044_into_oblivion](https://github.com/TobiasWehrum/GenerativeArt/tree/master/s044_into_oblivion)

### Sprocket

[http://www.benfarahmand.com/2013/10/source-code-audio-visualizer-with.html](http://www.benfarahmand.com/2013/10/source-code-audio-visualizer-with.html)
[https://gist.github.com/benfarahmand/6902359](https://gist.github.com/benfarahmand/6902359)

### Candywarp

////////////////////////////////////////////////////////////  
// CandyWarp  by mojovideotech  
//  
// based on :  
// glslsandbox.com/e#38710.0  
// Posted by Trisomie21  
// modified by @hintz  
//  
// Creative Commons Attribution-NonCommercial-ShareAlike 3.0  
////////////////////////////////////////////////////////////

[https://editor.isf.video/shaders/5e7a802d7c113618206dec38](https://editor.isf.video/shaders/5e7a802d7c113618206dec38)

## Shader credits

### Sobel
From shadertoy [https://www.shadertoy.com/view/ldsSWr](https://www.shadertoy.com/view/ldsSWr)  
// Basic edge detection via convolution  
// Ken Slade - [ken.slade@gmail.com](mailto:ken.slade@gmail.com)  
// at https://www.shadertoy.com/view/ldsSWr
// Based on original Sobel shader by:  
// Jeroen Baert - jeroen.baert@cs.kuleuven.be (www.forceflow.be)  
// at https://www.shadertoy.com/view/Xdf3Rf

### VHS_glitch
[https://www.shadertoy.com/view/ldjGzV](https://www.shadertoy.com/view/ldjGzV)
https://www.shadertoy.com/user/ryk
Please respect ryk’s request to “please do not use this for commercial purposes.”

Post processing shaders
[https://genekogan.com/works/processing-shader-examples/](https://genekogan.com/works/processing-shader-examples/)

