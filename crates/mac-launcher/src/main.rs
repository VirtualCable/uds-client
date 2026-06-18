// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#[cfg(target_os = "macos")]
mod launcher;

fn main() {
    #[cfg(target_os = "macos")]
    launcher::launch();
}
