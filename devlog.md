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
