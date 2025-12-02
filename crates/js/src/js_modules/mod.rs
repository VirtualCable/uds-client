use anyhow::Result;
use boa_engine::{Context};

// Js modules
mod file;
mod logger;
mod process;
mod tasks;
mod utils;
mod rdp;

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    utils::register(ctx)?;
    logger::register(ctx)?;
    process::register(ctx)?;
    tasks::register(ctx)?;
    file::register(ctx)?;
    rdp::register(ctx)?;
    Ok(())
}