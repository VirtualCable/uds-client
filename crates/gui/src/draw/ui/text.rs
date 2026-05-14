use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

/// Wrap a long string into multiple `OwnedSection`s, splitting at word boundaries.
/// `max_chars_per_line` — approximate max characters per line.
/// `x`, `y` — starting screen position.
/// `line_height` — vertical spacing between lines.
pub fn wrap(
    message: &str,
    max_chars_per_line: usize,
    font_scale: f32,
    color: [f32; 4],
    x: f32,
    y: f32,
    line_height: f32,
) -> Vec<OwnedSection> {
    let mut sections = Vec::new();
    let mut cur_y = y;
    for line in word_wrap(message, max_chars_per_line) {
        sections.push(
            Section::default()
                .add_text(
                    Text::new(line)
                        .with_scale(font_scale)
                        .with_color(color),
                )
                .with_screen_position((x, cur_y))
                .to_owned(),
        );
        cur_y += line_height;
    }
    sections
}

fn word_wrap(text: &str, max_chars: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + max_chars).min(text.len());
        if end >= text.len() {
            lines.push(&text[start..]);
            break;
        }
        // Try to break at a word boundary within the last ~20% of the line
        let search_start = (end as i32 - (max_chars as i32 / 5).max(1)).max(start as i32) as usize;
        if let Some(brk) = text[search_start..end].rfind(|c: char| [' ', '-'].contains(&c)) {
            let split = search_start + brk;
            lines.push(&text[start..split]);
            start = split + 1; // skip the space/dash
        } else {
            // Hard break if no word boundary found
            lines.push(&text[start..end]);
            start = end;
        }
    }
    lines
}
