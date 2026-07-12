//! Scrolling pitch-history widget.
//!
//! Renders a spectrograph-like view inside an egui `Ui`:
//! - **X-axis** = time (rightmost edge is "now"; older data scrolls left).
//! - **Y-axis** = frequency on a logarithmic scale (piano-roll style).
//! - Past ~4 seconds of pitch data is visible at once.

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
    /// * `history_len` - number of pitch frames to keep (e.g. 200 for ~4 s at 50 fps).
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
        // TODO: ring-buffer push — write at cursor, advance, wrap
        let _ = (hz, confidence);
    }

    /// Draw the spectrograph into the given egui `Ui`.
    ///
    /// Should fill the available space. The caller is responsible for putting
    /// this inside a panel / frame with the desired size.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // TODO: get the painter and response
        // TODO: draw background
        // TODO: draw frequency grid lines (piano key reference lines?)
        // TODO: iterate history ring-buffer and paint each frame:
        //       - X position = time offset from "now"
        //       - Y position = log(freq) mapped to widget height
        //       - colour intensity = confidence (brighter = more confident)
        //       - draw a small vertical line or point per frame
        // TODO: label axes
        // TODO: draw current pitch as a horizontal line or text overlay

        // Placeholder text so the panel isn't completely empty:
        ui.label("🎵 Spectrograph — waiting for audio…");
    }
}
