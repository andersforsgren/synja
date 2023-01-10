const A4_PITCH: f64 = 69.0;
const A4_FREQ: f64 = 440.0;

pub fn freq_to_midi_pitch(freq: f64) -> f64 {
    12.0 * (freq / A4_FREQ).log2() + A4_PITCH
}

pub fn midi_pitch_to_freq(pitch: f64) -> f64 {
    ((pitch - A4_PITCH) / 12.0).exp2() * A4_FREQ
}

pub fn midi_velocity_to_amplitude(velocity: u8) -> f32 {
    // https://pdfs.semanticscholar.org/92a7/dc5007d770e0c5a3a637f66ee128ba107a92.pdf
    let b = 0.023937f32;
    let m = (1.0 - b) / 127.0;
    let v = velocity as f32;
    (m * v + b) * (m * v + b)
}
