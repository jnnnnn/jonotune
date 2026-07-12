/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct JonotuneApp {
    /// The most recent detected pitch in Hz (0.0 = no signal).
    pitch_hz: f32,

    /// Ring buffer of recent pitch values for the scrolling history view.
    #[serde(skip)]
    pitch_history: Vec<f32>,
}

impl Default for JonotuneApp {
    fn default() -> Self {
        Self {
            pitch_hz: 0.0,
            pitch_history: vec![0.0; 512],
        }
    }
}

impl JonotuneApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize look and feel:
        // cc.egui_ctx.set_visuals(…);
        // cc.egui_ctx.set_fonts(…);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

impl eframe::App for JonotuneApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("🎤 jonotune");
            ui.label("Real-time pitch spectrograph for singing practice.");

            ui.separator();

            ui.label(format!("Pitch: {:.1} Hz", self.pitch_hz));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
            });
        });
    }
}
