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
    /// at the UI frame rate.
    fn process_audio(&mut self) {
        let Some(audio) = self.audio.as_mut() else {
            self.push_frame(0.0, 0.0);
            return;
        };
        let Some(detector) = self.detector.as_ref() else {
            self.push_frame(0.0, 0.0);
            return;
        };

        let mut read_buf = vec![0.0f32; 2048];
        let n = audio.read_samples(&mut read_buf);
        read_buf.truncate(n);

        // Compute RMS level of incoming audio for the VU meter.
        if n > 0 {
            let sum_sq: f32 = read_buf.iter().map(|s| s * s).sum();
            let rms = (sum_sq / n as f32).sqrt();
            let db = 20.0 * (rms + 1e-10f32).log10();
            let level = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
            let alpha = if level > self.mic_level { 0.6 } else { 0.08 };
            self.mic_level = alpha * level + (1.0 - alpha) * self.mic_level;
        }

        if n == 0 {
            self.push_frame(0.0, 0.0);
            return;
        }

        self.sample_buf.extend_from_slice(&read_buf);

        let min_samples = detector.min_samples();

        while self.sample_buf.len() >= min_samples {
            let window = &self.sample_buf[self.sample_buf.len() - min_samples..];
            let pitch = detector.detect(window);

            let hz = pitch.hz.unwrap_or(0.0);
            let conf = pitch.confidence;

            self.pitch_hz = hz;
            self.pitch_confidence = conf;

            let keep = min_samples / 2;
            let discard = self.sample_buf.len() - keep;
            self.sample_buf.drain(..discard);
        }

        if self.sample_buf.len() > min_samples * 4 {
            let excess = self.sample_buf.len() - min_samples * 2;
            self.sample_buf.drain(..excess);
        }

        self.push_frame(self.pitch_hz, self.pitch_confidence);
    }

    fn push_frame(&mut self, hz: f32, confidence: f32) {
        self.spectrograph.push(hz, confidence);
    }
}

impl eframe::App for JonotuneApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));

        self.process_audio();

        // ---- Top bar ----
        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let is_web = cfg!(target_arch = "wasm32");
                if is_web {
                    if self.audio.is_none() && ui.button("🎤 Enable Microphone").clicked() {
                        log::info!("Microphone button clicked (wasm)");
                    }
                } else if ui.button("🎤 Re-open Mic").clicked() {
                    self.try_open_mic();
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });

            // Row 1: VU meter + pitch + confidence.
            ui.horizontal(|ui| {
                draw_vu_meter(ui, self.mic_level);

                ui.add_space(8.0);

                ui.label("Pitch:");
                if self.pitch_hz > 0.0 {
                    ui.label(
                        egui::RichText::new(format!("{:.1} Hz", self.pitch_hz))
                            .monospace(),
                    );
                    let note = hz_to_note_name(self.pitch_hz);
                    ui.label(egui::RichText::new(format!("({note})")).strong());
                } else {
                    ui.label("—");
                }
                ui.label(format!(
                    "  confidence: {:.0}%",
                    self.pitch_confidence * 100.0
                ));
            });

            // Row 2: tuning indicator (only when a pitch is detected).
            if self.pitch_hz > 0.0 && self.pitch_confidence > 0.1 {
                ui.add_space(4.0);
                draw_tuning_bar(ui, self.pitch_hz, self.pitch_confidence);
            }
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

// ---------------------------------------------------------------------------
// VU meter
// ---------------------------------------------------------------------------

fn draw_vu_meter(ui: &mut egui::Ui, level: f32) {
    let bar_w = 120.0;
    let bar_h = 16.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(bar_w, bar_h),
        egui::Sense::hover(),
    );
    let painter = ui.painter();

    painter.rect_filled(rect, 2.0, egui::Color32::from_gray(32));

    let fill_w = rect.width() * level;
    if fill_w > 0.0 {
        let color = if level < 0.5 {
            egui::Color32::from_rgb((level * 2.0 * 200.0) as u8, 180, 40)
        } else if level < 0.8 {
            egui::Color32::from_rgb(200, ((1.0 - (level - 0.5) * 3.33) * 180.0) as u8, 40)
        } else {
            egui::Color32::from_rgb(220, 60, 40)
        };
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::Vec2::new(fill_w, bar_h)),
            2.0,
            color,
        );
    }

    painter.rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0f32, egui::Color32::from_gray(100)),
        egui::StrokeKind::Inside,
    );

    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:.0} dB", (level * 48.0) - 48.0),
        egui::FontId::monospace(10.0),
        egui::Color32::from_gray(200),
    );
}

// ---------------------------------------------------------------------------
// Tuning bar
// ---------------------------------------------------------------------------

/// Draw a horizontal bar showing cents deviation from the nearest note.
///
/// ```
///  G#4  [——●——————]  A4    +12¢
///        -50   0   +50
/// ```
fn draw_tuning_bar(ui: &mut egui::Ui, hz: f32, _confidence: f32) {
    let (nearest_midi, cents) = hz_to_cents(hz);
    let target_name = midi_to_note_name(nearest_midi);
    let lower_name = midi_to_note_name(nearest_midi - 1);

    let bar_w = 240.0;
    let bar_h = 22.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(bar_w, bar_h),
        egui::Sense::hover(),
    );
    let painter = ui.painter();

    // Background.
    painter.rect_filled(rect, 3.0, egui::Color32::from_gray(24));

    // Coloured zones (left = flat, right = sharp).
    let cx = rect.center().x;
    let half_w = rect.width() / 2.0;
    // Each cent = half_w / 50 pixels.
    let cents_to_px = |c: f32| -> f32 { (c / 50.0) * half_w };

    // Green zone (±10¢).
    let green_left = (cx + cents_to_px(-10.0)).max(rect.left());
    let green_right = (cx + cents_to_px(10.0)).min(rect.right());
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::Pos2::new(green_left, rect.top() + 2.0),
            egui::Pos2::new(green_right, rect.bottom() - 2.0),
        ),
        0.0,
        egui::Color32::from_rgb(40, 160, 60),
    );

    // Yellow zones (±10-25¢).
    let yellow_l = (cx + cents_to_px(-25.0)).max(rect.left());
    let yellow_r = (cx + cents_to_px(25.0)).min(rect.right());
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::Pos2::new(yellow_l, rect.top() + 2.0),
            egui::Pos2::new(green_left, rect.bottom() - 2.0),
        ),
        0.0,
        egui::Color32::from_rgb(180, 140, 30),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::Pos2::new(green_right, rect.top() + 2.0),
            egui::Pos2::new(yellow_r, rect.bottom() - 2.0),
        ),
        0.0,
        egui::Color32::from_rgb(180, 140, 30),
    );

    // Center line.
    painter.line_segment(
        [egui::Pos2::new(cx, rect.top()), egui::Pos2::new(cx, rect.bottom())],
        egui::Stroke::new(1.5f32, egui::Color32::from_gray(180)),
    );

    // Marker dot.
    let dot_x = (cx + cents_to_px(cents)).clamp(rect.left() + 4.0, rect.right() - 4.0);
    let dot_y = rect.center().y;
    painter.circle_filled(
        egui::Pos2::new(dot_x, dot_y),
        4.0,
        egui::Color32::WHITE,
    );

    // Labels.
    let font = egui::FontId::monospace(11.0);
    painter.text(
        egui::Pos2::new(rect.left() + 4.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &lower_name,
        font.clone(),
        egui::Color32::from_gray(140),
    );
    painter.text(
        egui::Pos2::new(rect.right() - 4.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        &target_name,
        font.clone(),
        egui::Color32::from_gray(220),
    );

    // Cents offset text (coloured: green in zone, red outside).
    let cents_color = if cents.abs() < 10.0 {
        egui::Color32::from_rgb(80, 220, 80)
    } else if cents.abs() < 25.0 {
        egui::Color32::from_rgb(220, 200, 80)
    } else {
        egui::Color32::from_rgb(240, 80, 80)
    };
    let sign = if cents >= 0.0 { "+" } else { "" };
    let cents_text = format!("{sign}{cents:.0}¢");
    painter.text(
        egui::Pos2::new(dot_x, rect.top() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        cents_text,
        egui::FontId::monospace(10.0),
        cents_color,
    );
}

// ---------------------------------------------------------------------------
// Note / frequency helpers
// ---------------------------------------------------------------------------

/// Returns (nearest_midi_note, cents_offset).
///
/// Positive cents = sharp, negative = flat.
/// A4 = MIDI 69 = 440 Hz.
fn hz_to_cents(hz: f32) -> (i32, f32) {
    if hz <= 0.0 {
        return (69, 0.0);
    }
    let midi_f = 69.0 + 12.0 * (hz / 440.0).log2();
    let nearest = midi_f.round() as i32;
    let cents = 100.0 * (midi_f - nearest as f32);
    (nearest, cents)
}

/// Convert a MIDI note number to a name like "A4" or "C#3".
fn midi_to_note_name(midi: i32) -> String {
    let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let idx = midi.rem_euclid(12) as usize;
    let octave = midi / 12 - 1;
    format!("{}{}", names[idx], octave)
}

/// Map a frequency in Hz to the nearest musical note name (e.g. "A4").
fn hz_to_note_name(hz: f32) -> String {
    if hz <= 0.0 {
        return "—".into();
    }
    let (midi, _) = hz_to_cents(hz);
    midi_to_note_name(midi)
}
