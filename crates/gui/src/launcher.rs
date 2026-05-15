use crate::monitor;
// BSD 3-Clause License, Authors: Adolfo Gómez
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use std::sync::Arc;
use std::time::Instant;
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

use crate::draw::ui::progress;
#[cfg(feature = "test-ui")]
use crate::draw::ui::button::{self, ButtonStyle};

#[derive(Default)]
#[allow(dead_code)]
pub enum LauncherInner {
    #[default]
    Invisible,
    #[cfg(feature = "test-ui")]
    Test {
        buttons: Vec<(&'static str, LaunchAction)>,
        request: Option<LaunchAction>,
        hover_idx: Option<usize>,
    },
    Progress {
        pct: u8,
        message: String,
        start: Instant,
        progress_duration_secs: u32,
        phase: ProgressPhase,
        auto_animate: bool,
        hover_idx: Option<usize>,
        cancelled: bool,
    },
}

#[derive(Default, PartialEq, Debug)]
pub enum ProgressPhase {
    #[default]
    Connecting,
    Connected,
}

#[cfg(feature = "test-ui")]
#[derive(Clone)]
pub enum LaunchAction {
    ShowProgress,
    GoInvisible,
    ShowWarning,
    ShowError,
    ShowYesNo,
    ConnectRdp,
    ConnectRail,
}

impl LauncherInner {
    #[cfg(feature = "test-ui")]
    pub fn new_test() -> Self {
        LauncherInner::Test {
            buttons: vec![
                ("RDP Connect", LaunchAction::ConnectRdp),
                ("RDP RAIL Notepad", LaunchAction::ConnectRail),
                ("Progress", LaunchAction::ShowProgress),
                ("Invisible", LaunchAction::GoInvisible),
                ("Warning", LaunchAction::ShowWarning),
                ("Error", LaunchAction::ShowError),
                ("Yes/No", LaunchAction::ShowYesNo),
            ],
            request: None,
            hover_idx: None,
        }
    }

    pub fn handle_mouse_move(&mut self, logical_x: f32, logical_y: f32) -> bool {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test { buttons, hover_idx, .. } => {
                let old_hover = *hover_idx;
                *hover_idx = None;
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                let bh = crate::monitor::scaled_val(28) as f32;
                let bw = crate::monitor::scaled_val(260) as f32;
                let sy = 42.0 * s;
                let bx = 70.0 * s;
                
                for (i, _) in buttons.iter().enumerate() {
                    let by = sy + i as f32 * (bh + 6.0 * s);
                    if y >= by && y <= by + bh && x >= bx && x <= bx + bw {
                        *hover_idx = Some(i);
                        break;
                    }
                }
                *hover_idx != old_hover
            }
            LauncherInner::Progress { hover_idx, .. } => {
                let old_hover = *hover_idx;
                *hover_idx = None;
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                
                // Cancel button position (same logic as in paint_launcher)
                let bw = crate::monitor::scaled_val(120) as f32;
                let bh = crate::monitor::scaled_val(32) as f32;
                let bx = (400.0 * s - bw) / 2.0;
                let by = (300.0 * s) - bh - 25.0 * s;

                if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                    *hover_idx = Some(0);
                }
                *hover_idx != old_hover
            }
            _ => {
                let _ = (logical_x, logical_y);
                false
            }
        }
    }

    pub fn handle_click(&mut self, logical_x: f32, logical_y: f32) {
        match self {
            #[cfg(feature = "test-ui")]
            LauncherInner::Test { buttons, request, .. } => {
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                let bh = crate::monitor::scaled_val(28) as f32;
                let bw = crate::monitor::scaled_val(260) as f32;
                let sy = 42.0 * s;
                let bx = 70.0 * s;
                
                for (i, _) in buttons.iter().enumerate() {
                    let by = sy + i as f32 * (bh + 6.0 * s);
                    if y >= by && y <= by + bh && x >= bx && x <= bx + bw {
                        *request = Some(buttons[i].1.clone());
                        break;
                    }
                }
            }
            LauncherInner::Progress { cancelled, .. } => {
                let s = *crate::monitor::SCALE_FACTOR as f32;
                let x = logical_x * s;
                let y = logical_y * s;
                let bw = crate::monitor::scaled_val(120) as f32;
                let bh = crate::monitor::scaled_val(32) as f32;
                let bx = (400.0 * s - bw) / 2.0;
                let by = (300.0 * s) - bh - 25.0 * s;

                if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                    *cancelled = true;
                }
            }
            _ => {
                let _ = (logical_x, logical_y);
            }
        }
    }
    #[cfg(feature = "test-ui")]
    pub fn take_request(&mut self) -> Option<LaunchAction> {
        match self {
            LauncherInner::Test { request, .. } => request.take(),
            _ => None,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        match self {
            LauncherInner::Progress { cancelled, .. } => *cancelled,
            _ => false,
        }
    }
}

pub struct Wave {
    pub y_base: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub offset: f32,
    pub thickness: f32,
    pub opacity: f32,
}

pub struct TestingLauncherState {
    pub window: Option<Arc<winit::window::Window>>,
    pub renderer: Option<WgpuRenderer>,
    pub inner: LauncherInner,
    pub last_mouse_pos: Option<(f32, f32)>,
    pub animation_time: f32,
    pub waves: Vec<Wave>,
}

impl Default for TestingLauncherState {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            inner: LauncherInner::Invisible,
            last_mouse_pos: None,
            animation_time: 0.0,
            waves: vec![
                Wave { y_base: 0.4, amplitude: 25.0, speed: 0.04, offset: 0.0, thickness: 5.0, opacity: 0.5 },
                Wave { y_base: 0.42, amplitude: 20.0, speed: 0.06, offset: 2.0, thickness: 3.5, opacity: 0.3 },
                Wave { y_base: 0.38, amplitude: 22.0, speed: 0.03, offset: 4.0, thickness: 6.0, opacity: 0.25 },
            ],
        }
    }
}

#[cfg(feature = "test-ui")]
pub fn paint_launcher(state: &mut TestingLauncherState) {
    let renderer = match &mut state.renderer {
        Some(r) => r,
        None => return,
    };
    let logo = crate::logo::load_logo();
    let phys = state
        .window
        .as_ref()
        .map(|w| w.inner_size())
        .unwrap_or(winit::dpi::PhysicalSize::new(400, 300));
    let pw = phys.width;
    let ph = phys.height;
    let s = *monitor::SCALE_FACTOR as f32;

    // Ensure renderer is configured for this size (fixes text alignment)
    renderer.reconfigure(pw, ph);

    let mut sections: Vec<OwnedSection> = Vec::new();
    let mut data: Vec<Vec<u8>> = Vec::new();
    let logo_idx = data.len();
    data.push(logo.rgba);

    struct OvDesc {
        data_idx: usize,
        w: u32,
        h: u32,
        x: f32,
        y: f32,
        scale: f32,
    }
    let mut ov_descs: Vec<OvDesc> = Vec::new();

    match &state.inner {
        LauncherInner::Invisible => {}
        #[cfg(feature = "test-ui")]
        LauncherInner::Test { .. } => {}
        LauncherInner::Progress {
            pct,
            message,
            start,
            progress_duration_secs,
            phase,
            auto_animate,
            hover_idx,
            ..
        } => {
            let elapsed = start.elapsed().as_secs_f32();
            let pct = if *auto_animate {
                (elapsed / *progress_duration_secs as f32 * 100.0).min(100.0)
            } else {
                *pct as f32
            };
            let message = if *auto_animate {
                match phase {
                    ProgressPhase::Connecting => "Connecting to RDP server...",
                    ProgressPhase::Connected => "Connected.",
                }
            } else {
                message.as_str()
            };

            // 1. Draw Panel Background + Waves
            let panel_data = {
                let mut pixmap = tiny_skia::Pixmap::new(pw, ph).unwrap();
                let rect = tiny_skia::Rect::from_xywh(0.0, 0.0, pw as f32, ph as f32).unwrap();
                let mut paint = tiny_skia::Paint::default();
                paint.set_color(tiny_skia::Color::from_rgba8(30, 30, 35, 255));
                pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                
                // Draw Waves
                let time = state.animation_time;
                for (idx, wave) in state.waves.iter().enumerate() {
                    let mut pb = tiny_skia::PathBuilder::new();
                    let step = 15.0 * s;
                    let y_base = wave.y_base * ph as f32;
                    
                    let mut first = true;
                    let mut x = -step;
                    while x <= pw as f32 + step {
                        let val = x * 0.005 + time * wave.speed + wave.offset;
                        let y = y_base + 
                                (val).sin() * wave.amplitude * s + 
                                (val * 0.3).cos() * (wave.amplitude * 0.4 * s);
                        
                        if first {
                            pb.move_to(x, y);
                            first = false;
                        } else {
                            pb.line_to(x, y);
                        }
                        x += step;
                    }
                    
                    if let Some(path) = pb.finish() {
                        let mut stroke_paint = tiny_skia::Paint::default();
                        let main_color = if idx % 2 == 0 { [100, 140, 255] } else { [160, 100, 255] };
                        let grad = tiny_skia::LinearGradient::new(
                            tiny_skia::Point::from_xy(0.0, 0.0),
                            tiny_skia::Point::from_xy(pw as f32, 0.0),
                            vec![
                                tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(main_color[0], main_color[1], main_color[2], 0)),
                                tiny_skia::GradientStop::new(0.5, tiny_skia::Color::from_rgba8(main_color[0], main_color[1], main_color[2], (wave.opacity * 255.0) as u8)),
                                tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(main_color[0], main_color[1], main_color[2], 0)),
                            ],
                            tiny_skia::SpreadMode::Pad,
                            tiny_skia::Transform::identity(),
                        ).unwrap();
                        stroke_paint.shader = grad;
                        let stroke = tiny_skia::Stroke { width: wave.thickness * s, ..Default::default() };
                        pixmap.stroke_path(&path, &stroke_paint, &stroke, tiny_skia::Transform::identity(), None);
                    }
                }

                // Optional: subtle border for the whole window
                let mut border = tiny_skia::Paint::default();
                border.set_color(tiny_skia::Color::from_rgba8(60, 60, 75, 255));
                let stroke = tiny_skia::Stroke { width: 2.0, ..Default::default() };
                let path = tiny_skia::PathBuilder::from_rect(rect);
                pixmap.stroke_path(&path, &border, &stroke, tiny_skia::Transform::identity(), None);
                pixmap.take()
            };
            let panel_idx = data.len();
            data.push(panel_data);
            ov_descs.push(OvDesc { data_idx: panel_idx, w: pw, h: ph, x: 0.0, y: 0.0, scale: 1.0 });

            // 2. Percentage text
            let fs = monitor::scaled_val(32) as f32; // Bigger percentage
            sections.push(
                Section::default()
                    .with_layout(wgpu_text::glyph_brush::Layout::default().h_align(wgpu_text::glyph_brush::HorizontalAlign::Center))
                    .add_text(
                        Text::new(&format!("{}%", pct as u8))
                            .with_scale(fs)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((pw as f32 / 2.0, 140.0 * s))
                    .to_owned(),
            );

            // 3. Progress bar
            let bw = monitor::scaled_val(280) as u32;
            let bh = monitor::scaled_val(12) as u32;
            let bx = (pw as f32 - bw as f32) / 2.0;
            let by = 190.0 * s;
            let i = data.len();
            data.push(progress::render(pct, bw, bh));
            ov_descs.push(OvDesc {
                data_idx: i,
                w: bw,
                h: bh,
                x: bx,
                y: by,
                scale: 1.0,
            });

            // 4. Status message below the bar
            let msg_fs = monitor::scaled_val(13) as f32;
            sections.push(
                Section::default()
                    .with_layout(wgpu_text::glyph_brush::Layout::default().h_align(wgpu_text::glyph_brush::HorizontalAlign::Center))
                    .add_text(
                        Text::new(message)
                            .with_scale(msg_fs)
                            .with_color([0.7, 0.7, 0.9, 1.0]),
                    )
                    .with_screen_position((pw as f32 / 2.0, by + bh as f32 + 10.0 * s))
                    .to_owned(),
            );

            // 5. CANCEL Button
            let btn_w = monitor::scaled_val(120) as u32;
            let btn_h = monitor::scaled_val(32) as u32;
            let btn_x = (pw as f32 - btn_w as f32) / 2.0;
            let btn_y = (ph as f32) - btn_h as f32 - 25.0 * s;
            
            let btn_style = ButtonStyle {
                font_scale: monitor::scaled_val(13) as f32,
                bg_color: if *hover_idx == Some(0) { [80, 45, 45, 255] } else { [60, 35, 35, 255] }, // Reddish dark
                border_color: if *hover_idx == Some(0) { [150, 80, 80, 255] } else { [100, 60, 60, 255] },
                ..ButtonStyle::default()
            };
            let (btn_data, btn_text) = button::render(btn_x, btn_y, btn_w, btn_h, "CANCEL", &btn_style);
            let b_idx = data.len();
            data.push(btn_data);
            ov_descs.push(OvDesc { data_idx: b_idx, w: btn_w, h: btn_h, x: btn_x, y: btn_y, scale: 1.0 });
            sections.push(btn_text);
        }
    }

    // Test buttons
    #[cfg(feature = "test-ui")]
    if let LauncherInner::Test { buttons, hover_idx, .. } = &state.inner {
        let bh = monitor::scaled_val(28) as u32;
        let bw = monitor::scaled_val(260) as u32;
        let sy = 42.0 * s;
        let bx = 70.0 * s;
        for (i, (label, _)) in buttons.iter().enumerate() {
            let y = sy + i as f32 * (bh as f32 + 6.0 * s);
            let style = ButtonStyle {
                font_scale: monitor::scaled_val(14) as f32,
                bg_color: if hover_idx == &Some(i) { [0x70, 0x70, 0x90, 0xFF] } else { [0x50, 0x50, 0x70, 0xFF] },
                border_color: if hover_idx == &Some(i) { [0x90, 0x90, 0xB0, 0xFF] } else { [0x70, 0x70, 0x90, 0xFF] },
                ..ButtonStyle::default()
            };
            let (btn_data, btn_text) = button::render(bx, y, bw, bh, label, &style);
            let di = data.len();
            data.push(btn_data);
            ov_descs.push(OvDesc {
                data_idx: di,
                w: bw,
                h: bh,
                x: bx,
                y,
                scale: 1.0,
            });
            sections.push(btn_text);
        }
    }

    // Logo (ALWAYS on top)
    ov_descs.push(OvDesc {
        data_idx: logo_idx,
        w: logo.width,
        h: logo.height,
        x: (pw as f32 - logo.width as f32 * s) / 2.0,
        y: (35.0 * s).min(ph as f32 - logo.height as f32 * s),
        scale: s,
    });

    // Phase 2: build overlays from stable data
    let mut overlays = Vec::with_capacity(ov_descs.len());
    for d in &ov_descs {
        overlays.push(OverlayParams {
            rgba: &data[d.data_idx],
            width: d.w,
            height: d.h,
            x: d.x,
            y: d.y,
            scale: d.scale,
        });
    }

    renderer.update_and_render(&[], pw, ph, &overlays, &sections, None);
}
