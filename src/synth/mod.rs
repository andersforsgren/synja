mod blep;
mod envelope;
mod filter;
mod huovilainen;
mod midi;
mod oscillator;
mod parameters;
mod presets;
mod voice;

pub use self::parameters::*;
pub use self::presets::*;
use self::voice::Voice;
use vst::buffer::AudioBuffer;

pub const MIN_FREQ: f64 = 20.0;
pub const MAX_FREQ: f64 = 20000.0;
const POLYPHONY: usize = 8;

// Settling time for smoothed state/parameter changes.
const FILTER_FACTOR: f32 = 0.1;

// Synth states: all parameters, followed by non-parameter states e.g. bend amount.
pub const NUM_STATES: usize = PARAMS.len() + 1;
pub const STATE_BEND: usize = NUM_STATES - 1;

pub(crate) use oscillator::WaveForm;

pub struct Synth {
    voices: Vec<Voice>,
    pub(crate) states: Vec<Smoothed>,
}

impl Default for Synth {
    fn default() -> Self {
        Synth {
            voices: (0..POLYPHONY).map(|i| Voice::new(i)).collect(),
            states: vec![Smoothed::default(); NUM_STATES],
        }
    }
}

impl Synth {
    pub fn generate_audio(&mut self, sample_rate: f32, buffer: &mut AudioBuffer<f32>) {

        let n = buffer.samples();
        let mut output = buffer.split().1;
        for i in 0..n {
            output[0][i] = 0.0;
            output[1][i] = 0.0;
        }

        for voice in self.voices.iter_mut().filter(|v| v.is_playing()) {
            voice.generate(&mut self.states, sample_rate, buffer);
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8, time: f64) {
        debug!("Note on {} ", note);
        let unison = self.states[Param::UnisonVoices.index()].get_discrete() as usize;
        let lfo_trig = self.states[Param::LfoKeyTrig.index()].get_discrete() == 1;
        let mut oldest_playing_voice: usize = 0;
        let mut oldest_playing_time = f64::MAX;
        let mut oldest_decaying_voice: Option<usize> = None;
        let mut oldest_decaying_time = f64::MAX;

        let mono = self.states[Param::PolyMode.index()].get_discrete() as usize == 1;
        if mono {
            // Mono: always trig voice 0
            self.voices[0].note_on(note, velocity, time, unison, lfo_trig);
        } else {
            for i in 0..POLYPHONY {
                if !self.voices[i].is_playing() {
                    // Found an idle voice. Use that.
                    self.voices[i].note_on(note, velocity, time, unison, lfo_trig);
                    return;
                } else {
                    if self.voices[i].amp_envelope.is_decaying()
                        && self.voices[i].start_time < oldest_decaying_time
                    {
                        oldest_decaying_voice = Some(i);
                        oldest_decaying_time = self.voices[i].start_time;
                    }
                    if self.voices[i].start_time < oldest_playing_time {
                        oldest_playing_voice = i;
                        oldest_playing_time = self.voices[i].start_time;
                    }
                }
            }
        }

        // Steal the oldest decaying voice if one exists. Otherwise the oldest playing voice.
        match oldest_decaying_voice {
            Some(v) => self.voices[v].note_on(note, velocity, time, unison, lfo_trig),
            None => {
                self.voices[oldest_playing_voice].note_on(note, velocity, time, unison, lfo_trig)
            }
        }
    }

    pub fn note_off(&mut self, note: u8) {
        debug!("Note off {} ", note);
        for i in 0..POLYPHONY {
            if self.voices[i].target_note == note {
                self.voices[i].note_off();
            }
        }
    }

    pub fn pitch_bend(&mut self, d1: u8, d2: u8) {
        let v1: u32 = d1 as u32;
        let v2: u32 = d2 as u32;
        let bend = (v2 * 256 + v1) as f32 / 16384.0 - 1.0;
        self.states[STATE_BEND].set(bend);
    }
}

#[derive(Clone, Default)]
pub(crate) struct Smoothed {
    state: f32,
    target: f32,
}

impl Smoothed {
    pub(crate) fn set(&mut self, value: f32) {
        self.target = value;
    }

    fn get(&mut self) -> f32 {
        self.state += (self.target - self.state) * FILTER_FACTOR;
        self.state
    }

    fn get_discrete(&mut self) -> i32 {
        self.target.round() as i32
    }
}
