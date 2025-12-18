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
use std::sync::{Arc, RwLock};

use anyhow::Result;
use crossbeam::channel::Sender;
use tokio::sync::oneshot;

use gui::window::types::GuiMessage;
use shared::{appdata, broker::api, consts, log, tasks};

async fn approve_host(
    tx: &Sender<GuiMessage>,
    host: &str,
    appdata: &mut appdata::AppData,
) -> Result<()> {
    let host_lower = host.to_lowercase();
    if appdata
        .approved_hosts
        .iter()
        .any(|h| h.to_lowercase() == host_lower)
    {
        log::info!("Host {} is already approved.", host);
        return Ok(());
    }

    log::debug!("Approving host {} with broker.", host);

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(GuiMessage::ShowYesNo(
        tr!("The server {}\nmust be approved.\nOnly approve UDS servers you trust.\nDo you want to continue?", host),
        Arc::new(RwLock::new(Some(reply_tx))),
    ))
    .ok();
    let answer = reply_rx.await.unwrap_or(false);
    if !answer {
        log::info!("Host {} not approved by user.", host);
        anyhow::bail!(tr!("Host {} not approved by user.", host));
    }
    appdata.approved_hosts.push(host.to_string());
    appdata.save();

    Ok(())
}

pub async fn run(
    tx: Sender<GuiMessage>,
    stop: shared::system::trigger::Trigger,
    host: &str,
    ticket: &str,
    scrambler: &str,
) -> Result<()> {
    let mut appdata = appdata::AppData::load();

    let api = api::new_api(
        host,
        None,
        appdata.verify_ssl.unwrap_or(true),
        appdata.disable_proxy.unwrap_or(false),
    );

    // Start with 0% progress
    tx.send(GuiMessage::Progress(
        0.0,
        tr!("Starting connection...").to_string(),
    ))
    .ok();

    // Approve host if needed
    approve_host(&tx, host, &mut appdata).await?;

    // Get version info
    let version = api.get_version_info().await?;

    log::info!("Broker version: {:?}", version);
    // There is a lot of time (10 years maybe? :P) before we reach version 10, so just a simple check

    // Note: Versions prior to 5.0.0. uses a different scheme, (udss:// instead of udssv2://),
    // so we don't need to check for older versions here.
    if version.required_version.as_str() <= consts::UDS_CLIENT_VERSION {
        log::info!("Client version is up to date.");
    } else {
        log::warn!(
            "Client version {} is outdated. Required version is {}.",
            consts::UDS_CLIENT_VERSION,
            version.required_version
        );
        anyhow::bail!(tr!(
            "Client version {} is outdated. Required version is {}.\nPlease download the latest version from\n{}\nand try again.",
            consts::UDS_CLIENT_VERSION,
            version.required_version,
            version.client_link
        ));
    }

    // If thereis a newer version,
    if version.available_version.as_str() > consts::UDS_CLIENT_VERSION {
        log::warn!(
            "A newer client version {} is available. Current version is {}.",
            version.available_version,
            consts::UDS_CLIENT_VERSION
        );
        tx.send(GuiMessage::ShowWarning(tr!(
            "A newer client version {} is available. Current version is {}.\n{}|Download the latest version",
            version.available_version,
            consts::UDS_CLIENT_VERSION,
            version.client_link
        )))
        .ok();
    }

    loop {
        match api.get_script(ticket, scrambler).await {
            Ok(script) => {
                // Check signature
                if script.verify_signature().is_err() {
                    anyhow::bail!(tr!("Script signature verification failed."));
                }
                tx.send(GuiMessage::Hide).ok();
                js::run_script(&script).await?;
                break;
            }
            Err(e) => {
                // Here we can only get an access denied error or a retryable error
                // because tls errors and other network errors must have been
                // raised before
                if !e.is_retryable() {
                    anyhow::bail!(tr!("Access denied by broker.\n{}", e.message));
                } else {
                    // Send percent to GUI
                    tx.send(GuiMessage::Progress(
                        e.percent as f32 / 100.0,
                        tr!("Preparing connection...").to_string(),
                    ))
                    .ok();
                }
            }
        }
        // Retry after some time, trigger returns true if triggered
        if stop
            .async_wait_timeout(std::time::Duration::from_secs(8))
            .await
        {
            log::info!("Stopping runner.");
            return Ok(());
        }
    }

    // All done, send hide message if NOT internal RDP is running
    if shared::tasks::is_internal_rdp_running() {
        log::debug!("Internal RDP is running.");
    } else {
        log::debug!("Hiding GUI.");
        tx.send(GuiMessage::Hide).ok();
    }

    // Execute the tasks in background, and wait with cleanup
    tasks::wait_all_and_cleanup(std::time::Duration::from_secs(4), stop).await;

    Ok(())
}
