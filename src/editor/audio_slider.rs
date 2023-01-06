use egui::*;
use std::ops::RangeInclusive;

/// Combined into one function (rather than two) to make it easier
/// for the borrow checker.
type GetSetValue<'a> = Box<dyn 'a + FnMut(Option<f64>) -> f64>;

fn get(get_set_value: &mut GetSetValue<'_>) -> f64 {
    (get_set_value)(None)
}

fn set(get_set_value: &mut GetSetValue<'_>, value: f64) {
    (get_set_value)(Some(value));
}

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct AudioSlider<'a> {
    get_set_value: GetSetValue<'a>,
    range: RangeInclusive<f64>,
    text: WidgetText,
    step: Option<f64>,
}

impl<'a> AudioSlider<'a> {
    pub fn from_get_set(
        range: RangeInclusive<f64>,
        get_set_value: impl 'a + FnMut(Option<f64>) -> f64,
    ) -> Self {
        Self {
            get_set_value: Box::new(get_set_value),
            range,
            text: Default::default(),
            step: None,
        }
    }

    pub fn text(mut self, text: impl Into<WidgetText>) -> Self {
        self.text = text.into();
        self
    }

    fn get_value(&mut self) -> f64 {
        let value = get(&mut self.get_set_value);
        let start = *self.range.start();
        let end = *self.range.end();
        value.clamp(start.min(end), start.max(end))
    }

    fn set_value(&mut self, mut value: f64) {
        let start = *self.range.start();
        let end = *self.range.end();
        value = value.clamp(start.min(end), start.max(end));
        if let Some(step) = self.step {
            value = (value / step).round() * step;
        }
        set(&mut self.get_set_value, value);
    }

    fn range(&self) -> RangeInclusive<f64> {
        self.range.clone()
    }

    /// For instance, `position` is the mouse position and `position_range` is the physical location of the slider on the screen.
    fn value_from_position(&self, position: f32, position_range: RangeInclusive<f32>) -> f64 {
        let normalized = remap_clamp(position, position_range, 0.0..=1.0) as f64;
        value_from_normalized(normalized, self.range())
    }

    fn position_from_value(&self, value: f64, position_range: RangeInclusive<f32>) -> f32 {
        let normalized = normalized_from_value(value, self.range());
        lerp(position_range, normalized as f32)
    }
}

impl<'a> AudioSlider<'a> {
    /// Just the slider, no text
    fn allocate_slider_space(&self, ui: &mut Ui, thickness: f32) -> Response {
        let desired_size = vec2(thickness, ui.spacing().slider_width);
        ui.allocate_response(desired_size, Sense::click_and_drag())
    }

    /// Just the slider, no text
    fn slider_ui(&mut self, ui: &mut Ui, response: &Response) {
        let rect = &response.rect;
        let position_range = self.position_range(rect);

        if let Some(pointer_position_2d) = response.interact_pointer_pos() {
            let position = pointer_position_2d.y;
            let new_value = self.value_from_position(position, position_range.clone());
            self.set_value(new_value);
        }

        if response.has_focus() {
            let decrement = ui.input().num_presses(Key::ArrowDown);
            let increment = ui.input().num_presses(Key::ArrowUp);
            let kb_step = increment as f32 - decrement as f32;

            if kb_step != 0.0 {
                let prev_value = self.get_value();
                let prev_position = self.position_from_value(prev_value, position_range.clone());
                let new_position = prev_position + kb_step;
                let new_value = match self.step {
                    Some(step) => prev_value + (kb_step as f64 * step),
                    _ => self.value_from_position(new_position, position_range.clone()),
                };
                self.set_value(new_value);
            }
        }

        // Paint it:
        if ui.is_rect_visible(response.rect) {
            let value = self.get_value();
            let rail_width = 4.0;
            let rail_rect = self.rail_rect(rect, rail_width);
            let position_1d = self.position_from_value(value, position_range);
            let slide_rect = self.slide_rect(rect, position_1d, rail_width);
            let visuals = ui.style().interact(response);
            ui.painter().add(epaint::RectShape {
                rect: rail_rect,
                rounding: ui.visuals().widgets.inactive.rounding,
                fill: ui.visuals().widgets.inactive.bg_fill,
                stroke: visuals.bg_stroke,
            });
            ui.painter().add(epaint::RectShape {
                rect: slide_rect,
                rounding: Rounding::none(),
                fill: Color32::from_rgb(215, 173, 29),
                stroke: visuals.bg_stroke,
            });
        }
    }

    fn position_range(&self, rect: &Rect) -> RangeInclusive<f32> {
        let handle_radius = self.handle_radius(rect);
        (rect.bottom() - handle_radius)..=(rect.top() + handle_radius)
    }

    fn rail_rect(&self, rect: &Rect, width: f32) -> Rect {
        Rect::from_min_max(
            pos2(rect.center().x - width, rect.top()),
            pos2(rect.center().x + width, rect.bottom()),
        )
    }

    fn slide_rect(&self, rect: &Rect, position_1d: f32, width: f32) -> Rect {
        Rect::from_min_max(
            pos2(rect.center().x - width, position_1d),
            pos2(rect.center().x + width, rect.bottom()),
        )
    }

    fn handle_radius(&self, rect: &Rect) -> f32 {
        rect.width() / 4.0
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let thickness = ui
            .text_style_height(&TextStyle::Body)
            .at_least(ui.spacing().interact_size.y);
        let response = self.allocate_slider_space(ui, thickness);
        self.slider_ui(ui, &response);

        if !self.text.is_empty() {
            ui.add(Label::new(self.text.clone()).wrap(false));
        }

        response
    }
}

impl<'a> Widget for AudioSlider<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let old_value = self.get_value();
        let inner_response = ui.vertical_centered(|ui| self.add_contents(ui));
        let mut response = inner_response.inner | inner_response.response;
        let value = self.get_value();
        response.changed = value != old_value;
        response.widget_info(|| WidgetInfo::slider(value, self.text.text()));
        response
    }
}

fn value_from_normalized(normalized: f64, range: RangeInclusive<f64>) -> f64 {
    let (min, max) = (*range.start(), *range.end());

    if min.is_nan() || max.is_nan() {
        f64::NAN
    } else if min == max {
        min
    } else if min > max {
        value_from_normalized(1.0 - normalized, max..=min)
    } else if normalized <= 0.0 {
        min
    } else if normalized >= 1.0 {
        max
    } else {
        lerp(range, normalized.clamp(0.0, 1.0))
    }
}

fn normalized_from_value(value: f64, range: RangeInclusive<f64>) -> f64 {
    let (min, max) = (*range.start(), *range.end());

    if min.is_nan() || max.is_nan() {
        f64::NAN
    } else if min == max {
        0.5 // empty range, show center of slider
    } else if min > max {
        1.0 - normalized_from_value(value, max..=min)
    } else if value <= min {
        0.0
    } else if value >= max {
        1.0
    } else {
        remap_clamp(value, range, 0.0..=1.0)
    }
}
