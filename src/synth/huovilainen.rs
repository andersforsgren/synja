use crate::synth::filter::*;
use num_traits::clamp;
use std::f64::consts::PI;

// Moog filter from
// https://github.com/ddiakopoulos/MoogLadders
// (LGPLv3)

pub struct HuovilainenMoog {
    stage: [f64; 4],
    stage_tanh: [f64; 3],
    delay: [f64; 6],

    tune: f64,
    acr: f64,
    res_quad: f64,
}

const THERMAL: f64 = 0.000025f64;

impl HuovilainenMoog {
    pub fn new() -> Self {
        HuovilainenMoog {
            stage: [0.0; 4],
            stage_tanh: [0.0; 3],
            delay: [0.0; 6],
            tune: 0.0,
            acr: 0.0,
            res_quad: 0.0,
        }
    }

    fn compute_coeffs(&mut self, cutoff: f64, resonance: f64, sample_rate: f32) {
        let total_cutoff = clamp(cutoff, 0.0, sample_rate as f64 / 2.0);

        let fc = total_cutoff / sample_rate as f64;
        let f = fc * 0.5; // oversampled
        let fc2 = fc * fc;
        let fc3 = fc * fc * fc;

        let fcr = 1.8730 * fc3 + 0.4955 * fc2 - 0.6490 * fc + 0.9988;
        self.acr = -3.9364 * fc2 + 1.8409 * fc + 0.9968;

        self.tune = (1.0 - (-((2.0 * PI) * f * fcr)).exp()) / THERMAL;

        self.res_quad = 4.0 * resonance * self.acr
    }
}

impl Filter for HuovilainenMoog {
    fn process(&mut self, in_sample: f32, sample_rate: f32, cutoff: f64, resonance: f64) -> f32 {
        self.compute_coeffs(cutoff, resonance, sample_rate);

        // Oversample
        for _j in 0..2 {
            let input = in_sample as f64 - self.res_quad * self.delay[5];
            self.stage[0] =
                self.delay[0] + self.tune * (tanh(input * THERMAL) - self.stage_tanh[0]);
            self.delay[0] = self.stage[0];
            for k in 1..4 {
                let input = self.stage[k - 1];
                self.stage_tanh[k - 1] = tanh(input * THERMAL);
                self.stage[k] = self.delay[k]
                    + self.tune
                        * (self.stage_tanh[k - 1]
                            - (if k != 3 {
                                self.stage_tanh[k]
                            } else {
                                tanh(self.delay[k] * THERMAL)
                            }));
                self.delay[k] = self.stage[k];
            }
            // 0.5 sample delay for phase compensation
            self.delay[5] = (self.stage[3] + self.delay[4]) * 0.5;
            self.delay[4] = self.stage[3];
        }
        self.delay[5] as f32
    }
}

fn tanh(x: f64) -> f64 {
    x.tanh()
}
