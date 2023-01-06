use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Param, PARAMS};

const CURRENT_FORMAT_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SynthPreset {
    pub name: String,
    pub params: Vec<f32>,
}

#[derive(Debug)]
pub struct SynthPresetBank {
    pub presets: Vec<SynthPreset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedSynthPreset {
    pub name: String,
    pub params: Vec<(String, f32)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedSynthPresetBank {
    #[serde(default)]
    pub version: u32,
    pub presets: Vec<SerializedSynthPreset>,
}

impl SynthPresetBank {
    pub fn from_serialized(data: SerializedSynthPresetBank) -> Self {
        let mut presets: Vec<SynthPreset> = vec![];

        for sp in data.presets {
            let mut paramvec: Vec<f32> = vec![0.0; PARAMS.len()]; // TODO: should be defaults?
            for (param_name, val) in sp.params {
                match Param::from_str(&param_name) {
                    Ok(par) => {
                        paramvec[par.index()] = par.get_config().map_to_daw(val as f64);
                    }
                    Err(_) => info!("Failed to parse param name {}", param_name),
                }
            }
            presets.push(SynthPreset {
                name: sp.name,
                params: paramvec,
            })
        }
        SynthPresetBank { presets }
    }

    pub fn to_serialized(&self) -> SerializedSynthPresetBank {
        let mut presets: Vec<SerializedSynthPreset> = vec![];
        for preset in self.presets.iter() {
            let mut serialized_params: Vec<(String, f32)> = vec![];
            for i in 0..PARAMS.len() {
                let param = Param::from_index(i);
                let param_name = param.to_string();
                serialized_params.push((
                    param_name,
                    param.get_config().map_to_plugin(preset.params[i]) as f32,
                ));
            }
            let sp = SerializedSynthPreset {
                name: preset.name.clone(),
                params: serialized_params,
            };
            presets.push(sp);
        }
        SerializedSynthPresetBank {
            version: CURRENT_FORMAT_VERSION,
            presets,
        }
    }
}
