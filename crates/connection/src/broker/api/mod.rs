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
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, ClientBuilder};

use crypt::{
    consts::{PRIVATE_KEY_SIZE, PUBLIC_KEY_SIZE},
    kem::generate_key_pair,
};
use shared::log;

use crate::consts;

use super::ticket::BrokerTicket;

pub mod types;

#[async_trait]
pub trait BrokerApi: Send + Sync {
    async fn get_version_info(&self) -> Result<types::Version, types::Error>;
    async fn get_script(
        &self,
        ticket: &str,
        scrambler: &str,
    ) -> Result<types::Script, types::Error>;
    async fn send_log(&self, log_str: String) -> Result<()>;
}

pub struct UdsBrokerApi {
    client: Client,
    broker_url: String,
    hostname: String,
    public_key: [u8; PUBLIC_KEY_SIZE],
    private_key: [u8; PRIVATE_KEY_SIZE],
}

impl UdsBrokerApi {
    pub fn new(
        broker_url: &str,
        timeout: Option<std::time::Duration>,
        verify_ssl: bool,
        skip_proxy: bool,
    ) -> Self {
        log::debug!("Creating UDSBrokerApi for URL: {}", broker_url);
        let mut builder = ClientBuilder::new()
            .use_rustls_tls() // Use rustls for TLS
            .timeout(timeout.unwrap_or(std::time::Duration::from_secs(32))) // Long enough timeout
            .connection_verbose(cfg!(debug_assertions))
            .danger_accept_invalid_certs(!verify_ssl);

        if skip_proxy {
            builder = builder.no_proxy();
        }

        // Note: unwraps are intentinonal here, if we cannot build the client, we want to
        // abort early.

        let client = builder.build().unwrap();

        // Generate ephemeral KEM keypair
        let (private_key, public_key) = generate_key_pair().unwrap();

        Self {
            client,
            broker_url: broker_url.to_string().trim_end_matches('/').to_string(),
            hostname: hostname::get().unwrap().to_string_lossy().to_string(),
            public_key: public_key.try_into().unwrap(),
            private_key: private_key.try_into().unwrap(),
        }
    }

    // Only for tests
    #[cfg(test)]
    pub fn with_keys(
        self,
        private_key: [u8; PRIVATE_KEY_SIZE],
        public_key: [u8; PUBLIC_KEY_SIZE],
    ) -> Self {
        Self {
            public_key,
            private_key,
            ..self
        }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(consts::UDS_CLIENT_AGENT).unwrap(),
        );
        // Add custom headers
        headers
    }
}

#[async_trait]
impl BrokerApi for UdsBrokerApi {
    async fn get_version_info(&self) -> Result<types::Version, types::Error> {
        log::debug!("Getting version info from broker at {}", self.broker_url);
        let response = self
            .client
            .get(self.broker_url.as_str())
            .headers(self.headers())
            .send()
            .await?;

        response
            .json::<types::BrokerResponse<types::Version>>()
            .await?
            .into_result()
    }

    async fn get_script(
        &self,
        ticket: &str,
        scrambler: &str,
    ) -> Result<types::Script, types::Error> {
        log::debug!(
            "Getting script from broker at {} with ticket {} and scrambler {}",
            self.broker_url,
            ticket,
            scrambler
        );

        // Prepare request body
        let req: types::TicketReqBody = types::TicketReqBody {
            scrambler,
            kem_kyber_key: &general_purpose::STANDARD.encode(self.public_key),
            hostname: &self.hostname,
            version: consts::UDS_CLIENT_VERSION,
        };

        let response = self
            .client
            .post(format!("{}/{}/ticket", self.broker_url, ticket,))
            .json(&req)
            .headers(self.headers())
            .send()
            .await?;

        // Extract real script info from Ticket
        response
            .json::<types::BrokerResponse<BrokerTicket>>()
            .await?
            .into_result()?
            .recover_data_from_json(ticket, &self.private_key)
            .map(|json_value| {
                serde_json::from_value::<types::Script>(json_value).map_err(|e| types::Error {
                    message: format!("Failed to parse script from ticket data: {}", e),
                    is_retryable: false,
                    percent: 0,
                })
            })?
    }

    async fn send_log(&self, log_str: String) -> Result<()> {
        log::debug!("Sending log to broker at {}", self.broker_url);
        let log_data = types::LogUpload { log: &log_str };
        self.client
            .put(format!("{}/log", self.broker_url))
            .headers(self.headers())
            .json(&log_data)
            .send()
            .await?;
        Ok(())
    }
}

pub fn new_api(
    host: &str,
    timeout: Option<std::time::Duration>,
    verify_ssl: bool,
    skip_proxy: bool,
) -> std::sync::Arc<dyn BrokerApi> {
    std::sync::Arc::new(UdsBrokerApi::new(
        &consts::URL_TEMPLATE.replace("{host}", host),
        timeout,
        verify_ssl,
        skip_proxy,
    ))
}

#[cfg(test)]
mod tests;
