use super::envelope::*;
use super::filter::*;
use super::midi::*;
use super::oscillator::*;
use super::{Param, Smoothed};
use crate::STATE_BEND;
use vst::buffer::AudioBuffer;

static UNISON_DETUNE_PATTERN: &'static [&[f64]] = &[
    &[],
    &[0.0],
    &[-1.0, 1.0],
    &[-1.0, 0.0, 1.0],
    &[-1.0, -0.5, 0.5, 1.0],
    &[-1.0, -0.5, 0.0, 0.5, 1.0],
    &[-1.0, -0.6667, -0.3333, 0.3333, 0.6667, 1.0],
    &[-1.0, -0.6667, -0.3333, 0.0, 0.3333, 0.6667, 1.0],
];

static UNISON_SPREAD_PATTERN: &'static [&[f64]] = &[
    &[],
    &[0.0],
    &[-1.0, 1.0],
    &[-1.0, 0.0, 1.0],
    &[-1.0, 1.0, -1.0, 1.0],
    &[-1.0, 1.0, 0.0, 1.0, -1.0],
    &[-1.0, 1.0, -1.0, 1.0, -1.0, 1.0],
    &[-1.0, 1.0, -1.0, 0.0, 1.0, -1.0, 1.0],
];

pub(crate) struct Voice {
    #[allow(dead_code)]
    pub id: usize,
    pub target_note: u8, // Porta target note
    pub note: f64,       // Current note
    pub bend: f32,       // bend in semitones
    pub velocity: u8,
    pub start_time: f64,
    pub osc1: Vec<Oscillator>,
    pub osc2: Vec<Oscillator>,
    pub lfo: Oscillator,
    pub filter: (Box<dyn Filter>, Box<dyn Filter>),
    pub amp_envelope: AdsrEnvelope,
    pub filter_envelope: AdsrEnvelope,
}

impl Voice {
    pub fn new(id: usize) -> Self {
        Voice {
            id,
            target_note: 0,
            note: 0.0,
            bend: 0.0,
            velocity: 0,
            start_time: 0.0,
            osc1: vec![],
            osc2: vec![],
            lfo: Oscillator::new(),
            filter: (
                Box::new(super::huovilainen::HuovilainenMoog::new()),
                Box::new(super::huovilainen::HuovilainenMoog::new()),
            ),
            amp_envelope: AdsrEnvelope::new(id),
            filter_envelope: AdsrEnvelope::new(id),
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: u8, time: f64, unison: usize, lfo_trig: bool) {
        if self.osc1.len() != unison {
            self.osc1 = (0..unison).map(|_| Oscillator::new()).collect();
            self.osc2 = (0..unison).map(|_| Oscillator::new()).collect();
        }
        self.target_note = note;
        if lfo_trig {
            self.lfo.trig();
        }
        self.velocity = velocity;
        self.start_time = time;
        self.amp_envelope.gate_on();
        self.filter_envelope.gate_on();
    }

    pub fn note_off(&mut self) {
        self.amp_envelope.gate_off();
        self.filter_envelope.gate_off();
    }

    pub fn is_playing(&self) -> bool {
        !self.amp_envelope.is_idle()
    }

    fn get_oscillator_semitone(&mut self, detune: f64, portamento: f64) -> f64 {
        if portamento <= 0.0 {
            self.note = self.target_note as f64;
        } else {
            self.note += (self.target_note as f64 - self.note) * 1.0 / (100.0 * portamento);
        }

        self.note + self.bend as f64 + detune
    }

    fn frequncy(&mut self, detune_semitones: f64, octave: i32, portamento: f64) -> f64 {
        let semitone = self.get_oscillator_semitone(detune_semitones, portamento);
        let octave_multiplier = 2.0f64.powf(octave as f64);
        midi_pitch_to_freq(semitone) * octave_multiplier
    }

    // Note amplitude from midi velocity
    fn note_amplitude(&self) -> f64 {
        midi_velocity_to_amplitude(self.velocity) as f64
    }

    pub fn generate(
        &mut self,
        states: &mut Vec<Smoothed>,
        sample_rate: f32,
        buffer: &mut AudioBuffer<f32>,
    ) {
        let samples = buffer.samples();
        let mut output = buffer.split().1;

        let osc1_waveform: WaveForm = states[Param::Osc1WaveForm.index()].get_discrete().into();
        let osc2_waveform: WaveForm = states[Param::Osc2WaveForm.index()].get_discrete().into();
        let lfo_waveform: WaveForm = states[Param::LfoWaveform.index()].get_discrete().into();

        self.bend = states[STATE_BEND].get();

        let portamento: f64 = if states[Param::PolyMode.index()].get_discrete() == 2 {
            0.0
        } else {
            states[Param::Portamento.index()].get() as f64 * (sample_rate as f64 / 44100.0)
        };
        let osc1_lfo_pitch_mod_depth_semitones: f64 =
            states[Param::LfoOsc1DetuneDepth.index()].get().into();
        let filter_lfo_mod_depth: f64 = states[Param::LfoFilterModDepth.index()].get().into();
        let filter_velocity_mod_depth: f64 = states[Param::FilterVelocityMod.index()].get().into();

        self.amp_envelope.set_envelope_parameters(
            sample_rate,
            states[Param::AmpEnvAttack.index()].get(),
            states[Param::AmpEnvDecay.index()].get(),
            states[Param::AmpEnvSustain.index()].get(),
            states[Param::AmpEnvRelease.index()].get(),
        );
        self.filter_envelope.set_envelope_parameters(
            sample_rate,
            states[Param::FilterEnvAttack.index()].get(),
            states[Param::FilterEnvDecay.index()].get(),
            states[Param::FilterEnvSustain.index()].get(),
            states[Param::FilterEnvRelease.index()].get(),
        );

        const KEYTRACK_PIVOT_NOTE: f64 = 48.0; // C3

        let base_cutoff = states[Param::FilterCutoff.index()].get() as f64;

        // Do the filter key tracking in semitones
        let base_cutoff_semitone = freq_to_midi_pitch(base_cutoff);
        let cutoff_semitone: f64 = base_cutoff_semitone
            + (self.get_oscillator_semitone(0.0, portamento) - KEYTRACK_PIVOT_NOTE)
                * states[Param::FilterKeyTrack.index()].get() as f64;

        let nvoices = self.osc1.len();
        let unison_scale = 1.0; /* / (nvoices as f32); Add volume with voices or not? */
        let detune_pattern = UNISON_DETUNE_PATTERN[nvoices];
        let spread_pattern = UNISON_SPREAD_PATTERN[nvoices];

        for i in 0..samples {
            let lfo = self.lfo.generate(
                lfo_waveform,
                states[Param::LfoFreq.index()].get() as f64,
                1.0,
                0.5,
                sample_rate,
            ) as f64;

            let osc1_lfo_detune = osc1_lfo_pitch_mod_depth_semitones * lfo as f64;

            let osc1_modulated_pw = states[Param::Osc1PulseWidth.index()].get();
            let osc2_modulated_pw = states[Param::Osc2PulseWidth.index()].get();
            let amp: f64 = self.note_amplitude();

            let osc1_octave = states[Param::Osc1Octave.index()].get_discrete() as i32;
            let osc1_detune = states[Param::Osc1Detune.index()].get() as f64 + osc1_lfo_detune;

            let unison_detune_semitones = states[Param::UnisonDetune.index()].get() as f64;
            let unison_stereo_spread = states[Param::UnisonStereoSpread.index()].get() as f64;

            // Aggregate unison OSC1
            let mut osc1 = (0.0f64, 0.0f64);
            for v in 0..nvoices {
                let f1 = self.frequncy(
                    osc1_detune + detune_pattern[v] * unison_detune_semitones + self.bend as f64,
                    osc1_octave,
                    portamento,
                );
                let mono_sample = self.osc1[v].generate(
                    osc1_waveform,
                    f1 as f64,
                    amp * states[Param::Osc1Level.index()].get() as f64,
                    osc1_modulated_pw,
                    sample_rate,
                );

                if nvoices == 1 {
                    osc1 = (osc1.0 + mono_sample, osc1.1 + mono_sample);
                } else {
                    let left_amp = 1.0 - unison_stereo_spread * spread_pattern[v];
                    let right_amp = 1.0 + unison_stereo_spread * spread_pattern[v];
                    osc1 = (
                        osc1.0 + mono_sample * left_amp,
                        osc1.1 + mono_sample * right_amp,
                    );
                }
            }

            // Octave and detune of OSC2 center freq from params/modulation
            let osc2_octave = states[Param::Osc2Octave.index()].get_discrete() as i32;
            let osc2_detune = states[Param::Osc2Detune.index()].get() as f64;

            // Aggregate unison OSC2
            let mut osc2 = (0.0f64, 0.0f64);
            for v in 0..nvoices {
                let f2 = self.frequncy(
                    osc2_detune + detune_pattern[v] * unison_detune_semitones + self.bend as f64,
                    osc2_octave,
                    portamento,
                );
                let mono_sample = self.osc2[v].generate(
                    osc2_waveform,
                    f2 as f64,
                    amp * states[Param::Osc2Level.index()].get() as f64,
                    osc2_modulated_pw,
                    sample_rate,
                );

                if nvoices == 1 {
                    osc2 = (osc2.0 + mono_sample, osc2.1 + mono_sample);
                } else {
                    let left_amp = 1.0 - unison_stereo_spread * spread_pattern[v];
                    let right_amp = 1.0 + unison_stereo_spread * spread_pattern[v];
                    osc2 = (
                        osc2.0 + mono_sample * left_amp,
                        osc2.1 + mono_sample * right_amp,
                    );
                }
            }

            osc1 = (osc1.0 * unison_scale, osc1.1 * unison_scale);
            osc2 = (osc2.0 * unison_scale, osc2.1 * unison_scale);

            let amp_env = self.amp_envelope.next();
            let filter_env = self.filter_envelope.next() as f64;
            let filter_env_mod_depth = states[Param::FilterEnvModGain.index()].get() as f64;

            let sample = (osc1.0 + osc2.0, osc1.1 + osc2.1);

            // Modulate cutoff in semitones
            let cutoff_mod_semitones = (filter_env * filter_env_mod_depth
                + lfo * filter_lfo_mod_depth
                + amp * filter_velocity_mod_depth)
                * 10.0
                * 12.0; // Full mod = 10 octaves
            let modulated_cutoff = midi_pitch_to_freq(cutoff_semitone + cutoff_mod_semitones)
                .clamp(20.0, crate::MAX_FREQ);

            let master = states[Param::MasterGain.index()].get();

            let resonance = states[Param::FilterResonance.index()].get() as f64;
            let filtered_sample_l =
                self.filter
                    .0
                    .process(sample.0 as f32, sample_rate, modulated_cutoff, resonance);
            let filtered_sample_r =
                self.filter
                    .1
                    .process(sample.1 as f32, sample_rate, modulated_cutoff, resonance);
            let amp_sample = (
                filtered_sample_l * amp_env * master,
                filtered_sample_r * amp_env * master,
            );

            output[0][i] += amp_sample.0;
            output[1][i] += amp_sample.1;
        }
    }
}
