// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;
use boa_engine::Context;

// Js modules
mod file;
mod logger;
mod process;
mod rdp;
mod tasks;
mod utils;

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    utils::register(ctx)?;
    logger::register(ctx)?;
    process::register(ctx)?;
    tasks::register(ctx)?;
    file::register(ctx)?;
    rdp::register(ctx)?;
    Ok(())
}
