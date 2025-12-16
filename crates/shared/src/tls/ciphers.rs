// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use rustls::{
    SupportedCipherSuite,
    crypto::{CryptoProvider, aws_lc_rs},
};

use crate::log;

pub const SECURE_CIPHERS: &str = concat!(
    "TLS_AES_256_GCM_SHA384:",
    "TLS_AES_128_GCM_SHA256:",
    "TLS_CHACHA20_POLY1305_SHA256:",
    "ECDHE-RSA-AES256-GCM-SHA384:",
    "ECDHE-RSA-AES128-GCM-SHA256:",
    "ECDHE-RSA-CHACHA20-POLY1305:",
    "ECDHE-ECDSA-AES128-GCM-SHA256:",
    "ECDHE-ECDSA-AES256-GCM-SHA384:",
    "ECDHE-ECDSA-CHACHA20-POLY1305",
);

fn openssl_to_rustls_cipher_name(cipher: &str) -> Option<SupportedCipherSuite> {
    let rust_cipher_name = match cipher.to_uppercase().as_str() {
        // TLS 1.3 Suites
        "TLS_AES_256_GCM_SHA384" => "TLS13_AES_256_GCM_SHA384",
        "TLS_AES_128_GCM_SHA256" => "TLS13_AES_128_GCM_SHA256",
        "TLS_CHACHA20_POLY1305_SHA256" => "TLS13_CHACHA20_POLY1305_SHA256",

        // TLS 1.2 Suites
        "ECDHE-ECDSA-AES256-GCM-SHA384" => "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
        "ECDHE-ECDSA-AES128-GCM-SHA256" => "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
        "ECDHE-ECDSA-CHACHA20-POLY1305" => "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
        "ECDHE-RSA-AES256-GCM-SHA384" => "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
        "ECDHE-RSA-AES128-GCM-SHA256" => "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
        "ECDHE-RSA-CHACHA20-POLY1305" => "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",

        // Some times listed with -SHA256 suffix for CHACHA20-POLY1305
        "ECDHE-ECDSA-CHACHA20-POLY1305-SHA256" => "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
        "ECDHE-RSA-CHACHA20-POLY1305-SHA256" => "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
        // Not found
        _ => return None,
    };
    // Only return the rustls cipher name if it is in the list of rustls::crypto::aws_lc_rs::ALL_CIPHER_SUITES

    for suite in rustls::crypto::aws_lc_rs::ALL_CIPHER_SUITES.iter() {
        if suite.suite().as_str().unwrap() == rust_cipher_name {
            return Some(*suite);
        }
    }

    None
}

fn filter_cipher_suites(ciphers: &str) -> Vec<SupportedCipherSuite> {
    ciphers
        .split(':')
        .collect::<Vec<&str>>()
        .iter()
        .filter_map(|cipher| openssl_to_rustls_cipher_name(cipher))
        .collect()
}

pub fn provider(ciphers: Option<&str>) -> CryptoProvider {
    let ciphers = if let Some(ciphers) = ciphers
        && !ciphers.is_empty()
    {
        filter_cipher_suites(ciphers)
    } else {
        filter_cipher_suites(SECURE_CIPHERS)
        //rustls::crypto::aws_lc_rs::DEFAULT_CIPHER_SUITES.to_vec()
    };

    // If empty, fall back to default
    let ciphers = if ciphers.is_empty() {
        log::warn!("No valid ciphers found in provided list, falling back to default ciphers");
        aws_lc_rs::DEFAULT_CIPHER_SUITES.to_vec()
    } else {
        ciphers
    };

    log::debug!("valid cipher_suites: {:?}", ciphers);

    rustls::crypto::CryptoProvider {
        cipher_suites: ciphers,
        ..aws_lc_rs::default_provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_empty_cipher_list() {
        let ciphers = "";
        let provider = provider(Some(ciphers));
        assert_eq!(
            provider.cipher_suites.len(),
            rustls::crypto::aws_lc_rs::DEFAULT_CIPHER_SUITES.len()
        );
    }

    #[test]
    fn test_invalid_cipher_list() {
        let ciphers = "ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512";
        let provider = provider(Some(ciphers));
        assert_eq!(
            provider.cipher_suites.len(),
            rustls::crypto::aws_lc_rs::DEFAULT_CIPHER_SUITES.len()
        );
    }

    #[test]
    fn test_some_valid_cipher_list() {
        let ciphers = "ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-CHACHA20-POLY1305-SHA256";
        let provider = provider(Some(ciphers));
        assert_eq!(provider.cipher_suites.len(), 2);
    }

    #[test]
    fn test_valid_cipher_list() {
        let ciphers = "TLS_AES_256_GCM_SHA384:TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256";
        let provider = provider(Some(ciphers));
        assert_eq!(provider.cipher_suites.len(), 3);
    }

    #[test]
    fn invalid_cipher_list_falls_back_to_default() {
        let provider = provider(Some("INVALID-CIPHER-FOO:BAR"));
        assert_eq!(
            provider.cipher_suites.len(),
            aws_lc_rs::DEFAULT_CIPHER_SUITES.len()
        );
    }

    #[test]
    fn none_cipher_list_falls_back_to_default() {
        let provider = provider(None);
        let ciphers_len = SECURE_CIPHERS.split(':').count();
        assert_eq!(provider.cipher_suites.len(), ciphers_len);
    }

    #[test]
    fn empty_cipher_list_falls_back_to_default() {
        let provider = provider(Some(""));
        let ciphers_len = SECURE_CIPHERS.split(':').count();

        assert_eq!(provider.cipher_suites.len(), ciphers_len);
    }
}
