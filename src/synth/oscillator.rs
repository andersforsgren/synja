use rand::Rng;

use crate::synth::blep::{BLEPDATA, BLEPLEN, KTABLE};
use std::f64::consts::PI;
use std::fmt::{Display, Formatter};

pub struct Oscillator {
    buffer: [f32; BLEPLEN / KTABLE],
    i_buffer: usize,
    n_init: usize,
    phase: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WaveForm {
    /// Bi-polar antialiased positive ramp saw
    Saw,
    /// Bi-polar antialiased square wave, variable pulse width
    Square,
    /// Sine waveform
    Sine,

    /// LFO: Unipolar non-antialiased square, fixed 50% pulse width
    UnipolarSquare,
    /// LFO: Bipolar non-antialiased square
    Triangle,
}

impl From<i32> for WaveForm {
    fn from(n: i32) -> Self {
        match n {
            0 => WaveForm::Saw,
            1 => WaveForm::Square,
            2 => WaveForm::Sine,
            3 => WaveForm::UnipolarSquare,
            4 => WaveForm::Triangle,
            _ => panic!("Unknown waveform {}", n),
        }
    }
}

impl Display for WaveForm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WaveForm::Saw => write!(f, "Saw"),
            WaveForm::Square => write!(f, "Square"),
            WaveForm::Sine => write!(f, "Sine"),
            WaveForm::UnipolarSquare => write!(f, "Square"),
            WaveForm::Triangle => write!(f, "Triangle"),
        }
    }
}

impl Oscillator {
    pub fn new() -> Self {
        Oscillator {
            phase: rand::thread_rng().gen(),
            buffer: [0.0f32; BLEPLEN / KTABLE],
            i_buffer: 0,
            n_init: 0,
        }
    }

    // Offset: where in the phase the discontinuity occurs. E.g. 0.25 = a quarter of the way into the phase.
    //         this would mean sourcing (in_index) from the blep table at 0.25*KTABLE = the 16th sample.
    fn add_blep(&mut self, offset: f64, amp: f64) {
        let mut out_index = self.i_buffer;
        let mut in_index = (KTABLE as f64 * offset).floor() as usize;

        // The remainder of the phase e.g. 0.75 phases would mean 48 BLEP samples.
        let frac: f64 = KTABLE as f64 * offset % 1.0;

        let c_blep = (BLEPDATA.len() / KTABLE) - 1;

        // Add
        for _i in 0..self.n_init {
            if out_index >= self.buffer.len() {
                out_index = 0;
            }
            let f: f64 = lerp(BLEPDATA[in_index], BLEPDATA[in_index + 1], frac);
            self.buffer[out_index] += (amp * (1.0 - f)) as f32;
            in_index += KTABLE; // Jump 64 samples at a time through the BLEP data
            out_index += 1;
        }
        // Copy
        for _i in self.n_init..c_blep {
            if out_index >= self.buffer.len() {
                out_index = 0;
            }
            let f: f64 = lerp(BLEPDATA[in_index], BLEPDATA[in_index + 1], frac);
            self.buffer[out_index] = (amp * (1.0 - f)) as f32;
            in_index += KTABLE;
            out_index += 1;
        }
        self.n_init = c_blep;
    }

    pub fn generate(
        &mut self,
        waveform: WaveForm,
        freq: f64,
        amplitude: f64,
        pulse_width: f32,
        sample_rate: f32,
    ) -> f64 {
        if freq <= 0.0 {
            return 0.0;
        }

        let dp = freq / sample_rate as f64;

        self.phase += dp;

        let wave = match waveform {
            WaveForm::Saw => {
                if self.phase > 1.0 {
                    self.phase -= 1.0;
                    self.add_blep(self.phase / dp, 1.0);
                }
                self.phase as f64 // Saw 0..1
            }
            WaveForm::Sine => {
                if self.phase > 1.0 {
                    self.phase -= 1.0;
                }
                (2.0 * PI * self.phase).sin() as f64 // sine -1..1
            }
            WaveForm::Square => {
                if self.phase > pulse_width as f64 && self.phase - dp <= pulse_width as f64 {
                    self.add_blep((self.phase - pulse_width as f64) / dp, 1.0);
                } else if self.phase > 1.0 {
                    self.phase -= 1.0;
                    self.add_blep(self.phase / dp, -1.0);
                }
                if self.phase > 0.0 && self.phase <= pulse_width as f64 {
                    1.0
                } else {
                    0.0
                } // square/pulse 0..1
            }
            WaveForm::UnipolarSquare => {
                if self.phase > 1.0 {
                    self.phase -= 1.0;
                }
                if self.phase > 0.0 && self.phase <= 0.5 as f64 {
                    1.0
                } else {
                    0.0
                } // square/pulse 0..1
            }
            WaveForm::Triangle => {
                if self.phase > 1.0 {
                    self.phase -= 1.0;
                }
                if self.phase > 0.5 {
                    (2.0 - 2.0 * self.phase) as f64
                } else {
                    2.0 * self.phase as f64
                } // Triangle 0..1
            }
        };
        // Scale to bipolar if required, and add BLEP
        match waveform {
            WaveForm::Sine => wave * amplitude,
            WaveForm::Triangle => (2.0 * wave - 1.0) * amplitude,
            WaveForm::UnipolarSquare => wave * amplitude,
            _ => {
                let mut blep = 0.0;
                if self.n_init > 0 {
                    blep = self.buffer[self.i_buffer] as f64;
                    self.n_init -= 1;
                    self.i_buffer += 1;
                    if self.i_buffer >= self.buffer.len() {
                        self.i_buffer = 0;
                    }
                }
                let sample = wave + blep; // blep is for 0..1 signal
                amplitude * ((2.0 * sample) - 1.0) // scale to -amp..amp signal
            }
        }
    }

    pub fn trig(&mut self) {
        self.phase = 0.0;
    }
}

fn lerp(a: f64, b: f64, frac: f64) -> f64 {
    (b - a) * frac + a
}
