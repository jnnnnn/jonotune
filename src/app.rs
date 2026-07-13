use crate::audio::AudioCapture;
use crate::pitch::PitchDetector;
use crate::spectrograph::Spectrograph;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct JonotuneApp {
    /// The most recent detected pitch in Hz (0.0 = no signal).
    pitch_hz: f32,
    /// Confidence of the most recent detection (0..1).
    pitch_confidence: f32,
    /// Smoothed microphone input level (0..1).
    #[serde(skip)]
    mic_level: f32,

    // ---- Non-serialized fields ----
    /// Platform audio capture backend (None until mic is opened).
    #[serde(skip)]
    audio: Option<Box<dyn AudioCapture>>,
    /// Pitch detector tuned to the capture sample rate.
    #[serde(skip)]
    detector: Option<PitchDetector>,
    /// Scrolling pitch-history widget.
    #[serde(skip)]
    spectrograph: Spectrograph,
    /// Accumulated recent samples for pitch detection.
    #[serde(skip)]
    sample_buf: Vec<f32>,
}

impl Default for JonotuneApp {
    fn default() -> Self {
        Self {
            pitch_hz: 0.0,
            pitch_confidence: 0.0,
            mic_level: 0.0,
            audio: None,
            detector: None,
            spectrograph: Spectrograph::new(256),
            sample_buf: Vec::new(),
        }
    }
}

impl JonotuneApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize look and feel if desired:
        // cc.egui_ctx.set_visuals(…);
        // cc.egui_ctx.set_fonts(…);

        // Load previous app state (if any).
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        // ---- Open the microphone ----
        // Native: synchronous.  Wasm: will need to be triggered by a button
        // (getUserMedia requires a user gesture), so we defer to the UI.
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.try_open_mic();
        }

        app
    }

    /// Attempt to open the default microphone and wire up the detector.
    fn try_open_mic(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let capture = crate::audio::create_audio_capture();
            if let Some(cap) = capture {
                let sr = cap.sample_rate();
                self.detector = Some(PitchDetector::new(sr));
                // Pre-allocate the sample buffer to the detector's required size.
                let min_samples = self.detector.as_ref().unwrap().min_samples();
                self.sample_buf = Vec::with_capacity(min_samples * 2);
                self.audio = Some(cap);
                log::info!("Microphone opened at {sr} Hz, min window: {min_samples} samples");
            } else {
                log::warn!("No microphone found");
            }
        }
    }

    /// Read a chunk of samples from the mic, run pitch detection, and push the
    /// result into the spectrograph history.
    ///
    /// Pushes exactly one frame per call, so the spectrograph scrolls smoothly
    /// at the UI frame rate regardless of how often pitch detections occur.
    /// When no pitch is detected, a silent frame (0 Hz) is pushed.
    fn process_audio(&mut self) {
        let Some(audio) = self.audio.as_mut() else {
            // No mic — push a silent frame so the trail keeps scrolling.
            self.push_frame(0.0, 0.0);
            return;
        };
        let Some(detector) = self.detector.as_ref() else {
            self.push_frame(0.0, 0.0);
            return;
        };

        // Read all available samples into a temporary buffer.
        let mut read_buf = vec![0.0f32; 2048];
        let n = audio.read_samples(&mut read_buf);
        read_buf.truncate(n);

        // Compute RMS level of incoming audio for the VU meter.
        if n > 0 {
            let sum_sq: f32 = read_buf.iter().map(|s| s * s).sum();
            let rms = (sum_sq / n as f32).sqrt();
            // Smooth the level display (fast attack, slow release).
            let alpha = if rms > self.mic_level { 0.3 } else { 0.05 };
            self.mic_level = alpha * rms + (1.0 - alpha) * self.mic_level;
            // Normalise to a rough 0..1 range (typical mic input is quiet).
            self.mic_level = (self.mic_level * 10.0).clamp(0.0, 1.0);
        }

        if n == 0 {
            self.push_frame(0.0, 0.0);
            return;
        }

        // Accumulate samples.
        self.sample_buf.extend_from_slice(&read_buf);

        let min_samples = detector.min_samples();

        // Run detection while we have enough samples.
        while self.sample_buf.len() >= min_samples {
            let window = &self.sample_buf[self.sample_buf.len() - min_samples..];
            let pitch = detector.detect(window);

            let hz = pitch.hz.unwrap_or(0.0);
            let conf = pitch.confidence;

            self.pitch_hz = hz;
            self.pitch_confidence = conf;

            // Discard processed samples (keep overlap of half the window
            // for smoother transitions between frames).
            let keep = min_samples / 2;
            let discard = self.sample_buf.len() - keep;
            self.sample_buf.drain(..discard);
        }

        // Cap buffer growth (avoid unbounded memory if something goes wrong).
        if self.sample_buf.len() > min_samples * 4 {
            let excess = self.sample_buf.len() - min_samples * 2;
            self.sample_buf.drain(..excess);
        }

        // Push exactly one frame per tick.  If we got a new detection we already
        // updated pitch_hz/pitch_confidence; otherwise we repeat the last state
        // so the trail stays connected.
        self.push_frame(self.pitch_hz, self.pitch_confidence);
    }

    /// Push a single (hz, confidence) frame into the spectrograph.
    fn push_frame(&mut self, hz: f32, confidence: f32) {
        self.spectrograph.push(hz, confidence);
    }
}

impl eframe::App for JonotuneApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Keep the UI alive continuously — without this, egui only repaints on input.
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        // ---- Process incoming audio ----
        self.process_audio();

        // ---- Top bar ----
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let is_web = cfg!(target_arch = "wasm32");
                if is_web {
                    // On wasm, we need a button to request mic access (user gesture).
                    if self.audio.is_none() && ui.button("🎤 Enable Microphone").clicked() {
                        // TODO: spawn async wasm::WasmAudio::new() via wasm_bindgen_futures
                        log::info!("Microphone button clicked (wasm)");
                    }
                } else if ui.button("🎤 Re-open Mic").clicked() {
                    self.try_open_mic();
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });

            // Pitch + level readout row.
            ui.horizontal(|ui| {
                // ---- VU meter ----
                let level_bar_w = 120.0;
                let level_h = 16.0;
                let (level_rect, _) = ui.allocate_exact_size(
                    egui::Vec2::new(level_bar_w, level_h),
                    egui::Sense::hover(),
                );
                let painter = ui.painter();
                // Background.
                painter.rect_filled(
                    level_rect,
                    2.0,
                    egui::Color32::from_gray(32),
                );
                // Fill.
                let fill_w = level_rect.width() * self.mic_level;
                if fill_w > 0.0 {
                    // Green → yellow → red gradient based on level.
                    let color = if self.mic_level < 0.5 {
                        egui::Color32::from_rgb(
                            (self.mic_level * 2.0 * 200.0) as u8,
                            180,
                            40,
                        )
                    } else if self.mic_level < 0.8 {
                        egui::Color32::from_rgb(
                            200,
                            ((1.0 - (self.mic_level - 0.5) * 3.33) * 180.0) as u8,
                            40,
                        )
                    } else {
                        egui::Color32::from_rgb(220, 60, 40)
                    };
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            level_rect.min,
                            egui::Vec2::new(fill_w, level_h),
                        ),
                        2.0,
                        color,
                    );
                }
                // Border.
                painter.rect_stroke(
                    level_rect,
                    2.0,
                    egui::Stroke::new(1.0f32, egui::Color32::from_gray(100)),
                    egui::StrokeKind::Inside,
                );
                // Label overlaid.
                painter.text(
                    level_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}%", self.mic_level * 100.0),
                    egui::FontId::monospace(10.0),
                    egui::Color32::from_gray(200),
                );

                ui.add_space(8.0);

                // ---- Pitch readout ----
                ui.label("Pitch:");
                if self.pitch_hz > 0.0 {
                    ui.label(
                        egui::RichText::new(format!("{:.1} Hz", self.pitch_hz))
                            .monospace(),
                    );
                    let note = hz_to_note_name(self.pitch_hz);
                    ui.label(
                        egui::RichText::new(format!("({note})"))
                            .strong(),
                    );
                } else {
                    ui.label("—");
                }
                ui.label(format!(
                    "  confidence: {:.0}%",
                    self.pitch_confidence * 100.0
                ));
            });
        });

        // ---- Spectrograph (main area) ----
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.spectrograph.ui(ui);
        });

        // ---- Bottom bar ----
        egui::Panel::bottom("bottom_panel").show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.audio.is_some() {
                    ui.label("🎤 Mic active");
                } else {
                    ui.label("🎤 No mic");
                }
                ui.separator();
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

/// Map a frequency in Hz to the nearest musical note name (e.g. "A4").
///
/// Uses A4 = 440 Hz, equal temperament.
fn hz_to_note_name(hz: f32) -> String {
    if hz <= 0.0 {
        return "—".into();
    }

    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    // MIDI note number: 69 = A4 = 440 Hz
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    let midi_rounded = midi.round() as i32;
    let note_idx = midi_rounded.rem_euclid(12) as usize;
    let octave = (midi_rounded / 12) - 1;

    format!("{}{}", note_names[note_idx], octave)
}
