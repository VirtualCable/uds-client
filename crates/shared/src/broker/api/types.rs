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
use std::io::Read;

use anyhow::Result;

use base64::engine::{Engine as _, general_purpose::STANDARD};
use bzip2::read::BzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Error {
    pub error: String,
    pub is_retryable: String, // "0" or "1", as string
}

#[derive(Debug, Deserialize)]
pub struct BrokerResponse<T> {
    pub result: T,
    pub error: Option<Error>,
}

impl <T> BrokerResponse<T> {
    pub fn into_result(self) -> Result<T> {
        if let Some(err) = self.error {
            Err(anyhow::anyhow!("Broker error: {}", err.error))
        } else {
            Ok(self.result)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Version {
    pub available_version: String,
    pub required_version: String,
    pub client_link: String,
}

#[derive(Debug, Deserialize)]
pub struct Log {
    pub level: String,
    pub ticket: String,
}

#[derive(Debug, Deserialize)]
pub struct Script {
    pub script: String,
    #[serde(rename = "type")]
    pub mime_type: String,
    pub signature: String,  // base64-encoded signature
    pub params: String,     // from codecs.encode(codecs.encode(json.dumps(self.parameters).encode(), 'bz2'), 'base64').decode()
    pub log: Log,
}

impl Script {
    fn decode_value(value: &str) -> Result<Vec<u8>> {
        let bz2_bytes = STANDARD.decode(value)?;

        let mut decoder = BzDecoder::new(&bz2_bytes[..]);
        let mut json_bytes = Vec::new();
        decoder.read_to_end(&mut json_bytes)?;

        Ok(json_bytes)
    }

    pub fn decoded_script(&self) -> Result<String> {
        let json_bytes = Script::decode_value(&self.script)?;
        Ok(String::from_utf8(json_bytes)?)
    }

    pub fn decoded_params(&self) -> Result<Value> {
        let json_bytes = Script::decode_value(&self.params)?;
        Ok(serde_json::from_slice(&json_bytes)?)
    }

    pub fn decoded_signature(&self) -> anyhow::Result<Vec<u8>> {
        let sig_bytes = STANDARD.decode(&self.signature)?;
        Ok(sig_bytes)
    }
}

#[derive(Debug, Serialize)]
pub struct LogUpload<'a> {
    // First word is LEVEL currently
    pub log: &'a str,
}
