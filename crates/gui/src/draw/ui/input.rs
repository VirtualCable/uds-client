// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tiny_skia::{Color, FillRule, Paint, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::{HorizontalAlign, Layout, OwnedSection, Section, Text, VerticalAlign};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

use super::button::rounded_rect_path;

#[derive(Clone, Debug)]
pub struct TextInputStyle {
    pub bg_color: [u8; 4],
    pub border_color: [u8; 4],
    pub focused_border_color: [u8; 4],
    pub hover_border_color: [u8; 4],
    pub radius: f32,
    pub font_scale: f32,
    pub text_color: [f32; 4],
    pub cursor_color: [u8; 4],
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            bg_color: [22, 22, 28, 255],
            border_color: [60, 60, 75, 255],
            focused_border_color: [80, 140, 240, 255],
            hover_border_color: [90, 90, 110, 255],
            radius: 6.0,
            font_scale: 14.0,
            text_color: [0.95, 0.95, 0.98, 1.0],
            cursor_color: [100, 160, 255, 255],
        }
    }
}

pub struct TextInput {
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
    pub value: String,
    pub cursor_pos: usize, // In characters
    pub is_password: bool,
    pub is_focused: bool,
    pub is_hovered: bool,
    pub max_length: Option<usize>,
    pub style: TextInputStyle,
}

impl TextInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        x: f32,
        y: f32,
        w: u32,
        h: u32,
        initial_value: String,
        is_password: bool,
        max_length: Option<usize>,
        style: TextInputStyle,
    ) -> Self {
        let cursor_pos = initial_value.chars().count();
        Self {
            x,
            y,
            w,
            h,
            value: initial_value,
            cursor_pos,
            is_password,
            is_focused: false,
            is_hovered: false,
            max_length,
            style,
        }
    }

    pub fn contains(&self, phys_x: f32, phys_y: f32) -> bool {
        phys_x >= self.x
            && phys_x <= self.x + self.w as f32
            && phys_y >= self.y
            && phys_y <= self.y + self.h as f32
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        let old_hover = self.is_hovered;
        self.is_hovered = self.contains(phys_x, phys_y);
        self.is_hovered != old_hover
    }

    pub fn handle_click(&mut self, phys_x: f32, phys_y: f32) -> bool {
        if self.contains(phys_x, phys_y) {
            self.is_focused = true;
            // Move cursor to end of text
            self.cursor_pos = self.value.chars().count();
            true
        } else {
            if self.is_focused {
                self.is_focused = false;
                true
            } else {
                false
            }
        }
    }

    #[allow(clippy::collapsible_match)]
    pub fn handle_key(&mut self, key_ev: &KeyEvent) -> bool {
        if !self.is_focused || !key_ev.state.is_pressed() {
            return false;
        }

        let char_count = self.value.chars().count();

        match &key_ev.logical_key {
            Key::Named(NamedKey::Backspace) => {
                if self.cursor_pos > 0 {
                    let mut chars: Vec<char> = self.value.chars().collect();
                    chars.remove(self.cursor_pos - 1);
                    self.value = chars.into_iter().collect();
                    self.cursor_pos -= 1;
                    return true;
                }
            }
            Key::Named(NamedKey::Delete) => {
                if self.cursor_pos < char_count {
                    let mut chars: Vec<char> = self.value.chars().collect();
                    chars.remove(self.cursor_pos);
                    self.value = chars.into_iter().collect();
                    return true;
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    return true;
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                if self.cursor_pos < char_count {
                    self.cursor_pos += 1;
                    return true;
                }
            }
            Key::Named(NamedKey::Home) => {
                if self.cursor_pos != 0 {
                    self.cursor_pos = 0;
                    return true;
                }
            }
            Key::Named(NamedKey::End) => {
                if self.cursor_pos != char_count {
                    self.cursor_pos = char_count;
                    return true;
                }
            }
            _ => {}
        }

        // Printable text input from winit KeyEvent text
        if let Some(text) = &key_ev.text {
            let mut changed = false;
            for c in text.chars() {
                if !c.is_control() {
                    let cur_count = self.value.chars().count();
                    if self.max_length.is_none_or(|m| cur_count < m) {
                        let mut chars: Vec<char> = self.value.chars().collect();
                        chars.insert(self.cursor_pos, c);
                        self.value = chars.into_iter().collect();
                        self.cursor_pos += 1;
                        changed = true;
                    }
                }
            }
            if changed {
                return true;
            }
        }

        false
    }

    pub fn render(&self) -> (Vec<u8>, OwnedSection) {
        let mut pixmap = Pixmap::new(self.w, self.h).unwrap();
        let rect_path =
            rounded_rect_path(0.0, 0.0, self.w as f32, self.h as f32, self.style.radius);

        let mut fill = Paint::default();
        let bg = self.style.bg_color;
        fill.set_color(Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
        pixmap.fill_path(
            &rect_path,
            &fill,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut stroke_paint = Paint::default();
        let border = if self.is_focused {
            self.style.focused_border_color
        } else if self.is_hovered {
            self.style.hover_border_color
        } else {
            self.style.border_color
        };
        stroke_paint.set_color(Color::from_rgba8(
            border[0], border[1], border[2], border[3],
        ));
        let stroke = Stroke {
            width: if self.is_focused { 2.0 } else { 1.5 },
            ..Default::default()
        };
        pixmap.stroke_path(
            &rect_path,
            &stroke_paint,
            &stroke,
            Transform::identity(),
            None,
        );

        let pad_x = 10.0;
        let char_width = self.style.font_scale * 0.55;

        // Draw cursor line inside pixmap if focused
        if self.is_focused {
            let cx = pad_x + self.cursor_pos as f32 * char_width;
            if cx < self.w as f32 - 4.0 {
                let cy1 = 6.0;
                let cy2 = self.h as f32 - 6.0;
                let mut cursor_builder = tiny_skia::PathBuilder::new();
                cursor_builder.move_to(cx, cy1);
                cursor_builder.line_to(cx, cy2);
                if let Some(cpath) = cursor_builder.finish() {
                    let mut cpaint = Paint::default();
                    let cc = self.style.cursor_color;
                    cpaint.set_color(Color::from_rgba8(cc[0], cc[1], cc[2], cc[3]));
                    let cstroke = Stroke {
                        width: 1.5,
                        ..Default::default()
                    };
                    pixmap.stroke_path(&cpath, &cpaint, &cstroke, Transform::identity(), None);
                }
            }
        }

        let display_text = if self.is_password {
            "●".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        };

        let tx = self.x + pad_x;
        let ty = self.y + self.h as f32 / 2.0;

        let section = Section::default()
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Left)
                    .v_align(VerticalAlign::Center),
            )
            .add_text(
                Text::new(&display_text)
                    .with_scale(self.style.font_scale)
                    .with_color(self.style.text_color),
            )
            .with_screen_position((tx, ty))
            .to_owned();

        (pixmap.take(), section)
    }
}

pub struct ChoiceInput {
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
    pub choices: Vec<String>,
    pub selected_index: usize,
    pub is_focused: bool,
    pub is_hovered: bool,
    pub style: TextInputStyle,
}

impl ChoiceInput {
    pub fn new(
        x: f32,
        y: f32,
        w: u32,
        h: u32,
        choices: Vec<String>,
        selected_value: &str,
        style: TextInputStyle,
    ) -> Self {
        let selected_index = choices
            .iter()
            .position(|c| c == selected_value)
            .unwrap_or(0);
        Self {
            x,
            y,
            w,
            h,
            choices,
            selected_index,
            is_focused: false,
            is_hovered: false,
            style,
        }
    }

    pub fn value(&self) -> String {
        self.choices
            .get(self.selected_index)
            .cloned()
            .unwrap_or_default()
    }

    pub fn contains(&self, phys_x: f32, phys_y: f32) -> bool {
        phys_x >= self.x
            && phys_x <= self.x + self.w as f32
            && phys_y >= self.y
            && phys_y <= self.y + self.h as f32
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        let old_hover = self.is_hovered;
        self.is_hovered = self.contains(phys_x, phys_y);
        self.is_hovered != old_hover
    }

    pub fn handle_click(&mut self, phys_x: f32, phys_y: f32) -> bool {
        if self.contains(phys_x, phys_y) {
            self.is_focused = true;
            if !self.choices.is_empty() {
                self.selected_index = (self.selected_index + 1) % self.choices.len();
            }
            true
        } else {
            if self.is_focused {
                self.is_focused = false;
                true
            } else {
                false
            }
        }
    }

    pub fn handle_key(&mut self, key_ev: &KeyEvent) -> bool {
        if !self.is_focused || !key_ev.state.is_pressed() || self.choices.is_empty() {
            return false;
        }

        match &key_ev.logical_key {
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowUp) => {
                if self.selected_index == 0 {
                    self.selected_index = self.choices.len() - 1;
                } else {
                    self.selected_index -= 1;
                }
                true
            }
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowDown | NamedKey::Space) => {
                self.selected_index = (self.selected_index + 1) % self.choices.len();
                true
            }
            _ => false,
        }
    }

    pub fn render(&self) -> (Vec<u8>, OwnedSection) {
        let mut pixmap = Pixmap::new(self.w, self.h).unwrap();
        let rect_path =
            rounded_rect_path(0.0, 0.0, self.w as f32, self.h as f32, self.style.radius);

        let mut fill = Paint::default();
        let bg = self.style.bg_color;
        fill.set_color(Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
        pixmap.fill_path(
            &rect_path,
            &fill,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut stroke_paint = Paint::default();
        let border = if self.is_focused {
            self.style.focused_border_color
        } else if self.is_hovered {
            self.style.hover_border_color
        } else {
            self.style.border_color
        };
        stroke_paint.set_color(Color::from_rgba8(
            border[0], border[1], border[2], border[3],
        ));
        let stroke = Stroke {
            width: if self.is_focused { 2.0 } else { 1.5 },
            ..Default::default()
        };
        pixmap.stroke_path(
            &rect_path,
            &stroke_paint,
            &stroke,
            Transform::identity(),
            None,
        );

        // Draw dropdown arrow ▼ on the right side
        let arrow_x = self.w as f32 - 16.0;
        let arrow_y = self.h as f32 / 2.0 - 2.0;
        let mut arrow = tiny_skia::PathBuilder::new();
        arrow.move_to(arrow_x - 4.0, arrow_y);
        arrow.line_to(arrow_x + 4.0, arrow_y);
        arrow.line_to(arrow_x, arrow_y + 5.0);
        arrow.close();
        if let Some(apath) = arrow.finish() {
            let mut apaint = Paint::default();
            apaint.set_color(Color::from_rgba8(140, 140, 160, 255));
            pixmap.fill_path(
                &apath,
                &apaint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        let pad_x = 10.0;
        let tx = self.x + pad_x;
        let ty = self.y + self.h as f32 / 2.0;

        let cur_val = self.value();
        let section = Section::default()
            .with_layout(
                Layout::default()
                    .h_align(HorizontalAlign::Left)
                    .v_align(VerticalAlign::Center),
            )
            .add_text(
                Text::new(&cur_val)
                    .with_scale(self.style.font_scale)
                    .with_color(self.style.text_color),
            )
            .with_screen_position((tx, ty))
            .to_owned();

        (pixmap.take(), section)
    }
}
