// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, Paint, Pixmap, Stroke, Transform};
use tokio::sync::oneshot;
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, OwnedSection, Section, Text, VerticalAlign};
use winit::event::{KeyEvent, WindowEvent};
use winit::keyboard::{Key, NamedKey};

use crate::draw::ui::button::{Button, ButtonStyle, rounded_rect_path};
use crate::draw::ui::input::{ChoiceInput, TextInput, TextInputStyle};
use crate::draw::ui::text;
use crate::monitor;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialogFieldType {
    String,
    Password,
    Choice,
    #[serde(alias = "label", alias = "markdown")]
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogField {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub size: Option<u32>,
    #[serde(rename = "type")]
    pub field_type: DialogFieldType,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub choices: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogButton {
    pub name: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub close_on_click: bool,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogSpec {
    pub title: String,
    #[serde(default)]
    pub fields: Vec<DialogField>,
    #[serde(default)]
    pub buttons: Vec<DialogButton>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogFieldResult {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: DialogFieldType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogButtonResult {
    pub name: String,
    pub label: String,
    pub pushed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogResult {
    pub fields: Vec<DialogFieldResult>,
    pub buttons: Vec<DialogButtonResult>,
    pub pushed: Option<String>,
    pub values: HashMap<String, String>,
}

// Logical layout constants
const MARGIN: f32 = 24.0;
const TITLE_FONT_SIZE: f32 = 18.0;
const LABEL_FONT_SIZE: f32 = 13.0;
const INPUT_HEIGHT: f32 = 34.0;
const BUTTON_HEIGHT: f32 = 36.0;
const FIELD_GAP: f32 = 14.0;
const DEFAULT_WIDTH: f32 = 460.0;
const MIN_HEIGHT: f32 = 180.0;

fn scaled(val: f32) -> f32 {
    val * *monitor::SCALE_FACTOR as f32
}

pub struct DialogState {
    pub renderer: WgpuRenderer,
    pub window: Arc<winit::window::Window>,
    pub spec: DialogSpec,
    pub response: Arc<RwLock<Option<oneshot::Sender<DialogResult>>>>,
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale: f32,

    pub text_inputs: Vec<(String, DialogFieldType, TextInput)>,
    pub choice_inputs: Vec<(String, DialogFieldType, ChoiceInput)>,
    pub static_texts: Vec<(String, f32, f32, f32, [f32; 4])>, // text, x, y, scale, color
    pub buttons: Vec<(DialogButton, Button)>,
    pub last_mouse_pos: Option<(f32, f32)>,
}

impl DialogState {
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        spec: DialogSpec,
        response: Arc<RwLock<Option<oneshot::Sender<DialogResult>>>>,
    ) -> anyhow::Result<Self> {
        let (dw, dh) = monitor::size(0).unwrap_or((1920, 1080));
        let sf = monitor::scale(0) as f32;

        let ww = spec.width.map(|w| w as f32).unwrap_or(DEFAULT_WIDTH);
        let wh = spec
            .height
            .map(|h| h as f32)
            .unwrap_or_else(|| Self::calculate_height(&spec, ww));

        let px = (dw as f32 - ww * sf) / 2.0;
        let py = (dh as f32 - wh * sf) / 2.0;

        let window = Arc::new(
            event_loop.create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_title(&spec.title)
                    .with_inner_size(winit::dpi::LogicalSize::new(ww, wh))
                    .with_resizable(false)
                    .with_position(winit::dpi::PhysicalPosition::new(px as i32, py as i32)),
            )?,
        );

        let phys = window.inner_size();
        let scale = *monitor::SCALE_FACTOR as f32;
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;

        let mut state = Self {
            renderer,
            window,
            spec,
            response,
            phys_w: phys.width,
            phys_h: phys.height,
            scale,
            text_inputs: Vec::new(),
            choice_inputs: Vec::new(),
            static_texts: Vec::new(),
            buttons: Vec::new(),
            last_mouse_pos: None,
        };

        state.layout_elements();

        // Focus the first text input or choice if available
        if let Some((_, _, input)) = state.text_inputs.first_mut() {
            input.is_focused = true;
        } else if let Some((_, _, choice)) = state.choice_inputs.first_mut() {
            choice.is_focused = true;
        }

        Ok(state)
    }

    fn calculate_height(spec: &DialogSpec, width: f32) -> f32 {
        let mut h = MARGIN; // Top margin
        h += TITLE_FONT_SIZE + 16.0; // Title + gap

        let content_w = width - MARGIN * 2.0;

        for field in &spec.fields {
            match field.field_type {
                DialogFieldType::Text => {
                    let chars_per_line = (content_w / (LABEL_FONT_SIZE * 0.55)) as usize;
                    let num_lines = text::lines(&field.value, chars_per_line).len().max(1);
                    h += num_lines as f32 * (LABEL_FONT_SIZE * 1.5) + FIELD_GAP;
                }
                DialogFieldType::String | DialogFieldType::Password => {
                    // Label line
                    h += LABEL_FONT_SIZE + 6.0;
                    // Input line
                    h += INPUT_HEIGHT + FIELD_GAP;
                }
                DialogFieldType::Choice => {
                    h += LABEL_FONT_SIZE + 6.0;
                    h += INPUT_HEIGHT + FIELD_GAP;
                }
            }
        }

        // Buttons row
        h += BUTTON_HEIGHT + MARGIN * 1.5;

        h.max(MIN_HEIGHT)
    }

    fn layout_elements(&mut self) {
        let s = self.scale;
        let pw = self.phys_w as f32;
        let ph = self.phys_h as f32;

        self.text_inputs.clear();
        self.choice_inputs.clear();
        self.static_texts.clear();
        self.buttons.clear();

        let margin = scaled(MARGIN);
        let field_gap = scaled(FIELD_GAP);
        let input_h = scaled(INPUT_HEIGHT);
        let label_font_size = scaled(LABEL_FONT_SIZE);
        let content_w = pw - margin * 2.0;

        let mut cur_y = margin;

        // Title text
        self.static_texts.push((
            self.spec.title.clone(),
            margin,
            cur_y,
            scaled(TITLE_FONT_SIZE),
            [0.98, 0.98, 1.0, 1.0],
        ));
        cur_y += scaled(TITLE_FONT_SIZE) + scaled(16.0);

        for field in &self.spec.fields {
            match field.field_type {
                DialogFieldType::Text => {
                    let chars_per_line = (content_w / (label_font_size * 0.55)).max(10.0) as usize;
                    for line in text::lines(&field.value, chars_per_line) {
                        self.static_texts.push((
                            line.to_string(),
                            margin,
                            cur_y,
                            label_font_size,
                            [0.8, 0.8, 0.85, 1.0],
                        ));
                        cur_y += label_font_size * 1.4;
                    }
                    cur_y += field_gap;
                }
                DialogFieldType::String | DialogFieldType::Password => {
                    let label = field.label.clone().unwrap_or_else(|| field.name.clone());
                    self.static_texts.push((
                        label,
                        margin,
                        cur_y,
                        label_font_size,
                        [0.75, 0.78, 0.85, 1.0],
                    ));
                    cur_y += label_font_size + scaled(6.0);

                    let input_w = if let Some(size) = field.size {
                        let estimated = scaled(size as f32 * 10.0);
                        estimated.min(content_w).max(scaled(80.0))
                    } else {
                        content_w
                    };

                    let is_pass = field.field_type == DialogFieldType::Password;
                    let input = TextInput::new(
                        margin,
                        cur_y,
                        input_w as u32,
                        input_h as u32,
                        field.value.clone(),
                        is_pass,
                        field.size.map(|s| s as usize),
                        TextInputStyle {
                            font_scale: scaled(14.0),
                            radius: 6.0 * s,
                            ..Default::default()
                        },
                    );
                    self.text_inputs
                        .push((field.name.clone(), field.field_type.clone(), input));
                    cur_y += input_h + field_gap;
                }
                DialogFieldType::Choice => {
                    let label = field.label.clone().unwrap_or_else(|| field.name.clone());
                    self.static_texts.push((
                        label,
                        margin,
                        cur_y,
                        label_font_size,
                        [0.75, 0.78, 0.85, 1.0],
                    ));
                    cur_y += label_font_size + scaled(6.0);

                    let choices = field.choices.clone().unwrap_or_default();
                    let input = ChoiceInput::new(
                        margin,
                        cur_y,
                        content_w as u32,
                        input_h as u32,
                        choices,
                        &field.value,
                        TextInputStyle {
                            font_scale: scaled(14.0),
                            radius: 6.0 * s,
                            ..Default::default()
                        },
                    );
                    self.choice_inputs
                        .push((field.name.clone(), field.field_type.clone(), input));
                    cur_y += input_h + field_gap;
                }
            }
        }

        // Layout buttons at the bottom
        let bh = scaled(BUTTON_HEIGHT);
        let by = ph - bh - margin;
        let btn_gap = scaled(10.0);

        let num_buttons = self.spec.buttons.len().max(1);
        let bw = (content_w - (num_buttons as f32 - 1.0) * btn_gap) / num_buttons as f32;
        let bw = bw.min(scaled(130.0)).max(scaled(80.0));

        let total_btns_w = num_buttons as f32 * bw + (num_buttons as f32 - 1.0) * btn_gap;
        let mut bx = pw - margin - total_btns_w;

        for btn_spec in &self.spec.buttons {
            let style = if btn_spec.primary {
                ButtonStyle {
                    bg_color: [40, 100, 200, 255],
                    border_color: [60, 130, 240, 255],
                    hover_bg_color: [55, 120, 230, 255],
                    hover_border_color: [90, 160, 255, 255],
                    font_scale: scaled(14.0),
                    radius: 7.0 * s,
                    ..Default::default()
                }
            } else {
                ButtonStyle {
                    bg_color: [45, 45, 55, 255],
                    border_color: [80, 80, 100, 255],
                    hover_bg_color: [65, 65, 80, 255],
                    hover_border_color: [120, 120, 150, 255],
                    font_scale: scaled(14.0),
                    radius: 7.0 * s,
                    ..Default::default()
                }
            };

            let btn = Button::new(bx, by, bw as u32, bh as u32, btn_spec.label.clone(), style);
            self.buttons.push((btn_spec.clone(), btn));
            bx += bw + btn_gap;
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.phys_w = width;
        self.phys_h = height;
        self.scale = *monitor::SCALE_FACTOR as f32;
        self.layout_elements();
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        let mut changed = false;
        for (_, btn) in &mut self.buttons {
            if btn.handle_mouse_move(x, y) {
                changed = true;
            }
        }
        for (_, _, input) in &mut self.text_inputs {
            if input.handle_mouse_move(x, y) {
                changed = true;
            }
        }
        for (_, _, choice) in &mut self.choice_inputs {
            if choice.handle_mouse_move(x, y) {
                changed = true;
            }
        }
        changed
    }

    pub fn handle_click(&mut self, x: f32, y: f32) -> bool {
        // Check buttons
        let clicked_btn = self
            .buttons
            .iter()
            .find(|(spec, btn)| btn.contains(x, y) && spec.close_on_click)
            .map(|(spec, _)| spec.name.clone());

        if let Some(name) = clicked_btn {
            self.finish_with_button(Some(&name));
            return true;
        }

        // Check text inputs for focus
        let mut any_input_clicked = false;
        for (_, _, input) in &mut self.text_inputs {
            if input.handle_click(x, y) {
                any_input_clicked = true;
            }
        }

        // Check choices
        let mut any_choice_clicked = false;
        for (_, _, choice) in &mut self.choice_inputs {
            if choice.handle_click(x, y) {
                any_choice_clicked = true;
            }
        }

        if any_input_clicked || any_choice_clicked {
            self.window.request_redraw();
        }

        false
    }

    pub fn handle_key(&mut self, key_ev: &KeyEvent) -> bool {
        if !key_ev.state.is_pressed() {
            return false;
        }

        match &key_ev.logical_key {
            Key::Named(NamedKey::Escape) => {
                // Cancel dialog
                self.finish_with_button(None);
                return true;
            }
            Key::Named(NamedKey::Enter) => {
                // If enter pressed, trigger primary button or first closeOnClick button
                let primary_btn = self
                    .buttons
                    .iter()
                    .find(|(spec, _)| spec.primary && spec.close_on_click)
                    .or_else(|| self.buttons.iter().find(|(spec, _)| spec.close_on_click));

                if let Some((btn_spec, _)) = primary_btn {
                    let name = btn_spec.name.clone();
                    self.finish_with_button(Some(&name));
                    return true;
                }
            }
            Key::Named(NamedKey::Tab) => {
                // Cycle focus to next input
                self.cycle_focus();
                self.window.request_redraw();
                return false;
            }
            _ => {}
        }

        // Forward to focused text input
        let mut changed = false;
        for (_, _, input) in &mut self.text_inputs {
            if input.is_focused && input.handle_key(key_ev) {
                changed = true;
            }
        }

        // Forward to focused choice
        for (_, _, choice) in &mut self.choice_inputs {
            if choice.is_focused && choice.handle_key(key_ev) {
                changed = true;
            }
        }

        if changed {
            self.window.request_redraw();
        }

        false
    }

    fn cycle_focus(&mut self) {
        let total = self.text_inputs.len() + self.choice_inputs.len();
        if total == 0 {
            return;
        }

        let mut current_idx = None;
        for (i, (_, _, input)) in self.text_inputs.iter().enumerate() {
            if input.is_focused {
                current_idx = Some(i);
                break;
            }
        }
        if current_idx.is_none() {
            for (i, (_, _, choice)) in self.choice_inputs.iter().enumerate() {
                if choice.is_focused {
                    current_idx = Some(self.text_inputs.len() + i);
                    break;
                }
            }
        }

        // Clear all focus
        for (_, _, input) in &mut self.text_inputs {
            input.is_focused = false;
        }
        for (_, _, choice) in &mut self.choice_inputs {
            choice.is_focused = false;
        }

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % total,
            None => 0,
        };

        if next_idx < self.text_inputs.len() {
            self.text_inputs[next_idx].2.is_focused = true;
        } else {
            let choice_idx = next_idx - self.text_inputs.len();
            self.choice_inputs[choice_idx].2.is_focused = true;
        }
    }

    pub fn finish_with_button(&mut self, pushed_button_name: Option<&str>) {
        if let Some(tx) = self.response.write().unwrap().take() {
            let mut field_results = Vec::new();
            let mut values_map = HashMap::new();

            for (name, ftype, input) in &self.text_inputs {
                field_results.push(DialogFieldResult {
                    name: name.clone(),
                    field_type: ftype.clone(),
                    value: input.value.clone(),
                });
                values_map.insert(name.clone(), input.value.clone());
            }

            for (name, ftype, choice) in &self.choice_inputs {
                let val = choice.value();
                field_results.push(DialogFieldResult {
                    name: name.clone(),
                    field_type: ftype.clone(),
                    value: val.clone(),
                });
                values_map.insert(name.clone(), val);
            }

            let mut button_results = Vec::new();
            for (btn_spec, _) in &self.buttons {
                let is_pushed = pushed_button_name.is_some_and(|p| p == btn_spec.name);
                button_results.push(DialogButtonResult {
                    name: btn_spec.name.clone(),
                    label: btn_spec.label.clone(),
                    pushed: is_pushed,
                });
            }

            let result = DialogResult {
                fields: field_results,
                buttons: button_results,
                pushed: pushed_button_name.map(|s| s.to_string()),
                values: values_map,
            };

            let _ = tx.send(result);
        }
    }

    pub fn paint(&mut self) {
        let s = self.scale;
        let pw = self.phys_w;
        let ph = self.phys_h;

        self.renderer.reconfigure(pw, ph);

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();

        // 1. Panel background
        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect = rounded_rect_path(2.0, 2.0, pw as f32 - 4.0, ph as f32 - 4.0, 10.0 * s);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(30, 30, 36, 255));
        panel_pixmap.fill_path(
            &rect,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let stroke = Stroke {
            width: 1.5,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba8(65, 65, 80, 255));
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);

        let panel_idx = data.len();
        data.push(panel_pixmap.take());
        ov_descs.push((panel_idx, pw, ph, 0.0, 0.0));

        // 2. Static texts (Title, labels, text blocks)
        for (txt, x, y, scale, color) in &self.static_texts {
            sections.push(
                Section::default()
                    .with_layout(
                        Layout::default()
                            .h_align(HorizontalAlign::Left)
                            .v_align(VerticalAlign::Top),
                    )
                    .add_text(Text::new(txt).with_scale(*scale).with_color(*color))
                    .with_screen_position((*x, *y))
                    .to_owned(),
            );
        }

        // 3. Text inputs
        for (_, _, input) in &self.text_inputs {
            let (pix, sec) = input.render();
            let idx = data.len();
            data.push(pix);
            ov_descs.push((idx, input.w, input.h, input.x, input.y));
            sections.push(sec);
        }

        // 4. Choice inputs
        for (_, _, choice) in &self.choice_inputs {
            let (pix, sec) = choice.render();
            let idx = data.len();
            data.push(pix);
            ov_descs.push((idx, choice.w, choice.h, choice.x, choice.y));
            sections.push(sec);
        }

        // 5. Buttons
        for (_, btn) in &self.buttons {
            let (pix, sec) = btn.render();
            let idx = data.len();
            data.push(pix);
            ov_descs.push((idx, btn.w, btn.h, btn.x, btn.y));
            sections.push(sec);
        }

        let overlays: Vec<OverlayParams> = ov_descs
            .iter()
            .map(|(idx, w, h, x, y)| OverlayParams {
                rgba: &data[*idx],
                width: *w,
                height: *h,
                x: *x,
                y: *y,
                scale: 1.0,
            })
            .collect();

        self.renderer
            .update_and_render(&[], pw, ph, &overlays, &sections, None, None);
    }
}

impl crate::AppHandler {
    pub(crate) fn handle_dialog_event(&mut self, event: WindowEvent) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };

        let mut close = false;
        match event {
            WindowEvent::CloseRequested => {
                dialog.finish_with_button(None);
                close = true;
            }
            WindowEvent::RedrawRequested => {
                dialog.paint();
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = dialog.last_mouse_pos {
                    close = dialog.handle_click(pos.0, pos.1);
                }
                if !close {
                    dialog.window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                dialog.resize(size.width, size.height);
                dialog.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = dialog.window.inner_size();
                dialog.resize(size.width, size.height);
                dialog.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let px = position.x as f32;
                let py = position.y as f32;
                dialog.last_mouse_pos = Some((px, py));
                if dialog.handle_mouse_move(px, py) {
                    dialog.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { ref event, .. } => {
                close = dialog.handle_key(event);
            }
            _ => {}
        }

        if close && let Some(d) = self.dialog.take() {
            let wid = d.window.id();
            self.unregister_window(wid);
        }
    }
}
