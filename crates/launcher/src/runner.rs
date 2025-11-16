use std::sync::mpsc;

use crate::{appdata, gui::progress::GuiMessage};
use anyhow::Result;

use shared::{broker::api, consts, log};

async fn approve_host(
    tx: &mpsc::Sender<GuiMessage>,
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

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    tx.send(GuiMessage::YesNo(
        format!("The server {}\nmust be approved.\nOnly approve UDS servers you trust.\nDo you want to continue?", host),
        Some(reply_tx),
    ))
    .ok();
    let answer = reply_rx.await.unwrap_or(false);
    log::info!("User answer: {}", answer);
    if !answer {
        anyhow::bail!("Host {} not approved by user.", host);
    }
    appdata.approved_hosts.push(host.to_string());
    appdata.save();

    Ok(())
}

pub async fn run(
    tx: mpsc::Sender<GuiMessage>,
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
        anyhow::bail!(
            "Client version {} is outdated. Required version is {}.\nPlease download the latest version from\n{}\nand try again.",
            consts::UDS_CLIENT_VERSION,
            version.required_version,
            version.client_link
        );
    }

    // If thereis a newer version,
    if version.available_version.as_str() > consts::UDS_CLIENT_VERSION {
        log::warn!(
            "A newer client version {} is available. Current version is {}.",
            version.available_version,
            consts::UDS_CLIENT_VERSION
        );
        tx.send(GuiMessage::Warning(format!(
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
                log::info!("Received script: {:?}", script);
                // Run the script
                break;
            }
            Err(e) => {
                // Here we can only get an access denied error or a retryable error
                // because tls errors and other network errors must have been
                // raised before
                if !e.is_retryable() {
                    anyhow::bail!("{}\n{}", "Access denied by broker.", e.message);
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

    // Start with 0% progress
    tx.send(GuiMessage::Progress(0.0)).ok();
    Ok(())
}
