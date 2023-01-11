mod blep;
mod editor;
mod envelope;
mod filter;
mod huovilainen;
mod midi;
mod oscillator;
mod voice;
use editor::{create_editor, frame_history::FrameHistory, SynthUiState};
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, EguiState};
use oscillator::WaveForm;
use rand_pcg::Pcg32;
use std::{
    borrow::BorrowMut,
    sync::{Arc, Mutex, atomic::AtomicU16},
    time::SystemTime,
};
use voice::Voice;

const NUM_VOICES: u32 = 16;
const MAX_BLOCK_SIZE: usize = 64;

#[derive(Default)]
pub enum EditText {
    #[default]
    None,
    Editing(String, u64),
}

struct Synth {
    params: Arc<SynthParams>,
    prng: Pcg32,
    voices: Vec<Voice>,
    time: f64,
    ui_state: Arc<SynthUiState>,
    env_chg: Arc<AtomicU16>,
}

#[derive(Clone, Copy, PartialEq, Enum)]
pub enum WaveFormParameter {
    /// Bi-polar antialiased positive ramp saw
    Saw,
    /// Bi-polar antialiased square wave, variable pulse width
    Square,
    /// Sine waveform
    Sine,
}

#[derive(Clone, Copy, PartialEq, Enum)]
pub enum LfoWaveFormParameter {
    /// Sine waveform
    Sine,
    /// LFO: Unipolar non-antialiased square, fixed 50% pulse width
    Square,
    /// LFO: Bipolar non-antialiased square
    Triangle,
}

impl Into<WaveForm> for WaveFormParameter {
    fn into(self) -> WaveForm {
        match self {
            WaveFormParameter::Saw => WaveForm::Saw,
            WaveFormParameter::Square => WaveForm::Square,
            WaveFormParameter::Sine => WaveForm::Sine,
        }
    }
}

impl Into<WaveForm> for LfoWaveFormParameter {
    fn into(self) -> WaveForm {
        match self {
            LfoWaveFormParameter::Triangle => WaveForm::Triangle,
            LfoWaveFormParameter::Square => WaveForm::UnipolarSquare,
            LfoWaveFormParameter::Sine => WaveForm::Sine,
        }
    }
}

#[derive(Params)]
pub struct SynthParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    // Filter
    #[id = "FilterCutoff"]
    filter_cutoff: FloatParam,
    #[id = "FilterResonance"]
    fiter_resonance: FloatParam,
    #[id = "FilterEnvModGain"]
    filter_env_mod_gain: FloatParam,
    #[id = "FilterKeyTrack"]
    filter_key_track: FloatParam,
    #[id = "FilterVelocityMod"]
    filter_velocity_mod: FloatParam,

    // Amp Envelope
    #[id = "AmpEnvAttack"]
    amp_env_attack: FloatParam,
    #[id = "AmpEnvDecay"]
    amp_env_decay: FloatParam,
    #[id = "AmpEnvSustain"]
    amp_env_sustain: FloatParam,
    #[id = "AmpEnvRelease"]
    amp_env_release: FloatParam,

    // Filter envelope
    #[id = "FilterEnvAttack"]
    filter_env_attack: FloatParam,
    #[id = "FilterEnvDecay"]
    filter_env_decay: FloatParam,
    #[id = "FilterEnvSustain"]
    filter_env_sustain: FloatParam,
    #[id = "FilterEnvRelease"]
    filter_env_release: FloatParam,

    // OSC1
    #[id = "Osc1Level"]
    osc1_level: FloatParam,
    #[id = "Osc1Octave"]
    osc1_octave: IntParam,
    #[id = "Osc1Detune"]
    osc1_detune: FloatParam,
    #[id = "Osc1WaveForm"]
    osc1_waveform: EnumParam<WaveFormParameter>,
    #[id = "Osc1PulseWidth"]
    osc1_pulsewidth: FloatParam,

    // OSC1
    #[id = "Osc2Level"]
    osc2_level: FloatParam,
    #[id = "Osc2Octave"]
    osc2_octave: IntParam,
    #[id = "Osc2Detune"]
    osc2_detune: FloatParam,
    #[id = "Osc2WaveForm"]
    osc2_waveform: EnumParam<WaveFormParameter>,
    #[id = "Osc2PulseWidth"]
    osc2_pulsewidth: FloatParam,

    // LFO
    #[id = "LfoHostSync"]
    lfo_host_sync: BoolParam,
    #[id = "LfoKeyTrig"]
    lfo_key_trig: BoolParam,
    #[id = "LfoFreq"]
    lfo_freq: FloatParam,
    #[id = "LfoWaveform"]
    lfo_waveform: EnumParam<LfoWaveFormParameter>,
    #[id = "LfoFilterModDepth"]
    lfo_filter_mod_depth: FloatParam,
    #[id = "LfoOsc1DetuneModDepth"]
    lfo_osc1_detune_mod_depth: FloatParam,

    #[id = "MasterGain"]
    master_gain: FloatParam,

    #[id = "UnisonVoices"]
    unison_voices: IntParam,
    #[id = "UnisonDetune"]
    unison_detune: FloatParam,
    #[id = "UnisonStereoSpread"]
    unison_stereo_spread: FloatParam,

    #[id = "PolyMode"]
    poly_mode: BoolParam,
    #[id = "Portamento"]
    portamento: FloatParam,
}

impl Default for Synth {
    fn default() -> Self {        
        let e = Arc::new(AtomicU16::new(0b1111_1111_1111_1111));
        Self {
            params: Arc::new(SynthParams::new(e.clone())),
            time: 0.0,
            prng: create_rng(),
            env_chg: e.clone(),
            voices: (0..NUM_VOICES).map(move |i| Voice::new(i as i32, 44100.0, &e.clone())).collect(),
            ui_state: Arc::new(SynthUiState {
                edit_text: Mutex::new(EditText::None),
                frame_history: Mutex::new(FrameHistory::default()),
            }),
        }
    }
}

fn create_rng() -> Pcg32 {
    Pcg32::new(111, 333)
}

impl  SynthParams {
    fn new(env_chg: Arc<AtomicU16>) -> Self {
        Self {
            editor_state: editor::default_editor_state(),

            filter_cutoff: freq_param("Filter Cutoff", 4000.0),
            master_gain: gain_param("Master", -6.0),
            amp_env_attack: env_time_param("Amp Attack", env_chg.clone()),
            amp_env_decay: env_time_param("Amp Decay", env_chg.clone()),
            amp_env_release: env_time_param("Amp Release", env_chg.clone()),
            amp_env_sustain: env_gain_param("Amp Sustain", env_chg.clone()),
            filter_env_attack: env_time_param("Filter Attack", env_chg.clone()),
            filter_env_decay: env_time_param("Filter Decay", env_chg.clone()),
            filter_env_release: env_time_param("Filter Release", env_chg.clone()),
            filter_env_sustain: env_gain_param("Filter Sustain", env_chg.clone()),
            fiter_resonance: percentage_param("Filter Resonance", 0.1),
            filter_env_mod_gain: symmetric_percentage_param("Filter env mod"),
            filter_key_track: percentage_param("Key track", 0.1),
            filter_velocity_mod: percentage_param("Filter Vel", 0.1),
            osc1_level: gain_param("Osc1 Level", 0.0),
            osc1_octave: IntParam::new("Osc1 Octave", 0, IntRange::Linear { min: -2, max: 2 }),
            osc1_detune: fine_detune_param("Osc1 Detune"),
            osc1_waveform: EnumParam::new("Osc1 Waveform", WaveFormParameter::Saw),
            osc1_pulsewidth: percentage_param("Osc1 PW", 0.5),
            osc2_level: gain_param("Osc2 Level", 0.0),
            osc2_octave: IntParam::new("Osc2 Octave", 0, IntRange::Linear { min: -2, max: 2 }),
            osc2_detune: fine_detune_param("Osc2 Detune"),
            osc2_waveform: EnumParam::new("Osc2 Waveform", WaveFormParameter::Saw),
            osc2_pulsewidth: percentage_param("Osc2 PW", 0.5),
            lfo_host_sync: BoolParam::new("Sync", false),
            lfo_key_trig: BoolParam::new("Trig", true),
            lfo_freq: FloatParam::new(
                "LFO Freq",
                2.0,
                FloatRange::Linear {
                    min: 0.01,
                    max: 20.0,
                },
            )
            .with_unit("Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            lfo_waveform: EnumParam::new("LFO Waveform", LfoWaveFormParameter::Sine),
            lfo_filter_mod_depth: symmetric_percentage_param("LFO Filter Mod Depth"),
            lfo_osc1_detune_mod_depth: symmetric_percentage_param("LFO OSC1 Detune Mod Depth"),
            unison_voices: IntParam::new("Unison Voices", 1, IntRange::Linear { min: 1, max: 7 }),
            unison_detune: FloatParam::new(
                "Unison Detune",
                0.0,
                FloatRange::Skewed {
                    min: 0.0,
                    max: 2.0,
                    factor: 1.0,
                },
            ),
            unison_stereo_spread: percentage_param("Unison Stereo Spread", 0.5),
            poly_mode: BoolParam::new("Poly", true),
            portamento: FloatParam::new(
                "Portamento",
                0.2,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 3000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_step_size(0.01)
            .with_unit("ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
        }
    }
}

fn percentage_param(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_step_size(0.01)
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(1))
}

fn symmetric_percentage_param(name: impl Into<String>) -> FloatParam {
    FloatParam::new(name, 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
        .with_step_size(0.01)
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(1))
}

fn env_time_param(name: impl Into<String>, env_chg: Arc<AtomicU16>) -> FloatParam {
    FloatParam::new(
        name,
        0.2,
        FloatRange::Skewed {
            min: 0.001,
            max: 20.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_step_size(0.01)
    .with_unit("s")
    .with_value_to_string(formatters::v2s_f32_rounded(3))
    .with_callback({
        let env_chg = env_chg.clone();
        Arc::new(move |_| env_chg.store(0xFFFF, std::sync::atomic::Ordering::Relaxed))
    })
}

fn env_gain_param(name: impl Into<String>, env_chg: Arc<AtomicU16>) -> FloatParam {
    FloatParam::new(
        name,
        util::db_to_gain(0.0),
        // Because we're representing gain as decibels the range is already logarithmic
        FloatRange::Linear {
            min: util::db_to_gain(-100.0),
            max: util::db_to_gain(0.0),
        },
    )
    .with_unit("dB")
    .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
    .with_callback({
        let env_chg = env_chg.clone();
        Arc::new(move |_| env_chg.store(0xFFFF, std::sync::atomic::Ordering::Relaxed))
    })
}

fn freq_param(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 20.0,
            max: 20000.0,
            factor: 0.5,
        },
    )
    .with_unit("Hz")
    .with_value_to_string(formatters::v2s_f32_rounded(0))
}

fn fine_detune_param(name: impl Into<String>) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear { min: -1.0, max: 1.0 },
    )
    .with_step_size(0.01)
    .with_unit("c")
    .with_value_to_string(Arc::new(move |value| format!("{:.0}", value * 100.0)))
}

fn gain_param(name: impl Into<String>, default_dbs: f32) -> FloatParam {
    FloatParam::new(
        name,
        util::db_to_gain(default_dbs),
        // Because we're representing gain as decibels the range is already logarithmic
        FloatRange::Linear {
            min: util::db_to_gain(-100.0),
            max: util::db_to_gain(0.0),
        },
    )
    .with_unit("dB")
    .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
}

impl Plugin for Synth {
    const NAME: &'static str = "Synja";
    const VENDOR: &'static str = "Anders Forsgren";
    const URL: &'static str = "https://github.com/andersforsgren/synja";
    const EMAIL: &'static str = "anders.forsgren@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const DEFAULT_INPUT_CHANNELS: u32 = 0;
    const DEFAULT_OUTPUT_CHANNELS: u32 = 2;

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        create_editor(self.params.clone(), self.ui_state.clone())
    }
    
    fn initialize(
        &mut self,
        _bus_config: &BusConfig,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {        
        self.voices = (0..NUM_VOICES).map(|i| Voice::new(i as i32, buffer_config.sample_rate, &self.env_chg)).collect();
        true
    }

    fn reset(&mut self) {
        self.prng = create_rng();
        
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as f64;

        // NIH-plug has a block-splitting adapter for `Buffer`. While this works great for effect
        // plugins, for polyphonic synths the block size should be `min(MAX_BLOCK_SIZE,
        // num_remaining_samples, next_event_idx - block_start_idx)`. Because blocks also need to be
        // split on note events, it's easier to work with raw audio here and to do the splitting by
        // hand.
        let num_samples = buffer.samples();
        //let sample_rate = context.transport().sample_rate;
        let output = buffer.as_slice();

        let mut next_event = context.next_event();
        let mut block_start: usize = 0;
        let mut block_end: usize = MAX_BLOCK_SIZE.min(num_samples);

        while block_start < num_samples {
            // First of all, handle all note events that happen at the start of the block, and cut
            // the block short if another event happens before the end of it. To handle polyphonic
            // modulation for new notes properly, we'll keep track of the next internal note index
            // at the block's start. If we receive polyphonic modulation that matches a voice that
            // has an internal note ID that's great than or equal to this one, then we should start
            // the note's smoother at the new value instead of fading in from the global value.
            //let this_sample_internal_voice_id_start = self.next_internal_voice_id;
            'events: loop {
                match next_event {
                    // If the event happens now, then we'll keep processing events
                    Some(event) if (event.timing() as usize) <= block_start => {
                        match event {
                            NoteEvent::NoteOn {
                                timing: _,
                                voice_id: _,
                                channel: _,
                                note,
                                velocity,
                            } => self.note_on(note, (velocity * 127.0) as u8, self.time),
                            NoteEvent::NoteOff {
                                timing: _,
                                voice_id: _,
                                channel: _,
                                note,
                                velocity: _,
                            } => {
                                self.note_off(note);
                            }
                            _ => (),
                        };

                        next_event = context.next_event();
                    }
                    // If the event happens before the end of the block, then the block should be cut
                    // short so the next block starts at the event
                    Some(event) if (event.timing() as usize) < block_end => {
                        block_end = event.timing() as usize;
                        break 'events;
                    }
                    _ => break 'events,
                }
            }

            // Silence!
            output[0][block_start..block_end].fill(0.0);
            output[1][block_start..block_end].fill(0.0);

            for voice in self.voices.iter_mut().filter(|v| v.is_playing()) {
                voice.generate(
                    self.params.borrow_mut(),
                    output,
                    block_start,
                    block_end,
                );
            }

            // And then just keep processing blocks until we've run out of buffer to fill
            block_start = block_end;
            block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);
        }

        ProcessStatus::Normal
    }
}

impl Synth {
    pub fn note_on(&mut self, note: u8, velocity: u8, time: f64) {
        let unison = self.params.unison_voices.value() as usize;
        let lfo_trig = self.params.lfo_key_trig.value();
        let mut oldest_playing_voice: usize = 0;
        let mut oldest_playing_time = f64::MAX;
        let mut oldest_decaying_voice: Option<usize> = None;
        let mut oldest_decaying_time = f64::MAX;

        let mono = false;
        if mono {
            // Mono: always trig voice 0
            self.voices[0].note_on(note, velocity, time, unison, lfo_trig);
        } else {
            for i in 0..NUM_VOICES as usize {
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
        for i in 0..NUM_VOICES as usize {
            if self.voices[i].target_note == note {
                self.voices[i].note_off();
            }
        }
    }
}

impl Vst3Plugin for Synth {
    const VST3_CLASS_ID: [u8; 16] = *b"Synja00000000000";
    const VST3_CATEGORIES: &'static str = "Instrument|Synth";
}

nih_export_vst3!(Synth);
