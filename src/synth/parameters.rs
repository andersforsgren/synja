use crate::synth::*;
use num::FromPrimitive;
use num_traits::Pow;
use std::ops::RangeInclusive;
use strum::EnumString;

#[derive(Debug)]
pub enum ParameterRange {
    Linear(f64, f64),
    Discrete(i32, i32),
    Logarithmic(f64, f64),
}

impl ParameterRange {
    /// Maps to 0..1
    pub fn map_to_daw(&self, x: f64) -> f32 {
        let d = match self {
            Self::Linear(min, max) => ((x - min) / (max - min)) as f32,
            Self::Logarithmic(min, max) => {
                let res = ((x.log2() - min.log2()) / (max.log2() - min.log2())) as f32;
                res
            }
            Self::Discrete(min, max) => ((x - (*min as f64)) / (max - min) as f64) as f32,
        };
        num::clamp(d, 0.0, 1.0)
    }

    pub fn range(&self) -> RangeInclusive<f32> {
        match self {
            Self::Linear(min, max) => *min as f32..=*max as f32,
            Self::Logarithmic(min, max) => *min as f32..=*max as f32,
            Self::Discrete(min, max) => *min as f32..=*max as f32,
        }
    }

    pub fn map_to_plugin(&self, daw_value: f32) -> f64 {
        match self {
            Self::Linear(min, max) => min + daw_value as f64 * (max - min),
            Self::Logarithmic(min, max) => {
                2.0f64.pow(min.log2() + daw_value as f64 * (max.log2() - min.log2()))
            }
            Self::Discrete(min, max) => {
                (*min as f64 + daw_value as f64 * ((max - min) as f64)).round()
            }
        }
    }
}

#[derive(PartialEq, Copy, Clone, Debug, FromPrimitive, EnumString)]
pub enum Param {
    // Filter
    FilterCutoff,
    FilterResonance,
    FilterEnvModGain,
    FilterKeyTrack,
    FilterVelocityMod,
    // Amp Envelope
    AmpEnvAttack,
    AmpEnvDecay,
    AmpEnvSustain,
    AmpEnvRelease,
    // Filter envelope
    FilterEnvAttack,
    FilterEnvDecay,
    FilterEnvSustain,
    FilterEnvRelease,
    // OSC1
    Osc1Level,
    Osc1Octave,
    Osc1Detune,
    Osc1WaveForm,
    Osc1PulseWidth,
    // OSC2
    Osc2Level,
    Osc2Octave,
    Osc2Detune,
    Osc2WaveForm,
    Osc2PulseWidth,
    // LFO
    LfoHostSync,
    LfoKeyTrig,
    LfoFreq,
    LfoWaveform,
    LfoFilterModDepth,
    LfoOsc1DetuneDepth,
    // Master
    MasterGain,

    UnisonVoices,
    UnisonDetune,
    UnisonStereoSpread,

    PolyMode,
    Portamento,
}

pub const PARAMS: [Param; 35] = [
    // Filter
    Param::FilterCutoff,
    Param::FilterResonance,
    Param::FilterEnvModGain,
    Param::FilterKeyTrack,
    Param::FilterVelocityMod,
    // Amp Envelope
    Param::AmpEnvAttack,
    Param::AmpEnvDecay,
    Param::AmpEnvSustain,
    Param::AmpEnvRelease,
    // Filter envelope
    Param::FilterEnvAttack,
    Param::FilterEnvDecay,
    Param::FilterEnvSustain,
    Param::FilterEnvRelease,
    // OSC1
    Param::Osc1Level,
    Param::Osc1Octave,
    Param::Osc1Detune,
    Param::Osc1WaveForm,
    Param::Osc1PulseWidth,
    // OSC2
    Param::Osc2Level,
    Param::Osc2Octave,
    Param::Osc2Detune,
    Param::Osc2WaveForm,
    Param::Osc2PulseWidth,
    // LFO
    Param::LfoHostSync,
    Param::LfoKeyTrig,
    Param::LfoFreq,
    Param::LfoWaveform,
    Param::LfoFilterModDepth,
    Param::LfoOsc1DetuneDepth,
    // Master
    Param::MasterGain,
    // Unison
    Param::UnisonVoices,
    Param::UnisonDetune,
    Param::UnisonStereoSpread,
    // Ctrl
    Param::PolyMode,
    Param::Portamento,
];

impl std::fmt::Display for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct ParamConfig {
    pub range: ParameterRange,
    pub default: f64,
    pub unit: &'static str,
    pub label: &'static str,
    pub daw_name: &'static str,
    pub daw_display: &'static dyn Fn(f32) -> String,
}

fn db_display(x: f32) -> String {
    if x <= 0.0f32 {
        return "-Inf. dB".to_string();
    }
    format!("{:.2} dB", 10.0 * x.log10()).to_string()
}

impl Param {
    pub fn index(&self) -> usize {
        *self as usize
    }

    pub fn iindex(&self) -> i32 {
        (*self as usize) as i32
    }

    pub fn from_index(i: usize) -> Self {
        let result: Self = FromPrimitive::from_usize(i).unwrap();
        result
    }

    pub fn get_config(&self) -> ParamConfig {
        const FREQUENCY: ParameterRange =
            ParameterRange::Logarithmic(MIN_FREQ, crate::synth::MAX_FREQ);
        const ENVELOPE_TIME: ParameterRange = ParameterRange::Logarithmic(0.001, 16.0);
        const FACTOR: ParameterRange = ParameterRange::Linear(0.0, 1.0);
        const SIGNED_FACTOR: ParameterRange = ParameterRange::Linear(-1.0, 1.0);

        match self {
            Param::FilterCutoff => ParamConfig {
                range: FREQUENCY,
                default: 4000.0,
                unit: "Hz",
                label: "Cutoff",
                daw_name: "Filter Cutoff",
                daw_display: &|value| format!("{:.0} Hz", value),
            },
            Param::FilterResonance => ParamConfig {
                range: ParameterRange::Linear(0.0, 0.9),
                default: 0.1,
                unit: "",
                label: "Resonance",
                daw_name: "Filter Resonance",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::FilterEnvModGain => ParamConfig {
                range: SIGNED_FACTOR,
                default: 0.0,
                unit: "",
                label: "Env",
                daw_name: "Filter Env Mod",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::FilterKeyTrack => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "Key",
                daw_name: "Filter key track",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::FilterVelocityMod => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "Vel",
                daw_name: "Filter velocity mod",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::AmpEnvAttack => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "A",
                daw_name: "Amp Env Attack",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::AmpEnvDecay => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "D",
                daw_name: "Amp Env Decay",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::AmpEnvSustain => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "S",
                daw_name: "Amp Env Sustain",
                daw_display: &|value| format!("{:.1}%", value * 100.0),
            },
            Param::AmpEnvRelease => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "R",
                daw_name: "Amp Env Release",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::FilterEnvAttack => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "A",
                daw_name: "Filter Env Attack",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::FilterEnvDecay => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "D",
                daw_name: "Filter Env Decay",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::FilterEnvSustain => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "S",
                daw_name: "Filter Env Sustain",
                daw_display: &|value| format!("{:.1}%", value * 100.0),
            },
            Param::FilterEnvRelease => ParamConfig {
                range: ENVELOPE_TIME,
                default: 0.01,
                unit: "s",
                label: "R",
                daw_name: "Filter Env Release",
                daw_display: &|value| format!("{:.3} s", value),
            },
            Param::Osc1Level => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "Osc1",
                daw_name: "OSC1 Level",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::Osc1Octave => ParamConfig {
                range: ParameterRange::Discrete(-3, 3),
                default: 0.0,
                unit: "",
                label: "Octave",
                daw_name: "OSC1 Octave",
                daw_display: &|value| format!("{:.2} oct", value),
            },
            Param::Osc1Detune => ParamConfig {
                range: ParameterRange::Linear(-2.0, 2.0),
                default: 0.0,
                unit: "st",
                label: "Detune",
                daw_name: "OSC1 Detune",
                daw_display: &|value| format!("{:.2} st", value),
            },
            Param::Osc1WaveForm => ParamConfig {
                range: ParameterRange::Discrete(0, 2),
                default: 0.0,
                unit: "",
                label: "",
                daw_name: "OSC1 Waveform",
                daw_display: &|value| format!("{:.0}", WaveForm::from(value.round() as i32)),
            },
            Param::Osc1PulseWidth => ParamConfig {
                range: ParameterRange::Linear(0.05, 0.95),
                default: 0.5,
                unit: "",
                label: "PW",
                daw_name: "OSC1 PulseWidth",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::Osc2Level => ParamConfig {
                range: FACTOR,
                default: 1.0,
                unit: "",
                label: "Osc2",
                daw_name: "OSC2 Level",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::Osc2Octave => ParamConfig {
                range: ParameterRange::Discrete(-3, 3),
                default: 0.0,
                unit: "",
                label: "Octave",
                daw_name: "OSC2 Octave",
                daw_display: &|value| format!("{:.2} oct", value),
            },
            Param::Osc2Detune => ParamConfig {
                range: ParameterRange::Linear(-2.0, 2.0),
                default: 0.0,
                unit: "st",
                label: "Detune",
                daw_name: "OSC2 Detune",
                daw_display: &|value| format!("{:.2} st", value),
            },
            Param::Osc2WaveForm => ParamConfig {
                range: ParameterRange::Discrete(0, 2),
                default: 0.0,
                unit: "",
                label: "",
                daw_name: "OSC2 Waveform",
                daw_display: &|value| format!("{:.0}", WaveForm::from(value.round() as i32)),
            },
            Param::Osc2PulseWidth => ParamConfig {
                range: ParameterRange::Linear(0.05, 0.95),
                default: 0.5,
                unit: "",
                label: "PW",
                daw_name: "OSC2 PulseWidth",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::LfoHostSync => ParamConfig {
                range: ParameterRange::Discrete(0, 1),
                default: 0.0,
                unit: "",
                label: "Sync",
                daw_name: "LFO Host sync",
                daw_display: &|value| format!("{}", if value > 0.5 { "On" } else { "Off" }),
            },
            Param::LfoKeyTrig => ParamConfig {
                range: ParameterRange::Discrete(0, 1),
                default: 0.0,
                unit: "",
                label: "Trig",
                daw_name: "LFO Key Retrig",
                daw_display: &|value| format!("{}", if value > 0.5 { "On" } else { "Off" }),
            },
            Param::LfoFreq => ParamConfig {
                range: ParameterRange::Logarithmic(0.05, 20.0),
                default: 1.0,
                unit: "Hz",
                label: "Freq",
                daw_name: "LFO freq",
                daw_display: &|value| format!("{:.1} Hz", value),
            },
            Param::LfoWaveform => ParamConfig {
                range: ParameterRange::Discrete(2, 4),
                default: 2.0,
                unit: "",
                label: "Waveform",
                daw_name: "LFO Waveform",
                daw_display: &|value| format!("{:.0}", WaveForm::from(value.round() as i32)),
            },
            Param::LfoFilterModDepth => ParamConfig {
                range: SIGNED_FACTOR,
                default: 0.0,
                unit: "",
                label: "LFO",
                daw_name: "Filter Cutoff LFO Mod",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::LfoOsc1DetuneDepth => ParamConfig {
                range: ParameterRange::Linear(0.0, 12.0),
                default: 0.0,
                unit: "Semitones",
                label: "LFO",
                daw_name: "OSC1 Detune LFO Mod",
                daw_display: &|value| format!("{:.2} st", value),
            },
            Param::UnisonVoices => ParamConfig {
                range: ParameterRange::Discrete(1, 7),
                default: 1.0,
                unit: "",
                label: "Voices",
                daw_name: "Unison voices",
                daw_display: &|value| format!("{:.0}", value),
            },
            Param::UnisonStereoSpread => ParamConfig {
                range: FACTOR,
                default: 0.0,
                unit: "",
                label: "Spread",
                daw_name: "Unison stereo spread",
                daw_display: &|value| format!("{:.2}", value),
            },
            Param::UnisonDetune => ParamConfig {
                range: ParameterRange::Linear(0.0, 0.5),
                default: 0.05,
                unit: "st",
                label: "Detune",
                daw_name: "Unison detune",
                daw_display: &|value| format!("{:.2} st", value),
            },
            Param::MasterGain => ParamConfig {
                range: ParameterRange::Linear(0.0, 2.0),
                default: 1.0,
                unit: "dB",
                label: "Master",
                daw_name: "Master Gain",
                daw_display: &|value| db_display(value),
            },
            Param::PolyMode => ParamConfig {
                range: ParameterRange::Linear(1.0, 2.0),
                default: 2.0,
                unit: "",
                label: "Mode",
                daw_name: "Mode",
                daw_display: &|value| {
                    if value > 1.0 {
                        "Poly".to_string()
                    } else {
                        "Mono ".to_string()
                    }
                },
            },
            Param::Portamento => ParamConfig {
                range: ParameterRange::Linear(0.0, 100.0),
                default: 0.0,
                unit: "",
                label: "Porta",
                daw_name: "Portamento",
                daw_display: &|value| format!("{:.0}", value),
            },
        }
    }
}

impl ParamConfig {
    pub fn map_to_daw(&self, x: f64) -> f32 {
        self.range.map_to_daw(x)
    }

    pub fn map_to_ui(&self, daw_value: f32) -> f64 {
        self.range.map_to_plugin(daw_value)
    }

    pub fn map_to_plugin(&self, daw_value: f32) -> f64 {
        self.range.map_to_plugin(daw_value)
    }
}
