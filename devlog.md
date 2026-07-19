# Dev Log — jonotune

A real-time pitch spectrograph for singing practice, built with egui/eframe.

---

## 2026-07-13 — Project Setup

> **Prompt:** I want to build a real-time spectrograph that shows a brief history of pitch to help me sing in tune. Some sort of spectrogram or whatever. I want to build it in egui so that I can deploy it as a web page. Find a good eframe template and get things set up with some git commits etc. Save all my prompts here to a file called "devlog.md".

### Setup done

- Cloned `eframe_template` (egui 0.34.1, eframe 0.34.1, Rust edition 2024).
- Renamed project to `jonotune`, updated window title & index.html.
- Added `cpal` 0.15 for native audio input (wasm will use Web Audio API later).
- Stripped template UI, added placeholder `JonotuneApp` with pitch state fields.
- Native build: ✅ compiles.  Web (wasm) check: ✅ compiles.
- `trunk` not yet installed (needed for `trunk serve` / web deployment).
- Commits:
  - `Initialize from eframe_template v0.34.1`
  - `Customize template: rename to jonotune, add cpal, strip UI`
  - `Update devlog with setup summary`

---

> **Prompt:** have set up trunk. now using rust 1.97.

### Done

- `trunk 0.21.14` installed.
- Bumped `rust-toolchain` from 1.92 → 1.97.
- Native build: ✅.  Web build (`trunk build`): ✅.

---

> **Prompt:** start with a skeleton and a few comments about what you will put in each spot.

### Architecture

Three new modules with skeleton code + TODO comments:

| File | Purpose |
|---|---|
| `src/audio.rs` | `AudioCapture` trait + `native` (cpal+ringbuf) and `wasm` (Web Audio API) backends |
| `src/pitch.rs` | `PitchDetector` — autocorrelation-based, with unit tests for sine wave & silence |
| `src/spectrograph.rs` | `Spectrograph` — ring-buffer history, egui widget with scrolling log-frequency view |
| `src/app.rs` | Wires mic → detector → spectrograph; top bar with pitch readout + note name |

- Added `ringbuf` 0.4 dependency (native audio ring buffer).
- `hz_to_note_name()` utility (e.g. 440 Hz → "A4").
- Native: ✅.  Wasm: ✅.

---

> **Prompt:** yep, get going

### Implemented

- **Native audio capture** (`audio.rs`): opens default mic via `cpal`, streams f32 samples into a lock-free `ringbuf`, UI thread drains each frame.
- **Pitch detection** (`pitch.rs`): YIN algorithm (squared difference + cumulative mean normalization + parabolic interpolation). 5 tests pass (A4=440 Hz, C4=262 Hz, E2=82 Hz, silence, noise).
- **App wiring** (`app.rs`): `process_audio()` drains mic samples into accumulator, runs YIN on overlapping windows, pushes (Hz, confidence) into spectrograph history.
- Switched from autocorrelation to YIN — handles pure tones and voice more robustly.
- Native: ✅ compiles & tests pass.  Wasm: ✅ compiles.

---

> **Prompt:** describe how it should look first  /  awesome, do it

### Spectrograph widget

Full implementation of `Spectrograph::ui()`:
- **Y-axis**: C3 (131 Hz) → C6 (1047 Hz), log scale, 3 octaves.
- **Grid**: thin lines at every semitone (C, C#, D, …), bolder lines at naturals (white keys).
- **Labels**: note names (C3, E4, A4, …) along the left edge.
- **Pitch trail**: connected line segments, amber/gold colour fading with age, opacity = confidence.
- **Silent gaps**: no line drawn when hz = 0.
- **Current marker**: bright glow dot at the right edge showing live pitch.
- **Confidence bar**: thin strip at the bottom, green=high, red=low.
- Ring-buffer push now works (wraps at `history_len`).

---

> **Prompt:** it's pretty jerky, it only moves a bit sometimes. add a level display for the mic and make it scroll continuously

### Fixes

- **Smooth scrolling**: `process_audio` now pushes exactly one frame per UI tick, even when no new detection occurs. Switched to `request_repaint_after(16ms)` for uninterrupted rendering.
- **VU meter**: painted level bar in the top panel (green→yellow→red gradient), with smoothed RMS from the mic input. Shows percentage overlay.
- Pitch readout uses monospace/strong styling for better readability.

---

> **Prompt:** mic level is pegged at 100%

### Fix

- VU meter now uses dB scale (RMS → dB, -48..0 dB mapped to 0..1), smoothed *after* scaling so the level doesn't saturate. Label shows dB instead of %.

---

> **Prompt:** that's working pretty well. can we also have some sort of dial or indicator that shows how close I am to the current predicted note ? like a +/- offset ? suggest designs and then implement

### Tuning indicator

Horizontal bar showing cents deviation from the nearest note:
- **Left label**: note one semitone below (e.g. "G#4")
- **Right label**: nearest note / target (e.g. "A4")
- **Colored zones**: green (±10¢ in tune), yellow (±25¢), red beyond
- **White dot**: current cents position
- **Cents text**: "+12¢" or "−8¢" above the dot, colored to match zone
- `hz_to_cents()` and `midi_to_note_name()` helpers extracted

---

> **Prompt:** having it disappear and reappear is bad. also the indicator bounces around a lot even if I hold the note.

### Fixes

- **Tuning bar always visible**: removed the `pitch_hz > 0 && confidence > 0.1` guard; bar now stays on screen at all times.
- **Exponential smoothing**: added `smooth_hz` and `smooth_confidence` fields with attack α=0.15 / release α=0.03, so the indicator doesn't jitter on steady notes.
- **Dim on silence**: `dim` multiplier (0.3 when confidence < 0.1) applied to background, zone colors, center line, labels, and cents text — the bar fades subtly instead of vanishing.
- **Dot alpha**: marker dot uses `Color32::from_rgba_premultiplied` with confidence-driven alpha; dot hidden entirely when `hz == 0`.
- Smoothed values fed into `draw_tuning_bar` instead of raw `pitch_hz` / `pitch_confidence`.

---

> **Prompt:** write a very practical and human readme

### Done

- Replaced the stock eframe_template README with a project-specific one.
- ASCII diagram of the UI, run instructions for native and web, what-works / what's-left list, architecture diagram, and tech stack summary.

---

> **Prompt:** have added a screenshot in doc, use that instead. remove tech stack and done/todo sections

### Done

- Replaced ASCII diagram with `doc/screen.png` screenshot.
- Removed "What works" / "What's still to do" and "Tech stack" sections — README is now tighter and more user-facing.

---

> **Prompt:** get the web version working

### Done

- Implemented `WasmAudio` backend: `getUserMedia` to `AudioContext` to
  `MediaStreamAudioSourceNode` to `AnalyserNode`. Pulls raw time-domain
  data each frame via `getByteTimeDomainData` and converts u8 to f32.
- Added web-sys features: AudioContext, AnalyserNode, MediaStream,
  MediaStreamConstraints, MediaStreamAudioSourceNode, MediaDevices,
  Navigator, Window, HtmlCanvasElement.
- Changed `audio` field to `Rc<RefCell<Option<Box<dyn AudioCapture>>>>`
  so the wasm async task can inject the capture after the UI starts.
- "Enable Microphone" button spawns `wasm_bindgen_futures::spawn_local`
  with the async mic-open; shows "Waiting for mic..." while pending.
- Detector set up lazily on first `process_audio` after async mic opens.
- Fixed API mismatches: `get_user_media_with_constraints` (not
  `get_user_media`), `set_audio` (not `audio`), cfg-gated wasm imports.
- Native + wasm both compile clean; `trunk build` succeeds; 5/5 tests
  pass.

---

> **Prompt:** Option<Rc<RefCell<Option<Box<dyn is pretty ugly, suggest some options to simplify / yup, do the mpsc

### Done

- Replaced `Rc<RefCell<Option<Box<dyn AudioCapture>>>>` with a plain
  `Option<Box<dyn AudioCapture>>` + a wasm-only mpsc channel.
- Button click creates a `std::sync::mpsc::channel()`, spawns the async
  task with the `Sender`, stores the `Receiver` in `audio_rx`.
- Each frame polls `rx.try_recv()`; when the result arrives, sets up
  the detector and stores the capture directly — no interior mutability.
- `process_audio` is back to simple `self.audio.as_mut()`.
- Removed `mic_active()` helper — just `.is_some()` now.

---

> **Prompt:** I would like it to only show a single octave, and wrap around when the octave changes. Some sort of octave indicator would be ok. I would also like to be able to tell exactly how in-tune I am from the graph. A piano keyboard on the right hand side that has bar graphs next to each key. When perfectly in tune, only one key should have the "confidence" as it's bar value. When we're off tune, the key(s) in that direction should have a little weighting. This way, as I wiggle around the note, I can see exactly how in tune I am. I want to be able to see clearly, so low values should still be quite visible.

### Design decisions

- **Octave folding**: `freq_to_y` now maps any frequency into a single C→B octave via MIDI-based folding. An "Oct 4" label appears bottom-right.
- **Piano keyboard**: 12 equal-height rows on the right side (white keys full-width light, black keys narrower dark), vertically aligned with the spectrograph grid.
- **Activation bars**: rainbow-coloured bars extend rightward from each key. 60¢ half-width triangular kernel, power-scaled (^0.4) so low activations remain visible.
- **Removed confidence bar**: trail now skips points below 5% confidence instead.
- Layout: graph (~65%) | keys (64px) | bars (120px).
- Native + wasm both compile; 5/5 tests pass.

---

> **Prompt:** fix GitHub Actions CI failures (ubuntu ALSA, rustfmt, clippy)

### Done

- **ALSA dependency**: Added `libasound2-dev` to the `check`, `test`, and
  `clippy` CI jobs — the `cpal` crate requires it on Linux for native audio.
- **Rustfmt**: Ran `cargo fmt` to fix indentation, import wrapping, and
  line-length issues in `audio.rs` and `pitch.rs`.
- **Clippy**: Fixed 15+ warnings:
  - Trait imports `as _` for anonymous usage (`audio.rs`)
  - `unwrap()` → `expect()` with messages (`app.rs`, `pitch.rs`)
  - Doc-comment backticks (`app.rs`, `spectrograph.rs`)
  - Redundant type annotations, `needless_range_loop` → iterator,
    `too_many_lines` → extracted `compute_cmnd` + `find_best_lag` (`pitch.rs`)
  - `&mut self` → `&self`, needless borrows, removed unused `&self` on
    associated fns, collapsed `if`, `f32::midpoint`, doc backticks
    (`spectrograph.rs`)
  - Renamed `clippy::empty_enum` → `empty_enums` (`Cargo.toml`)
- `cargo fmt --check`, `cargo clippy --all-features`, and `cargo test --lib`
  all pass cleanly.

---

## 2026-07-15 — Tuning reference control

> **Prompt:** Can we also add a control to let users tune to a different pitch?
> Suggest designs

### Implemented

- Added `tuning_hz` field to `JonotuneApp` (persisted via serde, default 440).
- Preset buttons in the top bar: **432 | 438 | 440 | 442 | 444** — one click.
  A `DragValue` alongside for custom values (400–500 Hz range).
- Threaded `tuning_hz` through all pitch-math:
  - `hz_to_cents()`, `hz_to_note_name()`, `draw_tuning_bar()` in `app.rs`
  - `freq_to_y()`, `freq_octave()`, `draw_trail()`, `draw_current_marker()`,
    `draw_keyboard_and_bars()` in `spectrograph.rs`
- All hardcoded 440.0 references are now parameterised.
- Native: ✅. Wasm: ✅. Trunk: ✅. Tests: 5/5.

---

> **Prompt:** When I play a C note it keeps wrapping really fast. Can we shift the
> graph up a half-semitone? That would also make the lines line up with the middle
> of the keys instead of the bottom.

### Fix

- Added +0.5 offset in `freq_to_y` before the octave fold modulo. Now the
  wrap boundary sits between B and C (instead of right on C), so crossing
  the octave boundary produces a ~0.002-unit jump instead of ~12 units.
- In-tune notes appear in the middle of their grid rows, centered between
  the two grid lines. Grid lines still mark note boundaries, aligning with
  the piano-key edges.
- Keyboard bars unaffected (the distance kernel is shift-invariant).
- Native: ✅. Wasm: ✅. Trunk: ✅. Tests: 5/5.

---

> **Prompt:** also fix the grid lines, and make the display show how far off the
> nearest gridline we are.

### Fix

- Moved grid lines from note boundaries to note centers (the same +0.5 offset
  used by the pitch dots).  Now draws 12 lines instead of 13 — one per
  semitone, running through the middle of each key row.
- A perfectly in-tune note lands exactly on its grid line.
- The vertical distance from the dot to the nearest line is proportional to
  cents deviation — you can see how out of tune you are directly on the graph.
- Natural notes get bold lines + labels; sharps/flats get thin lines only.
- Native: ✅. Wasm: ✅. Trunk: ✅. Tests: 5/5.

---

> **Prompt:** check that the gha deploy to jnnnnn.github.io/jonotune went ok

### CI fixes

- Deploy was fine; CI (`rust.yml`) was failing on `check`, `clippy`, and `test`
  due to missing `libasound2-dev` (required by cpal on Linux).
- Added `sudo apt-get update && sudo apt-get install libasound2-dev ...`
  to all three native jobs (`check`, `test`, `clippy`).
- Fixed two clippy warnings: extracted `draw_tuning_presets` helper to stay
  under `too_many_lines` limit, and wrapped `selectable_label` with `drop()`
  to satisfy `unused_must_use`.
- CI now all green: ✅ check, ✅ check_wasm, ✅ test, ✅ fmt, ✅ clippy.
- Pages deploy: ✅ at https://jnnnnn.github.io/jonotune/

---

## 2026-07-19 — Replace deprecated CI actions

> **Prompt:** Node.js 20 deprecation and `set-output` warnings in CI

### Fix

- Replaced deprecated `actions-rs/toolchain@v1` with `dtolnay/rust-toolchain@stable`.
- Replaced `actions-rs/cargo@v1` with direct `run: cargo ...` commands throughout
  `rust.yml` (check, test, fmt, clippy, check_wasm).
- Eliminates all Node.js 20 deprecation and `set-output` warnings.
- All 5 CI jobs remain green.
