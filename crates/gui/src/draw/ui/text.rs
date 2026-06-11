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
                .add_text(Text::new(line).with_scale(font_scale).with_color(color))
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
        // Advance by at most max_chars **characters** (respecting UTF-8 boundaries)
        let end = match text[start..].char_indices().nth(max_chars) {
            Some((offset, _)) => start + offset,
            None => {
                lines.push(&text[start..]);
                break;
            }
        };

        // Try to break at a word boundary within the last ~20% of the line
        let search_window = (max_chars / 5).max(1);
        let search_start = match text[start..end]
            .char_indices()
            .rev()
            .nth(search_window)
        {
            Some((offset, _)) => start + offset,
            None => start,
        };

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

    #[test]
    fn utf8_multi_byte_no_panic() {
        // Each accented char is 2 bytes in UTF-8; old code could panic here
        let lines = word_wrap("áéíóúáéíóúáéíóú", 5);
        assert!(lines.len() >= 3, "should break multiple times");
        // Verify every slice is valid UTF-8 (no panic = success)
        for line in &lines {
            assert!(line.len() <= 15, "byte length may exceed max_chars but must not panic");
        }
    }

    #[test]
    fn utf8_mixed_content() {
        let lines = word_wrap("café con leche y croissant", 8);
        assert!(lines.len() >= 2, "should break at word boundaries");
        // Ensure no line exceeds max_chars characters
        for line in &lines {
            assert!(line.chars().count() <= 8, "char count must respect max_chars");
        }
    }

    #[test]
    fn utf8_hard_break() {
        // Long run of multi-byte chars with no spaces — must hard-break safely
        let lines = word_wrap("aáéíóúbáéíóúcáéíóúdáéíóú", 5);
        assert!(lines.len() >= 4, "should hard-break multiple times");
        for line in &lines {
            assert!(line.chars().count() <= 5, "each line must have at most max_chars characters");
        }
    }
}
