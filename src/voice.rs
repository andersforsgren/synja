use crate::envelope::*;
use crate::filter::Filter;
use crate::huovilainen::HuovilainenMoog;
use crate::midi::*;
use crate::oscillator::*;
use crate::SynthParams;
use crate::MAX_BLOCK_SIZE;
use std::ops::Not;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub const MAX_UNISON: usize = 7;

static UNISON_DETUNE_PATTERN: &'static [&[f32]] = &[
    &[],
    &[0.0],
    &[-1.0, 1.0],
    &[-1.0, 0.0, 1.0],
    &[-1.0, -0.5, 0.5, 1.0],
    &[-1.0, -0.5, 0.0, 0.5, 1.0],
    &[-1.0, -0.6667, -0.3333, 0.3333, 0.6667, 1.0],
    &[-1.0, -0.6667, -0.3333, 0.0, 0.3333, 0.6667, 1.0],
];

static UNISON_SPREAD_PATTERN: &'static [&[f32]] = &[
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
    sample_rate: f32,
    #[allow(dead_code)]
    pub id: i32, // DAW voice identifier
    pub target_note: u8, // Portamento target note
    pub note: f32,       // Current note
    pub bend: f32,       // bend in semitones
    pub velocity: u8,
    pub start_time: f64,
    pub unison: usize,
    pub osc1: Vec<Oscillator>,
    pub osc2: Vec<Oscillator>,
    pub lfo: Oscillator,
    pub filter: (HuovilainenMoog, HuovilainenMoog),
    pub env_change: Arc<AtomicU16>,
    pub amp_envelope: AdsrEnvelope,
    pub filter_envelope: AdsrEnvelope,
}

impl Voice {
    pub fn new(id: i32, sample_rate: f32, env_chg: &Arc<AtomicU16>) -> Self {
        Voice {
            sample_rate,
            id,
            target_note: 0,
            note: 0.0,
            bend: 0.0,
            velocity: 0,
            start_time: 0.0,
            unison: 1,
            osc1: (0..MAX_UNISON).map(|_| Oscillator::new()).collect(),
            osc2: (0..MAX_UNISON).map(|_| Oscillator::new()).collect(),
            lfo: Oscillator::new(),
            filter: (HuovilainenMoog::new(), HuovilainenMoog::new()),
            env_change: env_chg.clone(),
            amp_envelope: AdsrEnvelope::new(id),
            filter_envelope: AdsrEnvelope::new(id),
        }
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: u8,
        time: f64,
        unison: usize,
        lfo_trig: bool,
        start_phases: &[f64; MAX_UNISON],
    ) {
        for i in 0..MAX_UNISON {
            self.osc1[i].set_phase(start_phases[i]);
        }
        self.target_note = note;
        if lfo_trig {
            self.lfo.trig();
        }
        self.unison = unison;
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

    fn get_oscillator_semitone(&mut self, detune: f32, portamento: f32) -> f32 {
        if portamento <= 0.0 {
            self.note = self.target_note as f32;
        } else {
            self.note += (self.target_note as f32 - self.note) * 1.0 / (100.0 * portamento);
        }

        self.note + self.bend as f32 + detune
    }

    fn frequency(&mut self, detune_semitones: f32, octave: i32, portamento: f32) -> f32 {
        // Requires +2 offset                -2    -1    0    1    2
        const OCTIAVE_MULTIPLIER: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];
        let octave_multiplier = OCTIAVE_MULTIPLIER[octave as usize + 2];

        let semitone = self.get_oscillator_semitone(detune_semitones, portamento);

        midi_pitch_to_freq(semitone) * octave_multiplier
    }

    // Note amplitude from midi velocity
    fn note_amplitude(&self) -> f64 {
        midi_velocity_to_amplitude(self.velocity) as f64
    }

    pub fn generate(
        &mut self,
        params: &mut Arc<SynthParams>,
        output: &mut [&mut [f32]],
        block_start: usize,
        block_end: usize,
    ) {
        let osc1_waveform: WaveForm = params.osc1_waveform.value().into();
        let osc2_waveform: WaveForm = params.osc2_waveform.value().into();
        let lfo_waveform: WaveForm = params.lfo_waveform.value().into();

        self.bend = 0.0; // states[STATE_BEND].get(); // TODO: Add pitch bend after switch to nih

        // These modulation depths should probably be smoothed at some point
        let osc1_lfo_pitch_mod_depth_semitones: f32 = params.lfo_osc1_detune_mod_depth.value();
        let filter_lfo_mod_depth: f32 = params.lfo_filter_mod_depth.value();
        let filter_velocity_mod_depth: f32 = params.filter_velocity_mod.value();

        let portamento: f32 = if params.poly_mode.value() {
            0.0
        } else {
            params.portamento.value() * (self.sample_rate / 44100.0)
        };

        // Only update the envelopes if an envelope parameter has changed, and this particular voice has not.
        let bit = 1u16 << (self.id as u16);
        if self.env_change.fetch_and(bit.not(), Ordering::Relaxed) & bit == bit {
            self.amp_envelope.set_envelope_parameters(
                self.sample_rate,
                params.amp_env_attack.value(),
                params.amp_env_decay.value(),
                params.amp_env_sustain.value(),
                params.amp_env_release.value(),
            );
            self.filter_envelope.set_envelope_parameters(
                self.sample_rate,
                params.filter_env_attack.value(),
                params.filter_env_decay.value(),
                params.filter_env_sustain.value(),
                params.filter_env_release.value(),
            );
        }

        const KEYTRACK_PIVOT_NOTE: f64 = 48.0; // C3

        let nvoices = self.unison;
        let unison_scale = 1.0;
        let detune_pattern = UNISON_DETUNE_PATTERN[nvoices];
        let spread_pattern = UNISON_SPREAD_PATTERN[nvoices];

        let block_len = block_end - block_start;

        // Audio-rate smoothed params into scratch arrays. Can't call next() per voice as they are shared between voices.
        let mut params_filter_cutoff = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_filter_resonance = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc1_pulsewidth = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc2_pulsewidth = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc1_detune = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc2_detune = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc1_level = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_osc2_level = [0.0f32; MAX_BLOCK_SIZE];
        let mut params_master_gain = [0.0f32; MAX_BLOCK_SIZE];
        params
            .filter_cutoff
            .smoothed
            .next_block(&mut params_filter_cutoff, block_len);
        params
            .filter_resonance
            .smoothed
            .next_block(&mut params_filter_resonance, block_len);
        params
            .osc1_pulsewidth
            .smoothed
            .next_block(&mut params_osc1_pulsewidth, block_len);
        params
            .osc2_pulsewidth
            .smoothed
            .next_block(&mut params_osc2_pulsewidth, block_len);
        params
            .osc1_level
            .smoothed
            .next_block(&mut params_osc1_level, block_len);
        params
            .osc2_level
            .smoothed
            .next_block(&mut params_osc2_level, block_len);
        params
            .osc1_detune
            .smoothed
            .next_block(&mut params_osc1_detune, block_len);
        params
            .osc2_detune
            .smoothed
            .next_block(&mut params_osc2_detune, block_len);
        params
            .master_gain
            .smoothed
            .next_block(&mut params_master_gain, block_len);

        for i in 0..block_len {
            let base_cutoff = params_filter_cutoff[i];

            // Do the filter key tracking in semitones
            let base_cutoff_semitone = freq_to_midi_pitch(base_cutoff as f32);
            let cutoff_semitone = base_cutoff_semitone
                + (self.get_oscillator_semitone(0.0, portamento) - KEYTRACK_PIVOT_NOTE as f32)
                    * params.filter_key_track.value();

            let lfo = self.lfo.generate(
                lfo_waveform,
                params.lfo_freq.value() as f64,
                1.0,
                0.5,
                self.sample_rate,
            ) as f32;

            let osc1_lfo_detune = osc1_lfo_pitch_mod_depth_semitones * lfo;

            let osc1_modulated_pw = params_osc1_pulsewidth[i];
            let osc2_modulated_pw = params_osc2_pulsewidth[i];
            let amp = self.note_amplitude() as f32;

            let osc1_detune = params_osc1_detune[i] + osc1_lfo_detune;

            // Aggregate unison OSC1
            let mut osc1 = (0.0, 0.0);
            for v in 0..nvoices {
                let f1 = self.frequency(
                    osc1_detune + detune_pattern[v] * params.unison_detune.value() + self.bend,
                    params.osc1_octave.value(),
                    portamento,
                );
                let mono_sample = self.osc1[v].generate(
                    osc1_waveform,
                    f1 as f64,
                    (amp * params_osc1_level[i]) as f64,
                    osc1_modulated_pw,
                    self.sample_rate,
                );

                if nvoices == 1 {
                    osc1 = (osc1.0 + mono_sample, osc1.1 + mono_sample);
                } else {
                    let left_amp = 1.0 - params.unison_stereo_spread.value() * spread_pattern[v];
                    let right_amp = 1.0 + params.unison_stereo_spread.value() * spread_pattern[v];
                    osc1 = (
                        osc1.0 + mono_sample * left_amp as f64,
                        osc1.1 + mono_sample * right_amp as f64,
                    );
                }
            }

            let osc2_detune = params_osc2_detune[i];

            // Aggregate unison OSC2
            let mut osc2 = (0.0f64, 0.0f64);

            for v in 0..nvoices {
                let f2 = self.frequency(
                    osc2_detune + detune_pattern[v] * params.unison_detune.value() + self.bend,
                    params.osc2_octave.value(),
                    portamento,
                );
                let mono_sample = self.osc2[v].generate(
                    osc2_waveform,
                    f2 as f64,
                    (amp * params_osc2_level[i]) as f64,
                    osc2_modulated_pw,
                    self.sample_rate,
                );

                if nvoices == 1 {
                    osc2 = (osc2.0 + mono_sample, osc2.1 + mono_sample);
                } else {
                    let left_amp = 1.0 - params.unison_stereo_spread.value() * spread_pattern[v];
                    let right_amp = 1.0 + params.unison_stereo_spread.value() * spread_pattern[v];
                    osc2 = (
                        osc2.0 + mono_sample * left_amp as f64,
                        osc2.1 + mono_sample * right_amp as f64,
                    );
                }
            }

            osc1 = (osc1.0 * unison_scale, osc1.1 * unison_scale);
            osc2 = (osc2.0 * unison_scale, osc2.1 * unison_scale);

            let amp_env = self.amp_envelope.next();
            let filter_env = self.filter_envelope.next();
            let filter_env_mod_depth = params.filter_env_mod_gain.value();

            let sample = (osc1.0 + osc2.0, osc1.1 + osc2.1);

            // Modulate cutoff in semitones
            let cutoff_mod_semitones = (filter_env * filter_env_mod_depth
                + lfo * filter_lfo_mod_depth
                + amp * filter_velocity_mod_depth)
                * 10.0
                * 12.0; // Full mod = 10 octaves = 120st

            let modulated_cutoff =
                midi_pitch_to_freq(cutoff_semitone + cutoff_mod_semitones).clamp(20.0, 20000.0);

            let master = params_master_gain[i];

            let resonance = params_filter_resonance[i];
            let filtered_sample_l = self.filter.0.process(
                sample.0 as f32,
                self.sample_rate,
                modulated_cutoff,
                resonance,
            );
            let filtered_sample_r = self.filter.1.process(
                sample.1 as f32,
                self.sample_rate,
                modulated_cutoff,
                resonance,
            );
            let amp_sample = (
                filtered_sample_l * amp_env * master,
                filtered_sample_r * amp_env * master,
            );

            output[0][block_start + i] += amp_sample.0;
            output[1][block_start + i] += amp_sample.1;
        }
    }
}
