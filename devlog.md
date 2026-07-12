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
