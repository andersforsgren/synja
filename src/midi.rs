#[allow(unused)]
use std::sync::LazyLock;

const A4_PITCH: f32 = 69.0;
const A4_FREQ: f32 = 440.0;

const PITCH_TABLE_SIZE: usize = 512;
const POW2_TABLE_SIZE: usize = 1001;

static PITCH: LazyLock<[f32; PITCH_TABLE_SIZE]> = LazyLock::new(|| {
    println!("initializing");
    let mut arr = [0.0; PITCH_TABLE_SIZE];
    for i in 0..PITCH_TABLE_SIZE {
        arr[i] = (1.0 / 12.0 * (i as i32 - 256) as f32).exp2();
    }
    arr
});

static POW2: LazyLock<[f32; POW2_TABLE_SIZE]> = LazyLock::new(|| {
    let mut arr = [0.0; POW2_TABLE_SIZE];
    for i in 0..POW2_TABLE_SIZE {
        arr[i] = (i as f32 / 12.0 / 1000.0).exp2();
    }
    arr
});

pub fn midi_pitch_to_freq(pitch: f32) -> f32 {
    let pitch_int = pitch as i32;
    let a: f32 = (pitch - pitch_int as f32) * 1000.0;
    let e = pitch_int + 256;
    let pow2idx = a as usize;
    let pow2frac = a - pow2idx as f32;
    let p = PITCH[(e - 69).clamp(0, (PITCH_TABLE_SIZE - 1) as i32) as usize];
    let pow2 = (1.0 - pow2frac) * POW2[pow2idx] + pow2frac * POW2[pow2idx + 1];
    A4_FREQ * p * pow2
}

fn midi_pitch_to_freq_slow(pitch: f32) -> f32 {
    ((pitch - A4_PITCH) / 12.0).exp2() * A4_FREQ
}

pub fn freq_to_midi_pitch_fast(freq: f32) -> f32 {
    12.0 * fast_math::log2_raw(freq / A4_FREQ) + A4_PITCH
}

pub fn midi_velocity_to_amplitude(velocity: u8) -> f32 {
    // https://pdfs.semanticscholar.org/92a7/dc5007d770e0c5a3a637f66ee128ba107a92.pdf
    let b = 0.023937f32;
    let m = (1.0 - b) / 127.0;
    let v = velocity as f32;
    (m * v + b) * (m * v + b)
}

#[allow(unused)]
mod tests {
    use super::midi_pitch_to_freq;
    use super::midi_pitch_to_freq_slow;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn midi_pitch_to_freq_reference() {
        assert_eq!(midi_pitch_to_freq_slow(69.0), 440.0);
        assert_approx_eq!(midi_pitch_to_freq_slow(71.0), 493.883301256, 0.001);
    }

    #[test]
    fn midi_pitch_to_freq_lookup() {
        // Check some known freqs
        assert_eq!(midi_pitch_to_freq(69.0), 440.0);
        assert_approx_eq!(midi_pitch_to_freq(70.0), 466.16376, 0.001);
        assert_approx_eq!(midi_pitch_to_freq(71.0), 493.883301256, 0.001);
        assert_approx_eq!(midi_pitch_to_freq(0.0), 8.1757, 0.001);

        // Check every cent in the normal midi range for deviation in the lookup.
        for i in 0..128 {
            for c in 0..99 {
                let p = i as f32 + c as f32 * 0.01;
                assert_approx_eq!(
                    midi_pitch_to_freq(p),
                    midi_pitch_to_freq_slow(p),
                    0.001 * midi_pitch_to_freq_slow(p)
                );
            }
        }
    }

    #[test]
    fn midi_pitch_to_freq_interpolated() {
        assert_approx_eq!(midi_pitch_to_freq(70.5), 479.8234, 0.01);
    }
}
