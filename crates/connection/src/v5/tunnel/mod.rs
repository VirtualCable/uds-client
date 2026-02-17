// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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

// * Server side on client will
//   - Accept connections from external clients on localhost:port
//   - Will keep listening on port until:
//       - Some initial timeout expires without any connections
//       - A stop trigger is fired
//       - Last connection is closed, but only if not initial timeout has expired yet
// * Server side will have an channel id associated
// * Relay data from connected clients to "proxy" (the middle element that will allow some remote errors resiliency)
// * Receive data from "proxy" and send it to the connected client
// * Note: Although server supports multi streams, currently they are of no use, as we only open the stream
//         ONCE the local connection is established. So we only have one stream per server instance.
//         For custom protocols, the remote support may be useful to open multiple streams over the same
//         tunnel connection.

use std::time::Duration;

use anyhow::Result;

use shared::{log, system::trigger::Trigger};

use super::{protocol::ticket::Ticket, proxy::Proxy, crypt::types::SharedSecret};

pub struct Tunnel {
    ticket: Ticket,
    shared_secret: SharedSecret,
    initial_timeout: Duration,
    tunnel_server: String, // Host:port of tunnel server to connect to
    stop: Trigger,
}

impl Tunnel {
    pub fn new(
        ticket: Ticket,
        shared_secret: SharedSecret,
        initial_timeout: Duration,
        tunnel_server: String,
        stop: Trigger,
    ) -> Self {
        Self {
            ticket,
            shared_secret,
            initial_timeout,
            tunnel_server,
            stop,
        }
    }

    pub async fn run(self) -> Result<()> {
        log::info!("Starting tunnel");
        // Create the proxy and run it
        let proxy = Proxy::new(
            &self.tunnel_server,
            self.ticket,
            self.shared_secret,
            self.initial_timeout,
            self.stop.clone(),
        );

        // If fails to connect, wil return error
        proxy.run().await?;
        Ok(())
    }
}

// pub async fn create_listener(
//     local_port: Option<u16>,
//     enable_ipv6: bool,
// ) -> Result<TcpListener> {
//     let addr = format!(
//         "{}:{}",
//         if enable_ipv6 {
//             consts::LISTEN_ADDRESS_V6
//         } else {
//             consts::LISTEN_ADDRESS
//         },
//         local_port.unwrap_or(0)
//     );
//     let listener = tokio::net::TcpListener::bind(&addr)
//         .await
//         .context("Failed to create TCP listener")?;

//     log::debug!("TCP listener created on {}", addr);

//     Ok(listener)
// }

// Tests module
#[cfg(test)]
mod tests;
