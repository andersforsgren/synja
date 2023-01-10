use nih_plug_egui::egui::util::History;

pub struct FrameHistory {
    frame_times: History<()>,
}

impl Default for FrameHistory {
    fn default() -> Self {
        Self {
            frame_times: History::new(0..120, 1.0),
        }
    }
}

impl FrameHistory {
    pub fn on_new_frame(&mut self, now: f64) {
        self.frame_times.add(now, ());
    }

    pub fn fps(&self) -> f32 {
        1.0 / self.frame_times.mean_time_interval().unwrap_or_default()
    }
}
