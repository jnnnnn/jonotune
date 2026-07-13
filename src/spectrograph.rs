//! Scrolling pitch-history widget with folded-octave view and piano-keyboard
//! confidence bars.
//!
//! Renders a spectrograph-like view inside an egui `Ui`:
//! - **Y-axis** = single octave (C → B), all octaves folded into one.
//! - **X-axis** = time (rightmost edge is "now"; older data scrolls left).
//! - Past ~5 seconds of pitch data is visible at once.
//! - Grid lines at every semitone, bolder at natural notes.
//! - Note labels along the left edge.
//! - Pitch trail, skipping low-confidence frames.
//! - Current pitch marker at the right edge of the graph.
//! - Piano-keyboard strip on the right with rainbow activation bars per note.

use egui::{Color32, Painter, Pos2, Rect, Stroke};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How wide each history frame is drawn, in points.
const FRAME_WIDTH: f32 = 3.0;
/// Width of the piano-key strip, in points.
const KEYBOARD_WIDTH: f32 = 64.0;
/// Width of the activation bar area to the right of the keys.
const BAR_WIDTH: f32 = 120.0;
/// Minimum confidence to draw a trail point or connect across.
const MIN_TRAIL_CONFIDENCE: f32 = 0.05;

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

    /// Draw the spectrograph, keyboard, and bars into the given egui `Ui`.
    ///
    /// * `smooth_hz` - smoothed current pitch for the activation bars.
    /// * `smooth_confidence` - smoothed confidence for bar activation.
    pub fn ui(&mut self, ui: &mut egui::Ui, smooth_hz: f32, smooth_confidence: f32) {
        let desired_size = ui.available_size();
        if desired_size.x <= 0.0 || desired_size.y <= 0.0 {
            return;
        }

        let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

        // Reserve space for keyboard + bars on the right.
        let right_margin = KEYBOARD_WIDTH + BAR_WIDTH;
        let graph_rect = if rect.width() > right_margin + 40.0 {
            Rect::from_min_max(
                rect.min,
                Pos2::new(rect.right() - right_margin, rect.bottom()),
            )
        } else {
            rect
        };
        let keys_rect = Rect::from_min_max(
            Pos2::new(graph_rect.right(), rect.top()),
            Pos2::new(graph_rect.right() + KEYBOARD_WIDTH, rect.bottom()),
        );
        let bars_rect = Rect::from_min_max(
            Pos2::new(keys_rect.right(), rect.top()),
            Pos2::new(keys_rect.right() + BAR_WIDTH, rect.bottom()),
        );

        let painter = ui.painter();

        // ---- 1. Graph background ----
        painter.rect_filled(graph_rect, 0.0, Color32::from_gray(18));

        // ---- 2. Grid lines & labels ----
        self.draw_grid(&painter, &graph_rect);

        // ---- 3. Pitch trail ----
        self.draw_trail(&painter, &graph_rect);

        // ---- 4. Current pitch marker ----
        self.draw_current_marker(&painter, &graph_rect);

        // ---- 5. Piano keyboard + activation bars ----
        self.draw_keyboard_and_bars(
            &painter,
            &keys_rect,
            &bars_rect,
            smooth_hz,
            smooth_confidence,
        );
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Map a frequency in Hz to a Y position within `rect`, folding all octaves
    /// into the C4–C5 range.  Higher frequency → smaller Y (closer to top).
    fn freq_to_y(freq: f32, rect: &Rect) -> f32 {
        if freq <= 0.0 {
            return rect.bottom();
        }
        // Fold into the C4 (MIDI 60) octave using log₂, then map linearly in MIDI space.
        let midi_f = 69.0 + 12.0 * (freq / 440.0).log2();
        let folded = ((midi_f - 60.0) % 12.0 + 12.0) % 12.0; // 0..12  (C → B)
        let t = folded / 12.0;
        rect.bottom() - t.clamp(0.0, 1.0) * rect.height()
    }

    /// Return the octave number for a frequency (C4 = octave 4).
    pub fn freq_octave(freq: f32) -> Option<i32> {
        if freq <= 0.0 {
            return None;
        }
        let midi_f = 69.0 + 12.0 * (freq / 440.0).log2();
        let midi = midi_f.round() as i32;
        Some(midi / 12 - 1)
    }

    /// Map a history index (0 = oldest visible, len-1 = newest) to X.
    fn index_to_x(idx: usize, rect: &Rect, total: usize) -> f32 {
        let t = idx as f32 / (total.max(1) - 1) as f32;
        rect.left() + t * rect.width()
    }

    /// Draw semitone grid lines and note labels for a single octave.
    fn draw_grid(&self, painter: &Painter, rect: &Rect) {
        let note_names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let is_natural = |i: i32| -> bool { matches!(i, 0 | 2 | 4 | 5 | 7 | 9 | 11) };

        // Draw 13 lines: 12 semitone boundaries + the top (C of next octave).
        for i in 0..=12 {
            // i = 0 → C, i = 1 → C#, ..., i = 11 → B, i = 12 → C (top)
            let t = i as f32 / 12.0;
            let y = rect.bottom() - t * rect.height();

            let note_idx = i % 12; // i=12 wraps to 0 (C)
            let (alpha, width) = if is_natural(note_idx) {
                (80, 1.0f32)
            } else {
                (35, 0.5f32)
            };

            let color = Color32::from_gray(128).gamma_multiply(alpha as f32 / 255.0);
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(width, color),
            );

            // Label naturals on the left edge.
            if is_natural(note_idx) && i < 12 {
                painter.text(
                    Pos2::new(rect.left() + 4.0, y - 4.0),
                    egui::Align2::LEFT_CENTER,
                    note_names[note_idx as usize],
                    egui::FontId::monospace(11.0),
                    Color32::from_gray(160),
                );
            }
        }
    }

    /// Draw the scrolling pitch trail, skipping low-confidence frames.
    fn draw_trail(&self, painter: &Painter, rect: &Rect) {
        let total = self.history_len;
        let mut prev_point: Option<(Pos2, f32)> = None; // (pos, hz)

        for i in 0..total {
            let idx = (self.cursor + i) % total;
            let (hz, confidence) = self.history[idx];

            // Skip low-confidence / silent frames.
            if hz <= 0.0 || confidence < MIN_TRAIL_CONFIDENCE {
                prev_point = None; // break the trail
                continue;
            }

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
                [Pos2::new(x, y - dash_half), Pos2::new(x, y + dash_half)],
                Stroke::new(FRAME_WIDTH, color),
            );

            // Connect to previous point if we had one (already checked confidence above).
            if let Some((prev_pos, prev_hz)) = prev_point {
                if hz > 0.0 && prev_hz > 0.0 {
                    painter.line_segment([prev_pos, point], Stroke::new(1.5f32, color));
                }
            }

            prev_point = Some((point, hz));
        }
    }

    /// Draw a bright marker for the current (most recent) pitch.
    fn draw_current_marker(&self, painter: &Painter, rect: &Rect) {
        let newest_idx = (self.cursor + self.history_len - 1) % self.history_len;
        let (hz, confidence) = self.history[newest_idx];

        if hz <= 0.0 || confidence < 0.1 {
            return;
        }

        let y = Self::freq_to_y(hz, rect);
        let x = rect.right();

        // Glow.
        painter.circle_filled(
            Pos2::new(x, y),
            8.0,
            Color32::from_rgba_premultiplied(255, 200, 60, 80),
        );
        // Core dot.
        painter.circle_filled(Pos2::new(x, y), 4.0, COLOR_RECENT);
    }

    /// Draw the piano-key strip (keys_rect) and per-note activation bars (bars_rect).
    fn draw_keyboard_and_bars(
        &self,
        painter: &Painter,
        keys_rect: &Rect,
        bars_rect: &Rect,
        smooth_hz: f32,
        smooth_confidence: f32,
    ) {
        let note_names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let is_white = |i: i32| -> bool { matches!(i, 0 | 2 | 4 | 5 | 7 | 9 | 11) };

        let row_h = keys_rect.height() / 12.0;
        // Use 60¢ half-width so neighbouring notes get ~33% activation at 50¢ offset.
        let kernel_width: f32 = 60.0;

        // Compute the folded chromatic position of the current pitch (0..12).
        let chroma = if smooth_hz > 0.0 && smooth_confidence >= 0.1 {
            let midi_f = 69.0 + 12.0 * (smooth_hz / 440.0).log2();
            ((midi_f - 60.0) % 12.0 + 12.0) % 12.0
        } else {
            -1.0 // sentinel: no valid pitch
        };

        // Background behind bars.
        painter.rect_filled(*bars_rect, 0.0, Color32::from_gray(14));

        for i in 0i32..12 {
            // Row i=0 is C (bottom), i=11 is B (top).
            let row_top = keys_rect.bottom() - (i + 1) as f32 * row_h;
            let row_bottom = keys_rect.bottom() - i as f32 * row_h;

            // ---- Piano key ----
            if is_white(i) {
                // White key: full width, light.
                let key_rect = Rect::from_min_max(
                    Pos2::new(keys_rect.left() + 1.0, row_top),
                    Pos2::new(keys_rect.right() - 1.0, row_bottom),
                );
                painter.rect_filled(key_rect, 1.0, Color32::from_gray(215));
                painter.rect_stroke(
                    key_rect,
                    1.0,
                    Stroke::new(0.5f32, Color32::from_gray(140)),
                    egui::StrokeKind::Inside,
                );
            } else {
                // Black key: narrower, dark, inset from edges.
                let inset = keys_rect.width() * 0.25;
                let key_rect = Rect::from_min_max(
                    Pos2::new(keys_rect.left() + inset, row_top),
                    Pos2::new(keys_rect.right() - inset, row_bottom),
                );
                painter.rect_filled(key_rect, 1.0, Color32::from_gray(50));
                painter.rect_stroke(
                    key_rect,
                    1.0,
                    Stroke::new(0.5f32, Color32::from_gray(80)),
                    egui::StrokeKind::Inside,
                );
            }

            // ---- Activation bar ----
            let activation = if chroma >= 0.0 {
                // Shortest circular distance on the chromatic circle.
                let raw_dist = (chroma - i as f32).abs();
                let dist = raw_dist.min(12.0 - raw_dist); // wrap around
                let cents_dist = dist * 100.0;
                (1.0 - cents_dist / kernel_width).max(0.0) * smooth_confidence
            } else {
                0.0
            };

            // Power-law scaling so low values are still visible.
            let display = activation.powf(0.4);

            let bar_color = hsv_to_rgb(i as f32 / 12.0, 0.75, 0.85);

            let max_bar_w = bars_rect.width() - 4.0;
            let bar_w = display * max_bar_w;
            if bar_w > 0.5 {
                let bar_rect = Rect::from_min_max(
                    Pos2::new(bars_rect.left() + 2.0, row_top + 1.0),
                    Pos2::new(bars_rect.left() + 2.0 + bar_w, row_bottom - 1.0),
                );
                painter.rect_filled(bar_rect, 1.0, bar_color);
            }

            // ---- Note label inside the key (white keys only, or all?) ----
            // Label white keys inside the key area.
            if is_white(i) {
                painter.text(
                    Pos2::new(keys_rect.left() + 6.0, (row_top + row_bottom) / 2.0),
                    egui::Align2::LEFT_CENTER,
                    note_names[i as usize],
                    egui::FontId::monospace(9.0),
                    Color32::from_gray(80),
                );
            }
        }

        // ---- Octave indicator ----
        if let Some(octave) = Self::freq_octave(smooth_hz) {
            let label = format!("Oct {octave}");
            painter.text(
                Pos2::new(bars_rect.right() - 4.0, bars_rect.bottom() - 4.0),
                egui::Align2::RIGHT_BOTTOM,
                label,
                egui::FontId::proportional(12.0),
                Color32::from_gray(160),
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

/// Convert HSV to RGB.  h ∈ [0, 1], s ∈ [0, 1], v ∈ [0, 1].
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let h_prime = h * 6.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color32::from_rgb(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}
