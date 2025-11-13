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
use rsa::{RsaPublicKey, pkcs1::DecodeRsaPublicKey, pkcs1v15::Pkcs1v15Sign};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::consts;

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub error: String,
    pub is_retryable: String, // "0" or "1", as string
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrokerResponse<T> {
    pub result: T,
    pub error: Option<Error>,
}

impl<T> BrokerResponse<T> {
    pub fn into_result(self) -> Result<T> {
        if let Some(err) = self.error {
            Err(anyhow::anyhow!("Broker error: {}", err.error))
        } else {
            Ok(self.result)
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
    pub ticket: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Script {
    pub script: String,
    #[serde(rename = "type")]
    pub script_type: String,
    pub signature: String, // base64-encoded signature
    pub params: String, // from codecs.encode(codecs.encode(json.dumps(self.parameters).encode(), 'bz2'), 'base64').decode()
    pub log: Log,
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
        let public_key = RsaPublicKey::from_pkcs1_pem(consts::PUBLIC_KEY_PEM)?;

        let script = self.decoded_script()?;
        let signature = self.decoded_signature()?;

        let digest = Sha256::digest(script.as_bytes());

        public_key
            .verify(Pkcs1v15Sign::new::<Sha256>(), &digest, &signature)
            .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
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
        script_type: "javascript".to_string(),
        signature: concat!(
            "dMGa9458H8gpnAD+E+c92NgvLSh7INJzmoNxoaXW4BWtptc5",
            "MeV6sO5nVjc54tEj8M8myLnKdve8jxqeAEpZ9QC/WDE/V7+U",
            "ePgS5ASV8dWroOdI/TqiIKJbMefeBnXVwI+1dNzVYLE++AMv",
            "lMbS4qA6vS8Efi2k8yN4ov64jOLO+aLCXJiDYKbgQgEBCDgF",
            "f7jD9Xog+xCxV0zdUzNTj5QeBYs5aXTIaq+DPGME2LivuL3s",
            "rd0vWv+fPztc01zM+z7PSDt+ZCorfPd84P1mzsIq9vDDgvtn",
            "H75XrhHb3LAc/+XLQMK2lSzUJOYb3Bf4KGIurjJQujBCggT+",
            "YauXZGkFTCc+JhKewEa85r+sAnvIaRqsRrJ2Rkt4rrQO8E79",
            "valbQyvWKbcjH8emhpervUo0mQn4qknKTxMxOjDpL0la33G7",
            "7DfAMlbUy8fHQyQ0pAO2lPRmJgQQTpOhVggyV/SehqXFI78P",
            "cuuWxBSA2vZbr8GKyYdGtCJssg/R5vbScG/MlMAGd3/mWZoS",
            "czyqxFFt1Jdu5KElTd9ihuEHc27MBgvjUFDtyjTACrPsDwge",
            "hR4YUdrJHlca/D+9/sgLApg0an7aO8OQmAL2Dxs1dlcZzhAR",
            "QSRAWMaLiqJPo7lP5iXq+Ua/PZW2DczGGgOW1X01dNEGtY8C",
            "daOyozDY6EU="
        )
        .to_string(),
        params: concat!(
            "QlpoOTFBWSZTWYztF2MAACeZgFAEOBAyAl4KIABUMiAYjQEi",
            "UzSNG1NEKUUXJMbwNrm78OGNwikUjayaRUjcESFPQlVPnSfi",
            "7kinChIRnaLsYA=="
        )
        .to_string(),
        log: Log {
            level: "debug".to_string(),
            ticket: "dummy_ticket".to_string(),
        },
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
}
