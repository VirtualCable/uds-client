// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tiny_skia::{
    Color, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, SpreadMode, Stroke,
    Transform,
};

pub struct Wave {
    pub y_base: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub offset: f32,
    pub thickness: f32,
    pub opacity: f32,
}

impl Wave {
    pub fn default_set() -> Vec<Self> {
        vec![
            Wave {
                y_base: 0.4,
                amplitude: 25.0,
                speed: 0.04,
                offset: 0.0,
                thickness: 5.0,
                opacity: 0.5,
            },
            Wave {
                y_base: 0.42,
                amplitude: 20.0,
                speed: 0.06,
                offset: 2.0,
                thickness: 3.5,
                opacity: 0.3,
            },
            Wave {
                y_base: 0.38,
                amplitude: 22.0,
                speed: 0.03,
                offset: 4.0,
                thickness: 6.0,
                opacity: 0.25,
            },
        ]
    }
}

pub fn render(width: u32, height: u32, time: f32, scale: f32, waves: &[Wave]) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height).unwrap();

    for (idx, wave) in waves.iter().enumerate() {
        let mut pb = PathBuilder::new();
        let step = 15.0 * scale;
        let y_base = wave.y_base * height as f32;

        let mut first = true;
        let mut x = -step;
        while x <= width as f32 + step {
            let val = x * 0.005 + time * wave.speed + wave.offset;
            let y = y_base
                + (val).sin() * wave.amplitude * scale
                + (val * 0.3).cos() * (wave.amplitude * 0.4 * scale);

            if first {
                pb.move_to(x, y);
                first = false;
            } else {
                pb.line_to(x, y);
            }
            x += step;
        }

        if let Some(path) = pb.finish() {
            let mut stroke_paint = Paint::default();
            let main_color = if idx % 2 == 0 {
                [100, 140, 255]
            } else {
                [160, 100, 255]
            };
            let grad = LinearGradient::new(
                Point::from_xy(0.0, 0.0),
                Point::from_xy(width as f32, 0.0),
                vec![
                    GradientStop::new(
                        0.0,
                        Color::from_rgba8(main_color[0], main_color[1], main_color[2], 0),
                    ),
                    GradientStop::new(
                        0.5,
                        Color::from_rgba8(
                            main_color[0],
                            main_color[1],
                            main_color[2],
                            (wave.opacity * 255.0) as u8,
                        ),
                    ),
                    GradientStop::new(
                        1.0,
                        Color::from_rgba8(main_color[0], main_color[1], main_color[2], 0),
                    ),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap();
            stroke_paint.shader = grad;
            let stroke = Stroke {
                width: wave.thickness * scale,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
        }
    }

    pixmap.take()
}
