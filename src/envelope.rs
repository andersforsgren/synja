use std::time::Instant;

#[derive(Debug, PartialEq)]
pub(crate) enum State {
    Idle,
    Attacking,
    Decaying,
    Sustaining,
    Releasing,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Adsr {
    pub attack_rate: f32,
    pub decay_rate: f32,
    pub sustain_level: f32,
    pub release_rate: f32,
}

#[derive(Debug)]
pub struct AdsrEnvelope {
    #[allow(dead_code)]
    voice_id: i32,
    pub(crate) state: State,
    level: f32,
    start_time: Option<Instant>,
    pub params: Adsr,

    attack_coeff: f32,
    decay_coeff: f32,
    release_coeff: f32,
    attack_base: f32,
    decay_base: f32,
    release_base: f32,

    target_ratio_a: f32,
    target_ratio_dr: f32,
}

impl AdsrEnvelope {
    pub fn new(voice_id: i32) -> Self {
        AdsrEnvelope {
            voice_id,
            state: State::Idle,
            start_time: None,
            level: 0.0,
            params: Adsr {
                attack_rate: 0.0,
                decay_rate: 0.0,
                sustain_level: 0.0,
                release_rate: 0.0,
            },
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            release_coeff: 0.0,
            attack_base: 0.0,
            decay_base: 0.0,
            release_base: 0.0,

            target_ratio_a: 0.1,
            target_ratio_dr: 0.001,
        }
    }

    pub fn set_envelope_parameters(
        &mut self,
        sample_rate: f32,
        attack_rate_seconds: f32,
        decay_rate_seconds: f32,
        sustain_level: f32,
        release_rate_seconds: f32,
    ) {
        // debug!(
        //     "Envelope params: A={}s D={}s S={} R{}s",
        //     attack_rate_seconds, decay_rate_seconds, sustain_level, release_rate_seconds
        // );
        self.params = Adsr {
            attack_rate: attack_rate_seconds,
            decay_rate: decay_rate_seconds,
            sustain_level,
            release_rate: release_rate_seconds,
        };
        self.attack_coeff = calc_coeff(self.params.attack_rate * sample_rate, self.target_ratio_a);
        self.attack_base = (1.0 + self.target_ratio_a) * (1.0 - self.attack_coeff);

        self.decay_coeff = calc_coeff(self.params.decay_rate * sample_rate, self.target_ratio_dr);
        self.decay_base =
            (self.params.sustain_level - self.target_ratio_dr) * (1.0 - self.decay_coeff);

        self.release_coeff =
            calc_coeff(self.params.release_rate * sample_rate, self.target_ratio_dr);
        self.release_base = -self.target_ratio_dr * (1.0 - self.release_coeff);
    }

    pub fn gate_on(&mut self) {
        self.start_time = Some(Instant::now());
        self.state = State::Attacking;
    }

    pub fn gate_off(&mut self) {
        match self.state {
            State::Attacking | State::Sustaining | State::Decaying => {
                self.state = State::Releasing;
            }
            _ => (),
        }
    }

    pub fn is_idle(&self) -> bool {
        self.state == State::Idle
    }

    pub fn is_decaying(&self) -> bool {
        self.state == State::Decaying
    }

    pub fn next(&mut self) -> f32 {
        self.process();
        self.level
    }

    pub fn process(&mut self) {
        match self.state {
            State::Attacking => {
                self.level = self.attack_base + self.level * self.attack_coeff;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.state = State::Decaying;
                }
            }
            State::Decaying => {
                self.level = self.decay_base + self.level * self.decay_coeff;
                if self.level <= self.params.sustain_level {
                    self.level = self.params.sustain_level;
                    self.state = State::Sustaining;
                }
            }
            State::Releasing => {
                self.level = self.release_base + self.level * self.release_coeff;
                if self.level <= 0.0 {
                    self.level = 0.0;
                    self.state = State::Idle;
                    self.start_time = None;
                }
            }
            _ => (),
        }
    }
}

fn calc_coeff(rate: f32, target_ratio: f32) -> f32 {
    if rate <= 0.0 {
        return 0.0;
    }
    (-((1.0 + target_ratio) / target_ratio).ln() / rate).exp()
}
