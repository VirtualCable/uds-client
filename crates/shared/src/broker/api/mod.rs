// Copyright (c) 2025 Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:
//
//    * Redistributions of source code must retain the above copyright notice,
//      this list of conditions and the following disclaimer.
//    * Redistributions in binary form must reproduce the above copyright notice,
//      this list of conditions and the following disclaimer in the documentation
//      and/or other materials provided with the distribution.
//    * Neither the name of Virtual Cable S.L.U. nor the names of its contributors
//      may be used to endorse or promote products derived from this software
//      without specific prior written permission.
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
/*!
Author: Adolfo Gómez, dkmaster at dkmon dot com
*/
use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, ClientBuilder};

use crate::{consts, log};

pub mod types;

#[async_trait]
pub trait BrokerApi: Send + Sync {
    async fn get_version_info(&self) -> Result<types::Version>;
    async fn get_script(&self, ticket: &str, scrambler: &str) -> Result<types::Script>;
    async fn send_log(&self, log_str: String) -> Result<()>;
}

pub struct UdsBrokerApi {
    client: Client,
    broker_url: String,
    hostname: String,
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
            .timeout(timeout.unwrap_or(std::time::Duration::from_secs(2)))
            .connection_verbose(cfg!(debug_assertions))
            .danger_accept_invalid_certs(!verify_ssl);

        if skip_proxy {
            builder = builder.no_proxy();
        }

        // panic if client cannot be built, as this is a programming error (invalid URL, etc)
        let client = builder.build().unwrap();

        Self {
            client,
            broker_url: broker_url.to_string().trim_end_matches('/').to_string(),
            hostname: hostname::get().unwrap().to_string_lossy().to_string(),
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
    async fn get_version_info(&self) -> Result<types::Version> {
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

    async fn get_script(&self, ticket: &str, scrambler: &str) -> Result<types::Script> {
        log::debug!(
            "Getting script from broker at {} with ticket {} and scrambler {}",
            self.broker_url,
            ticket,
            scrambler
        );
        let response = self
            .client
            .get(format!(
                "{}/{}/{}?hostname={}&version={}",
                self.broker_url,
                ticket,
                scrambler,
                self.hostname,
                consts::UDS_CLIENT_VERSION
            ))
            .headers(self.headers())
            .send()
            .await?;

        response
            .json::<types::BrokerResponse<types::Script>>()
            .await?
            .into_result()
    }

    async fn send_log(&self, log_str: String) -> Result<()> {
        log::debug!("Sending log to broker at {}", self.broker_url);
        let log_data = types::LogUpload {
            log: &log_str,
        };
        self
            .client
            .put(format!("{}/log", self.broker_url))
            .headers(self.headers())
            .json(&log_data)
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;