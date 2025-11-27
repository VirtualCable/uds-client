use eframe::egui;

use shared::{log, utils::split_lines};

pub(super) fn display_multiline_text(ui: &mut egui::Ui, text: &str, hover_text: String) {
    for line in split_lines(text, 40) {
        if line.starts_with("http") {
            // get label after |
            let (label, link) = if let Some(pos) = line.find('|') {
                (&line[pos + 1..], &line[..pos])
            } else {
                (line, line)
            };
            if ui
                .hyperlink_to(label, link)
                .on_hover_text(&hover_text)
                .clicked()
            {
                if let Err(e) = open::that(line) {
                    log::error!("Failed to open link {}: {}", line, e);
                }
            } else {
                // Because clippy wants to collapse this block
                // and then the meaning is lost
                // because we WANT to execute hyperling_to even if not clicked
                // and not show the label if not clicked.. stupid clippy :)
            }
        } else {
            ui.label(line);
        }
    }
}

pub(super) fn calculate_text_height(text: &str, max_width: usize, line_height: f32) -> f32 {
    let lines = split_lines(text, max_width);
    lines.len() as f32 * line_height
}