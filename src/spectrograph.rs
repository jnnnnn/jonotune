//! Scrolling pitch-history widget.
//!
//! Renders a spectrograph-like view inside an egui `Ui`:
//! - **Y-axis** = frequency on a logarithmic scale (C3–C6, 3 octaves).
//! - **X-axis** = time (rightmost edge is "now"; older data scrolls left).
//! - Past ~5 seconds of pitch data is visible at once.
//! - Grid lines at every semitone, bolder at natural notes.
//! - Note labels along the left edge.
//! - Pitch trail with confidence-based opacity and age-based color fade.
//! - Current pitch marker at the right edge.
//! - Thin confidence bar at the bottom.

use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Lowest displayed frequency (C3).
const F_MIN: f32 = 130.81;
/// Highest displayed frequency (C6).
const F_MAX: f32 = 1046.5;
/// Height of the confidence bar in points.
const CONFIDENCE_BAR_HEIGHT: f32 = 16.0;
/// How wide each history frame is drawn, in points.
const FRAME_WIDTH: f32 = 3.0;

// ---------------------------------------------------------------------------
// Pitch trail colours
// ---------------------------------------------------------------------------

/// Bright amber — colour of the most recent pitch trail.
const COLOR_RECENT: Color32 = Color32::from_rgb(255, 200, 60);
/// Dimmed version for older entries.
const COLOR_OLD: Color32 = Color32::from_rgb(80, 60, 20);

// ---------------------------------------------------------------------------
// Spectrograph
// ---------------------------------------------------------------------------

/// Holds the ring buffer of past pitch measurements and draws them.
pub struct Spectrograph {
    /// Maximum number of history entries visible.
    history_len: usize,
    /// Ring buffer of recent pitch detections: `(hz, confidence)`.
    history: Vec<(f32, f32)>,
    /// Write cursor into `history`.
    cursor: usize,
}

impl Spectrograph {
    /// Create a new spectrograph widget.
    ///
    /// * `history_len` - number of pitch frames to keep.
    pub fn new(history_len: usize) -> Self {
        Self {
            history: vec![(0.0, 0.0); history_len],
            cursor: 0,
            history_len,
        }
    }

    /// Push a new pitch detection onto the history.
    ///
    /// * `hz` - detected frequency (0.0 if no pitch detected).
    /// * `confidence` - 0..1 confidence value.
    pub fn push(&mut self, hz: f32, confidence: f32) {
        self.history[self.cursor] = (hz, confidence);
        self.cursor = (self.cursor + 1) % self.history_len;
    }

    /// Draw the spectrograph into the given egui `Ui`.
    ///
    /// Should fill the available space.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let desired_size = ui.available_size();
        // Don't render if there's no space.
        if desired_size.x <= 0.0 || desired_size.y <= 0.0 {
            return;
        }

        let (rect, _response) =
            ui.allocate_exact_size(desired_size, egui::Sense::hover());

        // Split into graph area and confidence bar.
        let bar_rect = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() - CONFIDENCE_BAR_HEIGHT),
            Vec2::new(rect.width(), CONFIDENCE_BAR_HEIGHT),
        );
        let graph_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), bar_rect.top()),
        );

        let painter = ui.painter();

        // ---- 1. Background ----
        painter.rect_filled(graph_rect, 0.0, Color32::from_gray(18));

        // ---- 2. Grid lines & labels ----
        self.draw_grid(&painter, &graph_rect);

        // ---- 3. Pitch trail ----
        self.draw_trail(&painter, &graph_rect);

        // ---- 4. Current pitch marker ----
        self.draw_current_marker(&painter, &graph_rect);

        // ---- 5. Confidence bar ----
        self.draw_confidence_bar(&painter, &bar_rect);
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Map a frequency in Hz to a Y position within `graph_rect`.
    /// Higher frequency → smaller Y (closer to top).
    fn freq_to_y(freq: f32, rect: &Rect) -> f32 {
        if freq <= 0.0 {
            return rect.bottom();
        }
        let t = (freq / F_MIN).ln() / (F_MAX / F_MIN).ln();
        rect.bottom() - t.clamp(0.0, 1.0) * rect.height()
    }

    /// Map a history index (0 = oldest visible, len-1 = newest) to X.
    fn index_to_x(idx: usize, rect: &Rect, total: usize) -> f32 {
        let t = idx as f32 / (total.max(1) - 1) as f32;
        rect.left() + t * rect.width()
    }

    /// Draw semitone grid lines and note labels.
    fn draw_grid(&self, painter: &Painter, rect: &Rect) {
        let note_names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        // White-key (natural) indices: 0=C, 2=D, 4=E, 5=F, 7=G, 9=A, 11=B
        let is_natural = |midi: i32| -> bool {
            matches!(midi.rem_euclid(12), 0 | 2 | 4 | 5 | 7 | 9 | 11)
        };

        // MIDI note 48 = C3, 72 = C5 (but we go to C6 = 84, so display C3..C6).
        // Actually: C3 = 48, C6 = 84. That's 3 octaves.
        for midi in 48..=84 {
            let freq = 440.0 * 2.0f32.powf((midi - 69) as f32 / 12.0);
            let y = Self::freq_to_y(freq, rect);

            let (alpha, width) = if is_natural(midi) {
                (60, 1.0f32) // bolder for naturals
            } else {
                (30, 0.5f32) // subtle for sharps/flats
            };

            let color = Color32::from_gray(128).gamma_multiply(alpha as f32 / 255.0);
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(width, color),
            );

            // Label naturals on the left edge.
            if is_natural(midi) {
                let octave = midi / 12 - 1;
                let label = format!("{}{}", note_names[midi as usize % 12], octave);
                painter.text(
                    Pos2::new(rect.left() + 4.0, y - 4.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::monospace(10.0),
                    Color32::from_gray(160),
                );
            }
        }
    }

    /// Draw the scrolling pitch trail.
    fn draw_trail(&self, painter: &Painter, rect: &Rect) {
        let total = self.history_len;
        let mut prev_point: Option<(Pos2, f32, f32)> = None;
        // `pitch_hz` of the most recent entry (for age-based fade).
        // We don't have a "most recent" per se — we use the confidence itself
        // and the position to infer recency.

        for i in 0..total {
            // Read in ring-buffer order: oldest → newest.
            let idx = (self.cursor + i) % total;
            let (hz, confidence) = self.history[idx];

            let x = Self::index_to_x(i, rect, total);
            let y = Self::freq_to_y(hz, rect);

            // Age-based color: 0 = oldest, 1 = newest.
            let age_t = i as f32 / (total.max(1) - 1) as f32;
            let base_color = lerp_color(COLOR_OLD, COLOR_RECENT, age_t);

            // Apply confidence as alpha.
            let alpha = (confidence * 255.0) as u8;
            let color = Color32::from_rgba_premultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                alpha,
            );

            let point = Pos2::new(x, y);

            // Draw a small vertical dash at this frame.
            let dash_half = 2.0;
            painter.line_segment(
                [
                    Pos2::new(x, y - dash_half),
                    Pos2::new(x, y + dash_half),
                ],
                Stroke::new(FRAME_WIDTH, color),
            );

            // Connect to previous point if both have valid pitch.
            if let Some((prev_pos, prev_hz, prev_conf)) = prev_point {
                if hz > 0.0 && prev_hz > 0.0 {
                    // Blend the line color between the two points.
                    let prev_age_t = (i - 1) as f32 / (total.max(1) - 1) as f32;
                    let prev_color = lerp_color(COLOR_OLD, COLOR_RECENT, prev_age_t);
                    let prev_alpha = (prev_conf * 255.0) as u8;
                    let prev_rgba = Color32::from_rgba_premultiplied(
                        prev_color.r(),
                        prev_color.g(),
                        prev_color.b(),
                        prev_alpha,
                    );

                    // Use the newer point's color for the segment.
                    painter.line_segment(
                        [prev_pos, point],
                        Stroke::new(1.5f32, color),
                    );
                    // Suppress unused warning.
                    let _ = prev_rgba;
                }
            }

            prev_point = Some((point, hz, confidence));
        }
    }

    /// Draw a bright marker for the current (most recent) pitch.
    fn draw_current_marker(&self, painter: &Painter, rect: &Rect) {
        // The most recent entry is at index (cursor - 1) wrapping around.
        let newest_idx = (self.cursor + self.history_len - 1) % self.history_len;
        let (hz, confidence) = self.history[newest_idx];

        if hz <= 0.0 || confidence < 0.1 {
            return;
        }

        let y = Self::freq_to_y(hz, rect);
        let x = rect.right();

        // Glow: larger semi-transparent circle behind the dot.
        painter.circle_filled(
            Pos2::new(x, y),
            8.0,
            Color32::from_rgba_premultiplied(255, 200, 60, 80),
        );
        // Core dot.
        painter.circle_filled(
            Pos2::new(x, y),
            4.0,
            COLOR_RECENT,
        );
    }

    /// Draw the confidence bar at the bottom.
    fn draw_confidence_bar(&self, painter: &Painter, rect: &Rect) {
        // Background.
        painter.rect_filled(*rect, 0.0, Color32::from_gray(12));

        let total = self.history_len;

        for i in 0..total {
            let idx = (self.cursor + i) % total;
            let (_hz, confidence) = self.history[idx];

            let x = Self::index_to_x(i, rect, total);
            let bar_w = rect.width() / total as f32;

            // Green for high confidence, red for low.
            let color = Color32::from_rgb(
                ((1.0 - confidence) * 180.0) as u8,
                (confidence * 180.0) as u8,
                40,
            );

            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x - bar_w / 2.0, rect.top()),
                    Vec2::new(bar_w.max(1.0), rect.height()),
                ),
                0.0,
                color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Linear interpolation between two `Color32` values.
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}
