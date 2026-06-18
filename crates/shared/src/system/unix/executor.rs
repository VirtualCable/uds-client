// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;
use std::process::{Command, Stdio};
use std::time::Duration;

use super::super::trigger;
use crate::log;

pub fn execute_app(
    application: &str,
    parameters: &[&str],
    stop: Option<trigger::Trigger>,
    cwd: Option<&str>,
) -> Result<()> {
    let mut cmd = Command::new(application);
    cmd.args(parameters)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn {}: {}", application, e))?;

    // If a stop trigger is provided, monitor it
    if let Some(stop) = stop {
        loop {
            // Has the process finished?
            match child.try_wait()? {
                Some(status) => {
                    log::info!("Process exited with status: {}", status);
                    break;
                }
                None => {
                    // Has the stop trigger been activated?
                    if stop.wait_timeout(Duration::from_millis(300)).is_ok() {
                        log::info!("Stop trigger activated, killing process");
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
    } else {
        // No stop trigger, just wait for the process to finish
        let status = child.wait()?;
        log::info!("Process exited with status: {}", status);
    }

    Ok(())
}
