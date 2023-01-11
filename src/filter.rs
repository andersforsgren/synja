pub trait Filter {
    fn process(&mut self, in_sample: f32, sample_rate: f32, cutoff: f32, resonance: f32) -> f32;
}
