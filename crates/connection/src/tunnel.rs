// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::{Context, Result};

use shared::log;

use crate::types::TunnelConnectInfo;

use crate::{v4, v5};

pub async fn start_tunnel(info: TunnelConnectInfo) -> Result<u16> {
    log::debug!("Sending initial test connection to tunnel server");
    // Check v5 tunnel first, if fails, fallback to v4
    if let Err(e) = v5::check_tunnel(&info).await {
        log::debug!("v5 tunnel check failed: {}, falling back to v4", e);
        v4::check_tunnel(&info)
            .await
            .context("v4 tunnel check failed")?;
        v4::start_tunnel(info).await
    } else {
        log::debug!("v5 tunnel check successful");
        v5::start_tunnel(info).await
    }
}
