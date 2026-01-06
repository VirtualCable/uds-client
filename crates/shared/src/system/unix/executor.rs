// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
                    if stop.wait_timeout(Duration::from_millis(300)) {
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
