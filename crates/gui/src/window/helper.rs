// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use eframe::egui;

use shared::{log, utils::split_lines};

const LINE_HEIGHT: f32 = 18.0;

pub(super) fn display_multiline_text(ui: &mut egui::Ui, text: &str, hover_text: String) {
    ui.add_space(18.0);
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

pub(super) fn calculate_text_height(text: &str, max_width: usize) -> f32 {
    let lines = split_lines(text, max_width);
    lines.len() as f32 * LINE_HEIGHT
}