# AGENTS.md — jonotune

A real-time pitch spectrograph for singing practice, built with egui/eframe.

## Build & Run

```sh
cargo build                    # native debug
cargo run                      # native
cargo test                     # 5 unit tests (pitch detector)
trunk build                    # wasm (output in dist/)
trunk serve                    # wasm dev server at http://127.0.0.1:8080/#dev
cargo check --target wasm32-unknown-unknown  # wasm check (fast)
```

## Architecture

```
src/
├── main.rs         # entry point, nothing interesting
├── lib.rs          # re-exports JonotuneApp
├── app.rs          # eframe::App impl, top bar, tuning bar, VU meter
├── audio.rs        # AudioCapture trait + native (cpal/ringbuf) + wasm (Web Audio)
├── pitch.rs        # YIN pitch detector with unit tests
└── spectrograph.rs # scrolling octave-folded trail + piano keyboard + bars
```

## Key patterns

- **Audio trait**: `AudioCapture` trait with `sample_rate()` and `read_samples()`.
  Native backend uses `cpal` + `ringbuf`. Wasm backend uses `AnalyserNode`.
- **Smooth state**: `smooth_hz` and `smooth_confidence` fields use exponential
  smoothing (attack α=0.15, release α=0.03). They feed the tuning bar and
  keyboard activation bars. Raw `pitch_hz`/`pitch_confidence` feed the
  spectrograph trail.
- **Spectrograph**: ring buffer (`history_len=256`), octave-folded via MIDI
  math. Keyboard bars use a 60¢ half-width triangular kernel with power-law
  scaling (^0.4) for low-value visibility.
- **Repaint**: `request_repaint_after(16ms)` keeps the display smooth.

## Platform gating

```rust
#[cfg(not(target_arch = "wasm32"))]  // native: cpal, sync mic open
#[cfg(target_arch = "wasm32")]       // wasm: web-sys, async mic open + mpsc
```

Always use `cfg` attr or `cfg!()` macro when touching audio, mic, or wasm
imports. Both targets must compile.

## Serde

`JonotuneApp` derives `Serialize/Deserialize` for state persistence. Fields
that can't be serialized (audio handles, detector, ring buffer, etc.) are
marked `#[serde(skip)]`.

## Dependencies

| crate | role |
|---|---|
| `egui` / `eframe` | UI framework |
| `cpal` | native audio input |
| `ringbuf` | lock-free ring buffer for audio thread→UI thread |
| `web-sys` | wasm Web Audio API (AudioContext, AnalyserNode, MediaStream) |
| `wasm-bindgen-futures` | async `getUserMedia` in wasm |
| `trunk` | wasm bundler/dev server |

## Workflow

- **Commit granularity**: commit after each meaningful change with a short
  imperative subject line. Group related edits into one commit.
- **devlog.md**: append each user prompt and a summary of what was done.
  Keep entries factual — what changed, which files, build status.
- **AGENTS.md**: keep this file current as the project evolves. It's the
  first thing an AI agent reads when picking up the project.
- **Validation**: after any code change, run `cargo build`, `cargo test`,
  and `trunk build` (or at minimum `cargo check --target wasm32-unknown-unknown`).
  Both native and wasm targets must compile. Mention results in devlog.
- **Service worker**: the `#dev` hash skips the SW during development.
  When changing filenames or rebuilding, unregister the old SW in browser
  DevTools → Application → Service Workers, or use a private window.
- **GitHub Pages**: push to `main` triggers `.github/workflows/pages.yml` →
  builds with trunk → deploys `dist/` to `gh-pages` branch via
  `JamesIves/github-pages-deploy-action`. The site appears at
  `https://<user>.github.io/jonotune/`. The repo Settings → Pages must
  point to the `gh-pages` branch.
