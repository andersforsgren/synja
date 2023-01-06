pub mod audio_slider;
use crate::synth::*;
use crate::*;
use baseview::{WindowHandle, WindowOpenOptions, WindowScalePolicy};
use egui::widgets::Label;
use egui::{Color32, Context, RichText, Ui};
use egui_baseview::{EguiWindow, Queue};
use egui_extras::{Size, StripBuilder};
use egui_extras_xt::common::WidgetShape;
use egui_extras_xt::displays::{DisplayStylePreset, IndicatorButton, SegmentedDisplayWidget};
use egui_extras_xt::knobs::AudioKnob;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use vst::editor::Editor;

pub struct SynthEditor {
    pub state: Arc<SynthParameters>,
    pub window_handle: Option<WindowHandle>,
    pub is_open: bool,
}

pub struct VstParent(pub *mut ::std::ffi::c_void);

const EDIT_DISPLAY_HOLD_TIME_MS: u64 = 1500;
const WINDOW_WIDTH: usize = 562;
const WINDOW_HEIGHT: usize = 480;

impl SynthEditor {
    pub(crate) fn new(params: Arc<SynthParameters>) -> SynthEditor {
        SynthEditor {
            window_handle: None,
            is_open: false,
            state: params,
        }
    }

    pub(crate) fn build(&self) -> impl FnMut(&Context, &mut Queue, &mut Arc<SynthParameters>) {
        |ctx: &Context, _queue: &mut Queue, _state: &mut Arc<SynthParameters>| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "mathfont".to_owned(),
                egui::FontData::from_static(include_bytes!("../fonts/STIXTwoMath-Regular.ttf")),
            );
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "mathfont".to_owned());

            ctx.set_fonts(fonts);
        }
    }

    pub fn update(&mut self) -> impl FnMut(&Context, &mut Queue, &mut Arc<SynthParameters>) {
        |egui_ctx: &Context, _queue: &mut Queue, state: &mut Arc<SynthParameters>| {
            egui_ctx.request_repaint();
            reset_edit(state);
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                let ed = state.editing_parameter.load(Ordering::Acquire);
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                ui.style_mut().spacing.slider_width = 64.0;
                StripBuilder::new(ui)
                    .size(Size::exact(54.0)) // top bar
                    .size(Size::remainder()) // control section
                    .vertical(|mut strip| {
                        strip.strip(|builder| {
                            builder.size(Size::remainder()).size(Size::exact(48.0)).horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.vertical(|ui| {
                                        let (preset_index, preset_name) = {
                                            let preset_index = state.preset_index.load(Ordering::Relaxed);
                                            let bank = state.preset_bank.lock().unwrap();
                                            let s = (*bank).presets[preset_index as usize].name.to_string(); // Avoid copying this?
                                            (preset_index, s)
                                        };

                                        // 2 lcds
                                        let action_txt = if ed >= 0 {
                                            let edit_index = ed as usize;
                                            let config = Param::from_index(edit_index).get_config();
                                            let display_value =
                                                config.map_to_ui(state.get_parameter(Param::from_index(edit_index))) as f32;
                                            let val_string: String = (config.daw_display)(display_value);
                                            format!("{}={}", config.daw_name.to_uppercase(), val_string)
                                        } else {
                                            "".to_string()
                                        };
                                        let preset_text = format!("{:>2}: {}", preset_index, preset_name);
                                        // Top lcd
                                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                        ui.horizontal(|ui| {
                                            ui.add(
                                                SegmentedDisplayWidget::sixteen_segment(lcd_format(&preset_text, 25))
                                                    .style_preset(DisplayStylePreset::DeLoreanAmber)
                                                    .show_dots(true)
                                                    .show_colons(true)
                                                    .show_apostrophes(false)
                                                    .digit_height(20.0),
                                            );
                                            if ui.button("<").clicked() {
                                                state.change_preset(preset_index - 1);
                                            }
                                            if ui.button(">").clicked() {
                                                state.change_preset(preset_index + 1);
                                            }
                                            if ui.button("Write").clicked() {
                                                state.write_current_preset();
                                            }
                                        });
                                        // Bottom lcd
                                        ui.add(
                                            SegmentedDisplayWidget::sixteen_segment(lcd_format(&action_txt, 30))
                                                .style_preset(DisplayStylePreset::DeLoreanAmber)
                                                .show_dots(true)
                                                .show_colons(true)
                                                .show_apostrophes(false)
                                                .digit_height(20.0),
                                        );
                                    }); // 2 lcds
                                });
                                strip.cell(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                                    create_param_knob(ui, state, Param::MasterGain, true);
                                });
                            });
                        });
                        strip.strip(|builder| {
                            builder
                                .size(Size::exact(70.0)) // LFO  column
                                .size(Size::exact(70.0)) // OSC1 column
                                .size(Size::exact(70.0)) // OSC2 column
                                .size(Size::exact(144.0)) // MIX/UNISON column
                                .size(Size::exact(144.0)) // FILTER column
                                .horizontal(|mut strip| {
                                    // LFO column
                                    strip.cell(|ui| {
                                        Self::control_block("LFO", ui, |ui| {
                                            ui.horizontal(|ui| {
                                                waveform_button(ui, state, Param::LfoWaveform, WaveForm::Sine);
                                                waveform_button(ui, state, Param::LfoWaveform, WaveForm::Triangle);
                                                waveform_button(ui, state, Param::LfoWaveform, WaveForm::UnipolarSquare);
                                            });
                                            ui.vertical_centered(|ui| {
                                                let host_sync = state.get_parameter(Param::LfoHostSync) > 0.5;
                                                ui.add_space(8.0);
                                                ui.add(
                                                    IndicatorButton::from_get_set(|new_val: Option<bool>| {
                                                        if let Some(v) = new_val {
                                                            state.set_parameter(Param::LfoHostSync, if v { 1.0 } else { 0.0 });
                                                            start_edit(state, Param::LfoHostSync);
                                                            end_edit(state);
                                                            false
                                                        } else {
                                                            host_sync
                                                        }
                                                    })
                                                    .label("Sync")
                                                    .style(DisplayStylePreset::DeLoreanAmber.style())
                                                    .height(32.0)
                                                    .width(48.0),
                                                );
                                                ui.add(
                                                    IndicatorButton::from_get_set(|new_val: Option<bool>| {
                                                        if let Some(v) = new_val {
                                                            state.set_parameter(Param::LfoKeyTrig, if v { 1.0 } else { 0.0 });
                                                            start_edit(state, Param::LfoKeyTrig);
                                                            end_edit(state);
                                                            false
                                                        } else {
                                                            state.get_parameter(Param::LfoKeyTrig) > 0.5
                                                        }
                                                    })
                                                    .label("Retrig")
                                                    .style(DisplayStylePreset::DeLoreanAmber.style())
                                                    .interactive(!host_sync)
                                                    .height(32.0)
                                                    .width(48.0),
                                                );
                                                create_param_knob(
                                                    ui,
                                                    state,
                                                    Param::LfoFreq,
                                                    state.get_parameter(Param::LfoHostSync) < 0.5,
                                                );
                                                ui.add(
                                                    IndicatorButton::from_get_set(|new_val: Option<bool>| {
                                                        if let Some(v) = new_val {
                                                            state.set_parameter(Param::PolyMode, if v { 1.0 } else { 0.0 });
                                                            start_edit(state, Param::PolyMode);
                                                            end_edit(state);
                                                            false
                                                        } else {
                                                            state.get_parameter(Param::PolyMode) > 0.5
                                                        }
                                                    })
                                                    .label("Poly")
                                                    .style(DisplayStylePreset::DeLoreanAmber.style())
                                                    .height(32.0)
                                                    .width(48.0),
                                                );
                                                create_param_knob(
                                                    ui,
                                                    state,
                                                    Param::Portamento,
                                                    state.get_parameter(Param::PolyMode) < 0.5,
                                                );
                                            });
                                        });
                                    }); // End LFO column

                                    // OSC1 column
                                    strip.cell(|ui| {
                                        Self::control_block("OSC1", ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.horizontal(|ui| {
                                                    waveform_button(ui, state, Param::Osc1WaveForm, WaveForm::Saw);
                                                    waveform_button(ui, state, Param::Osc1WaveForm, WaveForm::Square);
                                                    waveform_button(ui, state, Param::Osc1WaveForm, WaveForm::Sine);
                                                });
                                                param_knob(ui, state, Param::Osc1Octave);
                                                param_knob(ui, state, Param::Osc1Detune);
                                                param_knob(ui, state, Param::Osc1PulseWidth);
                                                param_knob(ui, state, Param::LfoOsc1DetuneDepth);
                                            });
                                        });
                                    }); // End OSC1 column

                                    // OSC2 column
                                    strip.cell(|ui| {
                                        Self::control_block("OSC2", ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.horizontal(|ui| {
                                                    waveform_button(ui, state, Param::Osc2WaveForm, WaveForm::Saw);
                                                    waveform_button(ui, state, Param::Osc2WaveForm, WaveForm::Square);
                                                    waveform_button(ui, state, Param::Osc2WaveForm, WaveForm::Sine);
                                                });
                                                param_knob(ui, state, Param::Osc2Octave);
                                                param_knob(ui, state, Param::Osc2Detune);
                                                param_knob(ui, state, Param::Osc2PulseWidth);
                                            });
                                        });
                                    }); // End OSC2 column

                                    // MIX/UNISON column
                                    strip.strip(|builder| {
                                        builder.size(Size::exact(96.0)).size(Size::remainder()).size(Size::exact(144.0)).vertical(
                                            |mut strip| {
                                                // Row 1/3: Mix
                                                strip.cell(|ui| {
                                                    Self::control_block("MIX", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        StripBuilder::new(ui)
                                                            .size(Size::relative(0.5))
                                                            .size(Size::relative(0.5))
                                                            .horizontal(|mut strip| {
                                                                // OscLevel - Osc2Level
                                                                strip.cell(|ui| {
                                                                    param_knob(ui, state, Param::Osc1Level);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_knob(ui, state, Param::Osc2Level);
                                                                });
                                                            }); // End levels side by side
                                                    });
                                                });

                                                // Row 2/3: Unison
                                                strip.cell(|ui| {
                                                    Self::control_block("UNISON", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        ui.vertical_centered(|ui| {
                                                            StripBuilder::new(ui)
                                                                .size(Size::exact(24.0))
                                                                .size(Size::remainder())
                                                                .vertical(|mut strip| {
                                                                    strip.cell(|ui| {
                                                                        let config = Param::UnisonVoices.get_config();
                                                                        let nvoices = config
                                                                            .map_to_ui(state.get_parameter(Param::UnisonVoices))
                                                                            as i32;
                                                                        if let ParameterRange::Discrete(from, to) = config.range {
                                                                            StripBuilder::new(ui)
                                                                                .sizes(
                                                                                    Size::relative(1.0 / (to - from + 1) as f32),
                                                                                    (to - from + 1) as usize,
                                                                                )
                                                                                .horizontal(|mut strip| {
                                                                                    for n in from..=to {
                                                                                        strip.cell(|ui| {
                                                                                            if ui
                                                                                                .selectable_label(
                                                                                                    nvoices == n,
                                                                                                    format!("{}", n),
                                                                                                )
                                                                                                .clicked()
                                                                                            {
                                                                                                state.set_parameter(
                                                                                                    Param::UnisonVoices,
                                                                                                    config.map_to_daw(n as f64) as f64,
                                                                                                );
                                                                                                start_edit(state, Param::UnisonVoices);
                                                                                                end_edit(state);
                                                                                            }
                                                                                        });
                                                                                    }
                                                                                });
                                                                        }
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        ui.add_space(4.0);
                                                                        StripBuilder::new(ui)
                                                                            .size(Size::relative(0.5))
                                                                            .size(Size::relative(0.5))
                                                                            .horizontal(|mut strip| {
                                                                                // OscLevel - Osc2Level
                                                                                strip.cell(|ui| {
                                                                                    param_knob(ui, state, Param::UnisonStereoSpread);
                                                                                });
                                                                                strip.cell(|ui| {
                                                                                    param_knob(ui, state, Param::UnisonDetune);
                                                                                });
                                                                            });
                                                                    });
                                                                });
                                                        });
                                                    });
                                                });

                                                // Row 3/3 Amp env
                                                strip.cell(|ui| {
                                                    Self::control_block("AMP ENV", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        ui.vertical_centered(|ui| {
                                                            StripBuilder::new(ui)
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .horizontal(|mut strip| {
                                                                    strip.cell(|ui| {
                                                                        param_slider(ui, state, Param::AmpEnvAttack);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider(ui, state, Param::AmpEnvDecay);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider(ui, state, Param::AmpEnvSustain);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider(ui, state, Param::AmpEnvRelease);
                                                                    });
                                                                });
                                                        });
                                                    });
                                                });
                                            },
                                        ); // End unison row
                                    }); // End MIX/UNISON column

                                    // FILTER column
                                    strip.strip(|builder| {
                                        builder.size(Size::remainder()).size(Size::exact(144.0)).vertical(|mut strip| {
                                            strip.cell(|ui| {
                                                Self::control_block("FILTER", ui, |ui| {
                                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                                                    StripBuilder::new(ui)
                                                        .size(Size::exact(64.0))
                                                        .size(Size::exact(64.0))
                                                        .size(Size::exact(64.0))
                                                        .vertical(|mut strip| {
                                                            strip.cell(|ui| {
                                                                StripBuilder::new(ui)
                                                                    .size(Size::relative(0.5))
                                                                    .size(Size::relative(0.5))
                                                                    .horizontal(|mut strip| {
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::FilterCutoff);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::FilterResonance);
                                                                        });
                                                                    }); // End cutoff/resonance side by side
                                                            });
                                                            strip.cell(|ui| {
                                                                StripBuilder::new(ui)
                                                                    .size(Size::relative(0.5))
                                                                    .size(Size::relative(0.5))
                                                                    .horizontal(|mut strip| {
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::FilterEnvModGain);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::FilterKeyTrack);
                                                                        });
                                                                    }); // End cutoff/resonance side by side
                                                            });
                                                            strip.cell(|ui| {
                                                                StripBuilder::new(ui)
                                                                    .size(Size::relative(0.5))
                                                                    .size(Size::relative(0.5))
                                                                    .horizontal(|mut strip| {
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::LfoFilterModDepth);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob(ui, state, Param::FilterVelocityMod);
                                                                        });
                                                                    }); // End cutoff/resonance side by side
                                                            });
                                                        });
                                                });
                                            });
                                            strip.cell(|ui| {
                                                Self::control_block("FILTER ENV", ui, |ui| {
                                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                    ui.vertical_centered(|ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {
                                                                    param_slider(ui, state, Param::FilterEnvAttack);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider(ui, state, Param::FilterEnvDecay);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider(ui, state, Param::FilterEnvSustain);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider(ui, state, Param::FilterEnvRelease);
                                                                });
                                                            });
                                                    });
                                                });
                                            });
                                        });
                                    }); // End FILTER column
                                }); // End main columns
                        });
                    }); // End vertical display/main
            });
        }
    }

    fn control_block(header: &str, ui: &mut Ui, controls: impl FnOnce(&mut Ui)) {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            5.0,
            Color32::from_black_alpha(64),
        );
        StripBuilder::new(ui)
            .size(Size::exact(24.0))
            .size(Size::remainder())
            .vertical(|mut strip| {
                // Header
                strip.cell(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::from(header).size(18.0).color(Color32::WHITE));
                    });
                });
                // Controls
                strip.cell(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.add_space(8.0);
                    controls(ui);
                });
            });
    }
}

fn lcd_format(msg: &str, width: usize) -> String {
    let mut result = String::new();
    let mut len = 0;
    for c in msg.chars() {
        result.push(c);
        if c != '.' && c != ':' && c != '\'' {
            len += 1
        }
        if len == width {
            return result;
        }
    }
    while len < width {
        result.push(' ');
        len += 1;
    }
    result
}

fn waveform_button(
    ui: &mut Ui,
    state: &mut Arc<SynthParameters>,
    param: Param,
    btn_waveform: WaveForm,
) {
    let config = param.get_config();
    let wf = state.get_parameter(param);
    let active_waveform: WaveForm = (config.map_to_ui(wf).round() as i32).into();
    let symbol = match btn_waveform {
        WaveForm::Saw => '\u{2a58}',
        WaveForm::Square | WaveForm::UnipolarSquare => '\u{2293}',
        WaveForm::Sine => '\u{223f}',
        WaveForm::Triangle => '\u{2227}',
    };
    let label = egui::SelectableLabel::new(
        active_waveform == btn_waveform,
        egui::RichText::new(format!("{}", symbol)).monospace(),
    );

    let response = ui.add(label);
    if response.clicked() {
        let daw_val = config.map_to_daw((btn_waveform as usize) as f64);
        state.set_parameter(param, daw_val as f64);
        start_edit(state, param);
        end_edit(state);
    }
}

fn param_knob(ui: &mut Ui, state: &mut Arc<SynthParameters>, param: Param) {
    create_param_knob(ui, state, param, true);
}

fn create_param_knob(
    ui: &mut Ui,
    state: &mut Arc<SynthParameters>,
    param: Param,
    interactive: bool,
) {
    ui.vertical_centered(|ui| {
        let knob_range = match param.get_config().range {
            ParameterRange::Linear(min, max) => (min as f32)..=(max as f32),
            ParameterRange::Discrete(min, max) => (min as f32)..=(max as f32),
            ParameterRange::Logarithmic(_, _) => 0.0..=1.0,
        };

        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let config = param.get_config();

        let mut knob = AudioKnob::from_get_set(|new_val: Option<f32>| {
            if let Some(v) = new_val {
                match param.get_config().range {
                    ParameterRange::Logarithmic(_, _) => {
                        // Log: knob edits 0..1 and uses no conversion
                        state.set_parameter(param, v as f64);
                        v
                    }
                    _ => {
                        // Other: knob edits true values (Hz, Seconds, ...)
                        let daw_value = param.get_config().map_to_daw(v as f64) as f64;
                        state.set_parameter(param, daw_value);
                        v
                    }
                }
            } else {
                if interactive {
                    let daw_value = state.get_parameter(param);
                    match param.get_config().range {
                        ParameterRange::Logarithmic(_, _) => {
                            // Log: knob edits 0..1 and uses no conversion
                            daw_value
                        }
                        _ => {
                            // Other: knob edits true values (Hz, Seconds, ...)
                            param.get_config().map_to_plugin(daw_value) as f32
                        }
                    }
                } else {
                    0.0
                }
            }
        })
        .diameter(32.0)
        //.drag_length(3.0)
        .range(knob_range)
        .shape(WidgetShape::Circle)
        .interactive(interactive)
        .thickness(0.3)
        .spread(0.8)
        .animated(true);

        if let ParameterRange::Discrete(min, max) = config.range {
            knob = knob.snap(Some(1.0 / ((max - min) as f32)));
        } else if let ParameterRange::Linear(_, _) = config.range {
            knob = knob.shift_snap(Some(0.5));
        }
        let response = ui.add(knob);
        ui.add_space(8.0);
        ui.add(Label::new(config.label));

        if response.double_clicked() {
            state.set_parameter_to_default(param);
        }
        if response.drag_started() {
            start_edit(state, param);
        } else if response.drag_released() {
            end_edit(state);
        }
    });
}

fn start_edit(state: &mut Arc<SynthParameters>, param: Param) {
    state
        .editing_parameter
        .store(param.index() as i32, Ordering::Relaxed);
    state.edit_end_time.store(0, Ordering::Relaxed);
}

fn end_edit(state: &mut Arc<SynthParameters>) {
    state.edit_end_time.store(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        Ordering::Relaxed,
    );
}

fn reset_edit(state: &mut Arc<SynthParameters>) {
    let edit_ended = state.edit_end_time.load(Ordering::Relaxed);
    if edit_ended > 0 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        if now - edit_ended > EDIT_DISPLAY_HOLD_TIME_MS {
            state.editing_parameter.store(-1, Ordering::Relaxed);
            state.edit_end_time.store(0, Ordering::Relaxed);
        }
    }
}

fn param_slider(ui: &mut Ui, state: &mut Arc<SynthParameters>, param: Param) {
    ui.vertical(|ui| {
        let slider = crate::editor::audio_slider::AudioSlider::from_get_set(
            0.0..=1.0,
            |new_val: Option<f64>| {
                if let Some(v) = new_val {
                    state.set_parameter(param, v as f64);
                    v
                } else {
                    state.get_parameter(param) as f64
                }
            },
        )
        .text(param.get_config().label);
        ui.add_space(10.0);
        let response = ui.add(slider);
        if response.double_clicked() {
            state.set_parameter_to_default(param);
        }
        if response.drag_started() {
            start_edit(state, param);
        } else if response.drag_released() {
            end_edit(state);
        }
    });
}

#[allow(dead_code)]
fn wrapper(ui: &Ui, color: Color32) {
    ui.painter()
        .rect_filled(ui.available_rect_before_wrap(), 0.0, color);
}

impl Editor for SynthEditor {
    fn size(&self) -> (i32, i32) {
        (WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32)
    }

    fn position(&self) -> (i32, i32) {
        (0, 0)
    }

    fn close(&mut self) {
        self.is_open = false;
        if let Some(mut window_handle) = self.window_handle.take() {
            window_handle.close();
        }
    }

    fn open(&mut self, parent: *mut ::std::ffi::c_void) -> bool {
        if self.is_open {
            return false;
        }

        let settings = WindowOpenOptions {
            title: format!("{} v{}", PRODUCT_NAME, PRODUCT_VERSION),
            size: baseview::Size::new(WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64),
            scale: WindowScalePolicy::SystemScaleFactor,
            gl_config: Some(Default::default()),
        };

        self.is_open = true;

        let window_handle = EguiWindow::open_parented(
            &VstParent(parent),
            settings,
            self.state.clone(),
            self.build(),
            self.update(),
        );

        self.window_handle = Some(window_handle);

        true
    }

    fn is_open(&mut self) -> bool {
        self.is_open
    }
}

unsafe impl HasRawWindowHandle for VstParent {
    #[cfg(target_os = "macos")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = raw_window_handle::AppKitHandle::empty();

        handle.ns_view = self.0;

        RawWindowHandle::AppKit(handle)
    }

    #[cfg(target_os = "windows")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = raw_window_handle::Win32Handle::empty();

        handle.hwnd = self.0;

        RawWindowHandle::Win32(handle)
    }

    #[cfg(target_os = "linux")]
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = raw_window_handle::XcbHandle::empty();

        handle.window = self.0 as u32;

        RawWindowHandle::Xcb(handle)
    }
}

/* Rounded rect
                                     ui.painter().rect_filled(
                                         ui.available_rect_before_wrap(),
                                         5.0,
                                         Color32::from_black_alpha(128),
                                     );

*/
