pub fn split_lines(text: &str, max_width: usize) -> Vec<&str> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let mut start = 0;
        let line_len = line.len();
        while start < line_len {
            let end = usize::min(start + max_width, line_len);
            lines.push(&line[start..end]);
            start += max_width;
        }
    }
    lines
}
