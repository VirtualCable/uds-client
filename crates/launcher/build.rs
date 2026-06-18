// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

fn main() {
    #[cfg(windows)]
    builder::build_windows(builder::BuildInfo {
        product_name: "UDS Launcher",
        description: "UDS Launcher Application",
        icon: None,
        bmp: None,
    });
}
