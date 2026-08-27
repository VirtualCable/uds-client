// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use anyhow::Result;
use flume::Sender;
use tokio::sync::oneshot;

use connection::{
    broker::api::{self, types},
    consts, tasks,
};
use gui::types::GuiMessage;
use shared::{appdata, log, tls::pinned};

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

async fn approve_certificate(
    tx: &Sender<GuiMessage>,
    host: &str,
    appdata: &mut appdata::AppData,
    error: types::Error,
) -> Result<()> {
    let Some(fingerprint) = pinned::last_rejected() else {
        return Err(error.into());
    };

    log::debug!("Asking user to approve certificate {}.", fingerprint);

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(GuiMessage::ShowYesNo(
        tr!(
            "The certificate of {}\ncannot be verified.\n\nSHA-256 fingerprint:\n{}\n\nOnly trust certificates you have checked.\nDo you want to continue?",
            host,
            split_fingerprint(&fingerprint),
        ),
        Arc::new(RwLock::new(Some(reply_tx))),
    ))
    .ok();
    let answer = reply_rx.await.unwrap_or(false);
    if !answer {
        log::info!("Certificate {} not approved by user.", fingerprint);
        return Err(error.into());
    }

    pinned::trust(&fingerprint);
    appdata.trusted_certs.push(fingerprint);
    appdata.save();

    Ok(())
}

fn split_fingerprint(fingerprint: &str) -> String {
    fingerprint
        .split(':')
        .collect::<Vec<&str>>()
        .chunks(16)
        .map(|chunk| chunk.join(":"))
        .collect::<Vec<String>>()
        .join("\n")
}

pub async fn run(
    tx: Sender<GuiMessage>,
    stop: shared::system::trigger::Trigger,
    host: &str,
    ticket: &str,
    scrambler: &str,
) -> Result<()> {
    let mut appdata = appdata::AppData::load();

    pinned::set_trusted(appdata.trusted_certs.clone());

    let api = api::new_api(
        host,
        None,
        appdata.verify_ssl.unwrap_or(true),
        appdata.disable_proxy.unwrap_or(false),
    );

    // Start with 0% progress
    tx.send(GuiMessage::Progress(
        0,
        tr!("Starting connection...").to_string(),
    ))
    .ok();

    // Approve host if needed
    approve_host(&tx, host, &mut appdata).await?;

    // Get version info
    let version = match api.get_version_info().await {
        Ok(version) => version,
        Err(e) if e.is_tls() => {
            approve_certificate(&tx, host, &mut appdata, e).await?;
            api.get_version_info().await?
        }
        Err(e) => return Err(e.into()),
    };

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
        log::debug!("Attempting to get script from broker.");
        match api.get_script(ticket, scrambler).await {
            Ok(script) => {
                // Check signature
                if script.verify_signature().is_err() {
                    anyhow::bail!(tr!("Script signature verification failed."));
                }
                js::run_script(&script).await?;
                break;
            }
            Err(e) => {
                log::debug!("Error getting script from broker: {:?}", e);
                // Here we can only get an access denied error or a retryable error
                // because tls errors and other network errors must have been
                // raised before
                if !e.is_retryable() {
                    anyhow::bail!(tr!("Access denied by broker.\n{}", e.message));
                } else {
                    // Send percent to GUI
                    tx.send(GuiMessage::Progress(
                        e.percent,
                        tr!("Preparing connection...").to_string(),
                    ))
                    .ok();
                }
            }
        }
        // Retry after some time
        if stop
            .wait_timeout_async(std::time::Duration::from_secs(8))
            .await
            .is_ok()
        {
            log::info!("Stopping runner.");
            return Ok(());
        }
    }

    log::debug!("Script obtained and executed successfully.");
    // All done, send hide message if NOT internal RDP is running
    if tasks::is_internal_rdp_running() {
        log::debug!("Internal RDP is running.");
    } else {
        log::debug!("No internal RDP is running. Closing Progress window.");
        tx.send(GuiMessage::CloseProgress).ok();
    }

    // Execute the tasks in background, and wait with cleanup
    tasks::wait_all_and_cleanup(std::time::Duration::from_secs(4), stop).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_split_in_two_lines() {
        let fingerprint = (0..32)
            .map(|i| format!("{:02X}", i))
            .collect::<Vec<String>>()
            .join(":");

        let split = split_fingerprint(&fingerprint);
        let lines: Vec<&str> = split.split('\n').collect();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].matches(':').count(), 15);
        assert_eq!(lines[1].matches(':').count(), 15);
        assert_eq!(lines.join(":"), fingerprint);
    }

    #[test]
    fn short_fingerprint_stays_in_one_line() {
        assert_eq!(split_fingerprint("AA:BB:CC"), "AA:BB:CC");
    }
}
