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

pub(crate) fn word_wrap(text: &str, max_chars: usize) -> Vec<&str> {
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

#[cfg(test)]
mod tests {
    use super::word_wrap;

    #[test]
    fn empty_string() {
        assert_eq!(word_wrap("", 10), Vec::<&str>::new());
    }

    #[test]
    fn short_word() {
        assert_eq!(word_wrap("hola", 10), vec!["hola"]);
    }

    #[test]
    fn two_short_words() {
        assert_eq!(word_wrap("hola mundo", 10), vec!["hola mundo"]);
    }

    #[test]
    fn wrap_at_space() {
        let lines = word_wrap("hola mundo cruel", 10);
        assert_eq!(lines.len(), 2);
        for line in &lines {
            assert!(line.len() <= 10);
        }
    }

    #[test]
    fn hard_break_no_boundary() {
        let lines = word_wrap("supercalifragilistico", 10);
        assert!(lines.len() >= 2, "should break into multiple lines");
        for line in &lines {
            assert!(line.len() <= 10);
        }
    }

    #[test]
    fn exact_fit() {
        assert_eq!(word_wrap("1234567890", 10), vec!["1234567890"]);
    }

    #[test]
    fn hyphen_break() {
        let lines = word_wrap("inter-procedural-analysis", 12);
        assert!(lines.len() >= 2, "should break at hyphen");
    }
}
