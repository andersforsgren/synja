#![allow(unused_qualifications)]
#[macro_use]
extern crate log;
#[macro_use]
extern crate num_derive;

mod editor;
mod synth;

use crate::editor::SynthEditor;
use crate::synth::*;
use log::LevelFilter;
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use vst::api::{Supported, TimeInfoFlags};
use vst::buffer::AudioBuffer;
use vst::editor::Editor;
use vst::event::{Event, MidiEvent};
use vst::host::Host;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin, PluginParameters};
use vst::util::ParameterTransfer;

const DEFAULT_PRESET_JSON: &str = include_str!("default_presets.json");

const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const PRODUCT_NAME: &str = "synja";

static INSTANCE_ID: AtomicUsize = AtomicUsize::new(0);

struct SynthPlugin {
    #[allow(dead_code)]
    id: usize,
    sample_rate: f32,
    time: f64,
    events_buffer: Vec<MidiEvent>,
    params: Arc<SynthParameters>,
    synth: Arc<Mutex<Synth>>,
    editor: Option<SynthEditor>,
}

pub struct SynthParameters {
    #[allow(dead_code)]
    host: HostCallback,
    transfer: ParameterTransfer,
    editing_parameter: AtomicI32,
    edit_end_time: AtomicU64,
    preset_index: AtomicI32,
    preset_bank: Arc<Mutex<SynthPresetBank>>,
}

impl Default for SynthPlugin {
    fn default() -> Self {
        let host = HostCallback::default();
        Self::new(host)
    }
}

impl Plugin for SynthPlugin {
    fn get_info(&self) -> Info {
        debug!("Get info");
        Info {
            name: "Synja".to_string(),
            vendor: "Anders Forsgren".to_string(),
            unique_id: 113300461,
            version: 0100,
            inputs: 2,
            outputs: 2,
            parameters: synth::PARAMS.len() as i32,
            category: Category::Synth,
            midi_outputs: 0,
            midi_inputs: 1,
            presets: 1,
            preset_chunks: true,
            ..Default::default()
        }
    }

    fn new(host: HostCallback) -> SynthPlugin {
        let id = INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            let now = chrono::Utc::now();
            let date_string = now.format("%Y-%m-%d-%H-%M-%S%.6f");
            let log_filename = format!("{}-{}.log", PRODUCT_NAME, date_string);
            let log_folder = dirs::data_local_dir().unwrap().join(PRODUCT_NAME);
            let _ = std::fs::create_dir(log_folder.clone());
            let log_file = std::fs::File::create(log_folder.join(log_filename)).unwrap();
            let mut bld = simplelog::ConfigBuilder::default();
            let time_format =
                time::macros::format_description!("[hour]:[minute]:[second].[subsecond]");
            bld.set_time_format_custom(time_format);
            let log_config = bld.build();
            match simplelog::CombinedLogger::init(vec![simplelog::WriteLogger::new(
                LevelFilter::Info,
                log_config,
                log_file,
            )]) {
                Err(_) => debug!("Failed to initialize logging"),
                _ => {}
            }
        }

        let default_presets = SynthPresetBank::from_serialized(
            serde_json::from_str::<SerializedSynthPresetBank>(DEFAULT_PRESET_JSON).unwrap(),
        );

        let params = Arc::new(SynthParameters {
            host,
            transfer: ParameterTransfer::new(PARAMS.len()),
            editing_parameter: AtomicI32::new(-1),
            edit_end_time: AtomicU64::new(0),
            preset_index: AtomicI32::new(0),
            preset_bank: Arc::new(Mutex::new(default_presets)),
        });

        info!("Plugin started id={}", id);
        SynthPlugin {
            id: id,
            sample_rate: 12345.0,
            events_buffer: vec![],
            time: 0.0,
            params: params.clone(),
            synth: Arc::new(Mutex::new(Synth::default())),
            editor: Some(SynthEditor::new(params)),
        }
    }

    fn init(&mut self) {
        info!("Setting parameters to default!");
        self.params.set_parameters_to_default();
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        info!("Sample rate set to {}", rate);
    }

    fn can_do(&self, can_do: CanDo) -> Supported {
        match can_do {
            CanDo::ReceiveMidiEvent => Supported::Yes,
            _ => Supported::Maybe,
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        self.time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as f64;

        self.events_buffer = vec![];
        let mut synth = self.synth.lock().unwrap();

        if let Some(time_info) = self
            .params
            .host
            .get_time_info((TimeInfoFlags::TEMPO_VALID | TimeInfoFlags::PPQ_POS_VALID).bits())
        {
            if self.params.get_parameter(Param::LfoHostSync) > 0.5 && time_info.tempo > 0.0 {
                let sync_resolution = 0.25; // 4/4
                let frequency: f64 = time_info.tempo / 60.0 * sync_resolution;
                self.params.set_parameter(Param::LfoFreq, frequency);
                debug!(
                    "Time info : tempo={:.2}  pos={:.2} lfo_freq_sync={:.?}Hz",
                    time_info.tempo, time_info.ppq_pos, frequency
                );
            }
        }

        for (p, value) in self.params.transfer.iterate(true) {
            let param: Param = Param::from_index(p);
            let v = param.get_config().map_to_ui(value);
            synth.states[p].set(v as f32);
        }
        synth.generate_audio(self.sample_rate, buffer);
    }

    fn process_events(&mut self, events: &vst::api::Events) {
        // https://www.midi.org/specifications-old/item/table-1-summary-of-midi-message

        for e in events.events() {
            if let Event::Midi(midi_event) = e {
                debug!("Midi event {:?}", midi_event.data);

                let hi = midi_event.data[0] & 0xF0;
                match hi {
                    // midi note off
                    128 => {
                        let mut syn = self.synth.lock().unwrap();
                        syn.note_off(midi_event.data[1]);
                    }
                    // midi note on
                    144 => {
                        let mut syn = self.synth.lock().unwrap();
                        syn.note_on(midi_event.data[1], midi_event.data[2], self.time);
                    }
                    // pitch wheel
                    224 => {
                        let mut syn = self.synth.lock().unwrap();
                        syn.pitch_bend(midi_event.data[1], midi_event.data[2]);
                    }
                    _ => {
                        debug!("Unknown MIDI event {:?}", midi_event.data);
                    }
                }
            }
        }
    }

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }

    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        if let Some(editor) = self.editor.take() {
            Some(Box::new(editor) as Box<dyn Editor>)
        } else {
            None
        }
    }
}

impl SynthParameters {
    pub fn get_parameter(&self, parameter: Param) -> f32 {
        self.transfer.get_parameter(parameter.index())
    }

    pub fn set_parameter(&self, parameter: Param, daw_value: f64) {
        self.transfer
            .set_parameter(parameter.index(), daw_value as f32)
    }

    pub fn set_parameter_to_default(&self, parameter: Param) {
        let preset_index = self.preset_index.load(Ordering::Relaxed) as usize;
        let value =
            self.preset_bank.lock().unwrap().presets[preset_index].params[parameter.index()];
        self.transfer.set_parameter(parameter.index(), value);
    }

    pub fn set_parameters_to_default(&self) {
        for param in PARAMS {
            self.set_parameter_to_default(param);
            let index: i32 = param.iindex();
            debug!(
                "Param: {} ({})",
                self.get_parameter_name(index),
                self.get_parameter_text(index)
            );
            debug!("  range: {:?}", param.get_config().range);
            debug!("  default: {}", param.get_config().default);
            debug!(
                "  default(daw): {}",
                param.get_config().map_to_daw(param.get_config().default)
            );
        }
    }

    pub fn write_current_preset(&self) {
        let cur_preset_index = self.preset_index.load(Ordering::Relaxed);
        let preset_bank = &mut *(self.preset_bank.lock().expect("Could not lock preset bank"));
        for p in 0..PARAMS.len() {
            preset_bank.presets[cur_preset_index as usize].params[p] =
                self.get_parameter(Param::from_index(p));
        }
    }
}

impl PluginParameters for SynthParameters {
    fn change_preset(&self, index: i32) {
        debug!("Change preset to {}", index);
        let preset_bank = self.preset_bank.lock().unwrap();
        let new_preset_index = (index as usize) % preset_bank.presets.len();
        let preset: &SynthPreset =
            &preset_bank.presets[(new_preset_index as usize) % preset_bank.presets.len()];
        for p in 0..PARAMS.len() {
            self.set_parameter(Param::from_index(p), preset.params[p] as f64);
        }
        self.preset_index
            .store(new_preset_index as i32, Ordering::Relaxed);
    }

    /// Get the current preset index.
    fn get_preset_num(&self) -> i32 {
        self.preset_index.load(Ordering::Relaxed)
    }

    /// Set the current preset name.
    fn set_preset_name(&self, name: String) {
        let idx = self.preset_index.load(Ordering::Relaxed) as usize;
        let mut preset_bank = self.preset_bank.lock().unwrap();
        let preset = &(*preset_bank).presets[idx];
        (*preset_bank).presets[idx] = SynthPreset {
            name,
            params: preset.params.clone(),
        };
    }

    /// Get the name of the preset at the index specified by `preset`.
    fn get_preset_name(&self, preset_index: i32) -> String {
        let preset_bank = self.preset_bank.lock().unwrap();
        (*preset_bank).presets[preset_index as usize]
            .name
            .to_string()
    }

    fn get_parameter_text(&self, index: i32) -> String {
        let value = self.transfer.get_parameter(index as usize);
        let config = PARAMS[index as usize].get_config();
        let display_value = config.map_to_ui(value) as f32;
        (config.daw_display)(display_value)
    }

    fn get_parameter_name(&self, index: i32) -> String {
        PARAMS[index as usize].get_config().daw_name.to_string()
    }

    fn get_parameter(&self, index: i32) -> f32 {
        self.transfer.get_parameter(index as usize)
    }

    fn can_be_automated(&self, index: i32) -> bool {
        match PARAMS[index as usize].get_config().range {
            ParameterRange::Discrete(_, _) => false,
            _ => true,
        }
    }

    fn set_parameter(&self, index: i32, val: f32) {
        self.transfer.set_parameter(index as usize, val);
    }

    fn get_preset_data(&self) -> Vec<u8> {
        debug!(
            "Getting raw data for current preset {}",
            self.preset_index.load(Ordering::Relaxed)
        );
        vec![]
    }

    fn get_bank_data(&self) -> Vec<u8> {
        self.write_current_preset(); // NOTE: implicit write
        debug!("Getting raw data for all presets");
        let preset_bank = &*(self.preset_bank.lock().expect("Could not lock preset bank"));
        let serializable_bank = preset_bank.to_serialized();
        let serialized = serde_json::to_string_pretty(&serializable_bank).unwrap();
        debug!("Serialized:\n{}", serialized);
        serialized.as_bytes().into()
    }

    fn load_preset_data(&self, data: &[u8]) {
        debug!(
            "Deserializing preset data for current preset {} from array of len {}",
            self.preset_index.load(Ordering::Relaxed),
            data.len()
        );
    }

    fn load_bank_data(&self, data: &[u8]) {
        if let Ok(bank_data_result) = serde_json::from_slice::<SerializedSynthPresetBank>(data) {
            debug!("Data format version: {}", bank_data_result.version);
            let mut x = self
                .preset_bank
                .lock()
                .expect("Could not lock synth preset bank");
            if bank_data_result.presets.len() > 0 {
                let bank_data = SynthPresetBank::from_serialized(bank_data_result);
                *x = bank_data;
                self.preset_index.store(0, Ordering::Relaxed);
                debug!(
                    "Loaded {} presets from {} bytes",
                    x.presets.len(),
                    data.len()
                );
                debug!(
                    "Preset 0 name: {} waveform: {}",
                    x.presets[0].name,
                    x.presets[0].params[Param::Osc1WaveForm.index()]
                );
                debug!("Deserialized data:\n{}", std::str::from_utf8(data).unwrap());
            } else {
                debug!("Ignored empty bank");
            }
        } else {
            warn!(
                "Could not deserialize bank data '{}'",
                std::str::from_utf8(data).unwrap_or("Bad encoding in json")
            );
        }
    }
}

vst::plugin_main!(SynthPlugin);
