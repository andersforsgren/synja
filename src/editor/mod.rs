mod audio_slider;
pub mod frame_history;
use crate::oscillator::WaveForm;
use crate::*;
use egui_extras::{Size, StripBuilder};
use egui_extras_xt::common::WidgetShape;
use egui_extras_xt::displays::{DisplayStylePreset, IndicatorButton, SegmentedDisplayWidget};
use egui_extras_xt::knobs::AudioKnob;
use nih_plug_egui::egui::{
    self, CentralPanel, Color32, FontData, FontDefinitions, FontFamily, Label, RichText, Ui,
    WidgetText,
};
use std::sync::Arc;

const WINDOW_WIDTH: u32 = 562;
const WINDOW_HEIGHT: u32 = 488;
const SHOW_FPS: bool = false;

pub fn default_editor_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub struct SynthUiState {
    pub edit_text: Mutex<EditText>,
    pub frame_history: Mutex<frame_history::FrameHistory>,
}

pub fn create_editor(
    params: Arc<SynthParams>,
    synth_ui_state: Arc<SynthUiState>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |ctx, _state| {
            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                "mathfont".to_owned(),
                FontData::from_static(include_bytes!("../fonts/STIXTwoMath-Regular.ttf")),
            );
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "mathfont".to_owned());

            ctx.set_fonts(fonts);
        },
        move |egui_ctx, setter, _state| {
            let time = egui_ctx.input().time;
            let ui_state = synth_ui_state.clone();

            let mut fps_history = ui_state.frame_history.lock().unwrap();
            fps_history.on_new_frame(time);

            CentralPanel::default().show(egui_ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
                ui.style_mut().spacing.slider_width = 64.0;
                StripBuilder::new(ui)
                    .size(Size::exact(54.0)) // top bar
                    .size(Size::remainder()) // control section
                    .size(Size::exact(if SHOW_FPS { 10.0 } else { 0.0 })) // footer
                    .vertical(|mut strip| {
                        // Top bar              
                        strip.strip(|builder| {
                            reset_edit_text(&ui_state);
                            builder.size(Size::remainder()).size(Size::exact(48.0)).horizontal(|mut strip| {
                                strip.cell(|ui| {
                                    ui.vertical(|ui| {

                                        let txt = ui_state.edit_text.lock().unwrap();
                                        let action_txt = match &*txt {
                                            EditText::Editing(s, _) => s,
                                            _ => "",
                                        };

                                        /*  TODO: Presets not implemented in VST3 / nih-plug version
                                        let (preset_index, preset_name) = (0, "".to_owned()); {
                                            let preset_index = state.ui_state.preset_index;
                                            let bank = state.preset_bank.lock().unwrap();
                                            let s = (*bank).presets[preset_index as usize].name.to_string();
                                            (preset_index, s)
                                        };
                                        // Top lcd (preset display)
                                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                        let preset_text = format!("{:>2}: {}", preset_index, preset_name);
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
                                                //state.change_preset(preset_index - 1);
                                            }
                                            if ui.button(">").clicked() {
                                                //state.change_preset(preset_index + 1);
                                            }
                                            if ui.button("Write").clicked() {
                                                //state.write_current_preset();
                                            }
                                        });
                                         */
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
                                    create_param_knob("Master", ui, setter, &params.master_gain, &ui_state, true, false);
                                });
                            });
                        });
                        // Main control section
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
                                        control_block("LFO", ui, |ui| {
                                            ui.horizontal(|ui| {
                                                waveform_button(ui, setter, &params.lfo_waveform, LfoWaveFormParameter::Sine);
                                                waveform_button(ui, setter, &params.lfo_waveform, LfoWaveFormParameter::Triangle);
                                                waveform_button(ui, setter, &params.lfo_waveform, LfoWaveFormParameter::Square);
                                            });
                                            ui.vertical_centered(|ui| {
                                                let host_sync = params.lfo_host_sync.value();
                                                ui.add_space(8.0);
                                                ui.add(
                                                    IndicatorButton::from_get_set(|new_val: Option<bool>| {
                                                        if let Some(v) = new_val {
                                                            setter.set_parameter(&params.lfo_host_sync, v);
                                                            set_edit_param(&ui_state, &params.lfo_host_sync);
                                                            v
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
                                                            setter.set_parameter(&params.lfo_key_trig, v);
                                                            set_edit_param(&ui_state, &params.lfo_key_trig);
                                                            v
                                                        } else {
                                                            params.lfo_key_trig.value()
                                                        }
                                                    })
                                                    .label("Retrig")
                                                    .style(DisplayStylePreset::DeLoreanAmber.style())
                                                    .interactive(!host_sync)
                                                    .height(32.0)
                                                    .width(48.0),
                                                );
                                                create_param_knob(
                                                    "Rate",
                                                    ui,
                                                    setter,
                                                    &params.lfo_freq,
                                                    &ui_state,
                                                    !params.lfo_host_sync.value(),
                                                    false,
                                                );
                                                ui.add(
                                                    IndicatorButton::from_get_set(|new_val: Option<bool>| {
                                                        if let Some(v) = new_val {
                                                            setter.set_parameter(&params.poly_mode, v);
                                                            v
                                                        } else {
                                                            params.poly_mode.value()
                                                        }
                                                    })
                                                    .label("Poly")
                                                    .style(DisplayStylePreset::DeLoreanAmber.style())
                                                    .height(32.0)
                                                    .width(48.0),
                                                );
                                                create_param_knob(
                                                    "Porta",
                                                    ui,
                                                    setter,
                                                    &params.portamento,
                                                    &ui_state,
                                                    !params.poly_mode.value(),
                                                    false,
                                                );
                                            });
                                        });
                                    }); // End LFO column

                                    // OSC1 column
                                    strip.cell(|ui| {
                                        control_block("OSC1", ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.horizontal(|ui| {
                                                    waveform_button(ui, setter, &params.osc1_waveform, WaveFormParameter::Saw); 
                                                    waveform_button(ui, setter, &params.osc1_waveform, WaveFormParameter::Square);
                                                    waveform_button(ui, setter, &params.osc1_waveform, WaveFormParameter::Sine);
                                                });
                                                create_param_knob("Oct", ui, setter, &params.osc1_octave, &ui_state, true, true);
                                                create_param_knob("Detune", ui, setter, &params.osc1_detune, &ui_state, true, true);
                                                param_knob("PW", ui, setter, &params.osc1_pulsewidth, &ui_state);
                                                create_param_knob("LFO", ui, setter, &params.lfo_osc1_detune_mod_depth, &ui_state, true, true);
                                            });
                                        });
                                    }); // End OSC1 column

                                    // OSC2 column
                                    strip.cell(|ui| {
                                        control_block("OSC2", ui, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.horizontal(|ui| {
                                                    waveform_button(ui, setter, &params.osc2_waveform, WaveFormParameter::Saw);
                                                    waveform_button(ui, setter, &params.osc2_waveform, WaveFormParameter::Square);
                                                    waveform_button(ui, setter, &params.osc2_waveform, WaveFormParameter::Sine);
                                                });
                                                create_param_knob("Oct", ui, setter, &params.osc2_octave, &ui_state, true, true);
                                                create_param_knob("Detune", ui, setter, &params.osc2_detune, &ui_state, true, true);
                                                param_knob("PW", ui, setter, &params.osc2_pulsewidth, &ui_state);
                                            });
                                        });
                                    }); // End OSC2 column

                                    // MIX/UNISON column
                                    strip.strip(|builder| {
                                        builder.size(Size::exact(96.0)).size(Size::remainder()).size(Size::exact(144.0)).vertical(
                                            |mut strip| {
                                                // Row 1/3: Mix
                                                strip.cell(|ui| {
                                                    control_block("MIX", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        StripBuilder::new(ui)
                                                            .size(Size::relative(0.5))
                                                            .size(Size::relative(0.5))
                                                            .horizontal(|mut strip| {
                                                                // OscLevel - Osc2Level
                                                                strip.cell(|ui| {
                                                                    param_knob("Osc 1", ui, setter, &params.osc1_level, &ui_state);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_knob("Osc 2", ui, setter, &params.osc2_level, &ui_state);
                                                                });
                                                            }); // End levels side by side
                                                    });
                                                });

                                                // Row 2/3: Unison
                                                strip.cell(|ui| {
                                                    control_block("UNISON", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        ui.vertical_centered(|ui| {
                                                            StripBuilder::new(ui)
                                                                .size(Size::exact(24.0))
                                                                .size(Size::remainder())
                                                                .vertical(|mut strip| {
                                                                    strip.cell(|ui| {
                                                                        let num_buttons = params.unison_voices.step_count().unwrap() + 1;
                                                                        let nvoices = params.unison_voices.value();
                                                                        const MIN: i32 = 1;
                                                                        const MAX: i32 = 7; // TODO how to get range from IntParam?
                                                                        StripBuilder::new(ui)
                                                                            .sizes(
                                                                                Size::relative(1.0 / num_buttons as f32),
                                                                                num_buttons,
                                                                            )
                                                                            .horizontal(|mut strip| {
                                                                                for n in MIN..=MAX {
                                                                                    strip.cell(|ui| {
                                                                                        if ui
                                                                                            .selectable_label(
                                                                                                nvoices == n,
                                                                                                format!("{}", n),
                                                                                            )
                                                                                            .clicked()
                                                                                        {
                                                                                            setter.set_parameter(&params.unison_voices, n);
                                                                                            set_edit_param(&ui_state, &params.unison_voices);
                                                                                        }
                                                                                    });
                                                                                }
                                                                            });
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        ui.add_space(4.0);
                                                                        StripBuilder::new(ui)
                                                                            .size(Size::relative(0.5))
                                                                            .size(Size::relative(0.5))
                                                                            .horizontal(|mut strip| {
                                                                                strip.cell(|ui| {
                                                                                    param_knob("Spread", ui, setter, &params.unison_stereo_spread, &ui_state);
                                                                                });
                                                                                strip.cell(|ui| {
                                                                                    param_knob("Detune", ui, setter, &params.unison_detune, &ui_state);
                                                                                });
                                                                            });
                                                                    });
                                                                });
                                                        });
                                                    });
                                                });

                                                // Row 3/3 Amp env
                                                strip.cell(|ui| {
                                                    control_block("AMP ENV", ui, |ui| {
                                                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                        ui.vertical_centered(|ui| {
                                                            StripBuilder::new(ui)
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .size(Size::relative(0.25))
                                                                .horizontal(|mut strip| {
                                                                    strip.cell(|ui| {
                                                                        param_slider("A", ui, setter, &params.amp_env_attack, &ui_state);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider("D", ui, setter, &params.amp_env_decay, &ui_state);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider("S", ui, setter, &params.amp_env_sustain, &ui_state);
                                                                    });
                                                                    strip.cell(|ui| {
                                                                        param_slider("R", ui, setter, &params.amp_env_release, &ui_state);
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
                                                control_block("FILTER", ui, |ui| {
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
                                                                            param_knob("Cutoff", ui, setter, &params.filter_cutoff, &ui_state);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob("Res", ui, setter, &params.fiter_resonance, &ui_state);
                                                                        });
                                                                    }); // End cutoff/resonance
                                                            });
                                                            strip.cell(|ui| {
                                                                StripBuilder::new(ui)
                                                                    .size(Size::relative(0.5))
                                                                    .size(Size::relative(0.5))
                                                                    .horizontal(|mut strip| {
                                                                        strip.cell(|ui| {
                                                                            create_param_knob("Env", ui, setter, &params.filter_env_mod_gain, &ui_state, true, true);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob("Key", ui, setter, &params.filter_key_track, &ui_state);
                                                                        });
                                                                    }); // End envmod/keytrack
                                                            });
                                                            strip.cell(|ui| {
                                                                StripBuilder::new(ui)
                                                                    .size(Size::relative(0.5))
                                                                    .size(Size::relative(0.5))
                                                                    .horizontal(|mut strip| {
                                                                        strip.cell(|ui| {
                                                                            create_param_knob("LFO", ui, setter, &params.lfo_filter_mod_depth, &ui_state, true, true);
                                                                        });
                                                                        strip.cell(|ui| {
                                                                            param_knob("Vel", ui, setter, &params.filter_velocity_mod, &ui_state);
                                                                        });
                                                                    }); // End filter lfo mod/velocity mod
                                                            });
                                                        });
                                                });
                                            });
                                            strip.cell(|ui| {
                                                control_block("FILTER ENV", ui, |ui| {
                                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                                                    ui.vertical_centered(|ui| {
                                                        StripBuilder::new(ui)
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .size(Size::relative(0.25))
                                                            .horizontal(|mut strip| {
                                                                strip.cell(|ui| {
                                                                    param_slider("A", ui, setter, &params.filter_env_attack, &ui_state);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider("D", ui, setter, &params.filter_env_decay, &ui_state);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider("S", ui, setter, &params.filter_env_sustain, &ui_state);
                                                                });
                                                                strip.cell(|ui| {
                                                                    param_slider("R", ui, setter, &params.filter_env_release, &ui_state);
                                                                });
                                                            });
                                                    });
                                                });
                                            });
                                        });
                                    }); // End FILTER column
                                }); // End main columns
                        });
                        if SHOW_FPS {
                            strip.cell(|ui|{
                                ui.label(format!("{:2} FPS", fps_history.fps()));
                            })
                        }
                    }); // End vertical display/main
            });
        },
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn reset_edit_text(ui_state: &Arc<SynthUiState>) {
    let mut txt = ui_state.edit_text.lock().unwrap();
    if let EditText::Editing(_, t) = &*txt {
        if now() - t > 2000 {
            *txt = EditText::None;
        }
    }
}

fn set_edit_param<P>(ui_state: &Arc<SynthUiState>, param: &P)
where
    P: Param,
{
    let plain = param.unmodulated_plain_value();
    let s = format!(
        "{}: {}",
        param.name(),
        param.normalized_value_to_string(param.preview_normalized(plain), true)
    );
    let mut txt = ui_state.edit_text.lock().unwrap();
    *txt = EditText::Editing(s, now());
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

fn waveform_button<E>(ui: &mut Ui, setter: &ParamSetter, param: &EnumParam<E>, btn_waveform: E)
where
    E: Copy + Enum + PartialEq + Into<WaveForm> + 'static,
{
    let osc_wf: WaveForm = param.value().into();
    let osc_btn_wf: WaveForm = btn_waveform.into();
    let symbol = match osc_btn_wf {
        WaveForm::Saw => '\u{2a58}',
        WaveForm::Square | WaveForm::UnipolarSquare => '\u{2293}',
        WaveForm::Sine => '\u{223f}',
        WaveForm::Triangle => '\u{2227}',
    };
    let label = egui::SelectableLabel::new(
        osc_wf == osc_btn_wf,
        egui::RichText::new(format!("{}", symbol)).monospace(),
    );
    let response = ui.add(label);
    if response.clicked() {
        setter.set_parameter(param, btn_waveform);
    }
}

fn param_knob<P>(
    label: impl Into<WidgetText>,
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    ui_state: &Arc<SynthUiState>,
) where
    P: Param,
{
    create_param_knob(label, ui, setter, param, ui_state, true, false);
}

fn create_param_knob<P>(
    label: impl Into<WidgetText>,
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    ui_state: &Arc<SynthUiState>,
    interactive: bool,
    symmetric: bool,
) where
    P: Param,
{
    ui.vertical_centered(|ui| {
        let knob_range = if symmetric { -0.5..=0.5 } else { 0.0..=1.0 };
        let offset = if symmetric { -0.5 } else { 0.0 }; // Offset between normalized value and knob value.
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        let knob = AudioKnob::from_get_set(|new_val: Option<f32>| {
            if let Some(k) = new_val {
                let v = k - offset;
                setter.set_parameter_normalized(param, v);
                set_edit_param(ui_state, param);
                v
            } else {
                if interactive {
                    let normalized_value = param.unmodulated_normalized_value();
                    normalized_value + offset
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

        // Snap
        let response = ui.add(knob);
        ui.add_space(8.0);
        ui.add(Label::new(label));

        if response.double_clicked() {
            setter.set_parameter(param, param.default_plain_value());
        }
        if response.drag_started() {
            setter.begin_set_parameter(param);
        } else if response.drag_released() {
            setter.end_set_parameter(param);
        }
    });
}

fn param_slider<P>(
    label: impl Into<WidgetText>,
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    ui_state: &Arc<SynthUiState>,
) where
    P: Param,
{
    ui.vertical(|ui| {
        let slider = crate::editor::audio_slider::AudioSlider::from_get_set(
            0.0..=1.0,
            |new_val: Option<f64>| {
                if let Some(v) = new_val {
                    setter.set_parameter_normalized(param, v as f32);
                    set_edit_param(ui_state, param);
                    v
                } else {
                    param.unmodulated_normalized_value() as f64
                }
            },
        )
        .text(label);
        ui.add_space(10.0);
        let response = ui.add(slider);
        if response.double_clicked() {
            setter.set_parameter(param, param.default_plain_value());
        }
        if response.drag_started() {
            setter.begin_set_parameter(param);
        } else if response.drag_released() {
            setter.end_set_parameter(param);
        }
    });
}

#[allow(dead_code)]
fn wrapper(ui: &Ui, color: Color32) {
    ui.painter()
        .rect_filled(ui.available_rect_before_wrap(), 0.0, color);
}
