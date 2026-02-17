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

use std::error::Error as StdError;
use std::{fmt, io::Read};

use anyhow::Result;

use base64::engine::{Engine as _, general_purpose::STANDARD};
use bzip2::read::BzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crypt::config::CryptoConfig;

use shared::log;

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub message: String,
    pub is_retryable: bool, // old implementation used "0" and "1" strings
    pub percent: u8,        // 0...100
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (Retrayable?: {})", self.message, self.is_retryable)
    }
}

// for implicit conversion to anyhow::Error on ? operator
impl From<Error> for anyhow::Error {
    fn from(err: Error) -> Self {
        log::debug!("Converting Broker Error to anyhow::Error: {}", err);
        anyhow::anyhow!(err.message)
    }
}

impl From<&Error> for anyhow::Error {
    fn from(err: &Error) -> Self {
        log::debug!("Converting Broker Error to anyhow::Error: {}", err);
        anyhow::anyhow!(err.message.clone())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error {
            message: err.to_string(),
            is_retryable: false,
            percent: 0,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        // Defaults to the reqwest error string
        let mut error_text = "Error connecting to broker".to_string();

        // Check if the error or any of its sources is a certificate error
        let mut cur: &dyn StdError = &err;
        loop {
            let cur_str = cur.to_string();
            log::debug!("Checking error source: {}", cur_str);
            let msg = cur_str.to_lowercase();

            if msg.contains("ssl")
                || msg.contains("tls")
                || msg.contains("certificate")
                || msg.contains("verify")
                || msg.contains("x509")
                || msg.contains("handshake")
            {
                error_text = format!("TLS: {}", &cur_str);
                break;
            }

            if let Some(next) = cur.source() {
                cur = next;
            } else {
                break;
            }
        }

        Error {
            message: error_text,
            is_retryable: false,
            percent: 0,
        }
    }
}

impl Error {
    pub fn is_retryable(&self) -> bool {
        self.is_retryable
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrokerResponse<T> {
    pub result: Option<T>,
    pub error: Option<Error>,
}

impl<T> BrokerResponse<T> {
    pub fn into_result(self) -> Result<T, Error> {
        if let Some(err) = self.error {
            // This may be a retryable error, so this is normal
            Err(err)
        } else if let Some(res) = self.result {
            Ok(res)
        } else {
            Err(Error {
                message: "No result or error in BrokerResponse".to_string(),
                is_retryable: false,
                percent: 0,
            })
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Version {
    pub available_version: String,
    pub required_version: String,
    pub client_link: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    pub level: String,
    pub ticket: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum ScriptType {
    #[serde(rename = "javascript")]
    #[default]
    Javascript,
}

impl fmt::Display for ScriptType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptType::Javascript => write!(f, "javascript"),
        }
    }
}

// Any invalid valuw will return default (Javascript)
impl From<&str> for ScriptType {
    fn from(s: &str) -> Self {
        match s {
            "javascript" => ScriptType::Javascript,
            _ => ScriptType::Javascript,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TicketReqBody<'a> {
    pub scrambler: &'a str,
    pub kem_kyber_key: &'a str,
    pub hostname: &'a str,
    pub version: &'a str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Script {
    pub script: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub script_type: ScriptType,
    pub signature: String,           // base64-encoded signature
    pub signature_algorithm: String, // Optional signature algorithm
    pub params: String, // from codecs.encode(codecs.encode(json.dumps(self.parameters).encode(), 'bz2'), 'base64').decode()
    pub log: Log,
    pub crypto_params: Option<CryptoConfig>,  // provided by the broker for cryptographic operations
}

impl Script {
    fn decode_value(value: &str) -> Result<Vec<u8>> {
        let bz2_bytes: Vec<u8> = STANDARD.decode(value)?;

        let mut decoder = BzDecoder::new(&bz2_bytes[..]);
        let mut decoded_bytes = Vec::new();
        decoder.read_to_end(&mut decoded_bytes)?;

        Ok(decoded_bytes)
    }

    pub fn decoded_script(&self) -> Result<String> {
        let decoded_script_bytes = Script::decode_value(&self.script)?;
        Ok(String::from_utf8(decoded_script_bytes)?)
    }

    pub fn decoded_params(&self) -> Result<Value> {
        let json_bytes = Script::decode_value(&self.params)?;
        Ok(serde_json::from_slice(&json_bytes)?)
    }

    pub fn decoded_signature(&self) -> Result<Vec<u8>> {
        let sig_bytes = STANDARD.decode(&self.signature)?;
        Ok(sig_bytes)
    }

    pub fn verify_signature(&self) -> anyhow::Result<()> {
        let script = self.decoded_script()?;

        crypt::verify_signature(script.as_bytes(), &self.signature)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogUpload<'a> {
    // First word is LEVEL currently
    pub log: &'a str,
}

// Test helper to get a sample Script
#[cfg(test)]
pub fn get_test_script() -> Script {
    Script {
        script: concat!(
            "QlpoOTFBWSZTWQPlXDcAAC+fgFBpABgCAAQAL2fepCAAaiKe",
            "KP1JpmoNA9T1AiamxR6nqZAA0/VB8LwfsHGsWwV0lUmkhFZ6",
            "qJApvS7LtQTMnJ0icWQtCOfrScIsEoJ4I2racg8EbIdDFwUV",
            "v1NPj/F3JFOFCQA+VcNw"
        )
        .to_string(),
        script_type: ScriptType::Javascript,
        signature: concat!(
            "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"
        )
        .to_string(),
        signature_algorithm: "ML DSA 65".to_string(),
        params: concat!(
            "QlpoOTFBWSZTWYztF2MAACeZgFAEOBAyAl4KIABUMiAYjQEi",
            "UzSNG1NEKUUXJMbwNrm78OGNwikUjayaRUjcESFPQlVPnSfi",
            "7kinChIRnaLsYA=="
        )
        .to_string(),
        log: Log {
            level: "debug".to_string(),
            ticket: Some("dummy_ticket".to_string()),
        },
        crypto_params: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoded_script() -> Result<()> {
        let script = get_test_script();
        let decoded_script = script.decoded_script()?;
        assert!(decoded_script.contains(r#"throw new Error("The selected transport is not supported on your platform: " + (data.transport || "" ) );"#));
        Ok(())
    }

    // Test params: params = {'param1': 'test', 'param2': 1, 'param3': { 'subparam1': 'subtest', 'subparam2': 2}}
    #[test]
    fn test_decoded_params() -> Result<()> {
        let script = get_test_script();
        let decoded_params = script.decoded_params()?;
        let decoded_params = decoded_params.as_object().unwrap();
        assert_eq!(
            decoded_params.get("param1").unwrap().as_str().unwrap(),
            "test"
        );
        assert_eq!(decoded_params.get("param2").unwrap().as_i64().unwrap(), 1);
        let subparams = decoded_params.get("param3").unwrap().as_object().unwrap();
        assert_eq!(
            subparams.get("subparam1").unwrap().as_str().unwrap(),
            "subtest"
        );
        assert_eq!(subparams.get("subparam2").unwrap().as_i64().unwrap(), 2);
        Ok(())
    }

    #[test]
    fn test_verify_signature() -> Result<()> {
        let script = get_test_script();
        script.verify_signature()?;
        Ok(())
    }

    #[test]
    fn test_invalid_signature() {
        let mut script = get_test_script();
        // Corrupt the signature
        script.signature.replace_range(32..43, "A");
        let result = script.verify_signature();
        assert!(result.is_err(), "Signature verification should have failed");
    }
}
