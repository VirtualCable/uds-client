use std::rc::Rc;
use std::sync::mpsc;

use crate::gui::progress::GuiMessage;
use anyhow::Result;

use shared::{broker::api, consts, log};

pub async fn run(
    tx: mpsc::Sender<GuiMessage>,
    _stop: shared::system::trigger::Trigger,
    host: &str,
    _ticket: &str,
    _scrambler: &str,
) -> Result<()> {
    let api: Rc<dyn api::BrokerApi> = api::new_api(host);

    // Get version info
    let version = api.get_version_info().await?;

    log::info!("Broker version: {:?}", version);
    // There is a lot of time (10 years maybe? :P) before we reach version 10, so just a simple check
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
            "A newer client version {} is available. Current version is {}.\nPlease consider updating from\n{}",
            version.available_version,
            consts::UDS_CLIENT_VERSION,
            version.client_link
        )))
        .ok();
    }

    tx.send(GuiMessage::Progress(0.0)).ok();
    Ok(())
}
