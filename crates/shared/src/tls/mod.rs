// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use rustls::crypto::CryptoProvider;

use crate::log;

pub mod ciphers;
pub mod noverify;

/// Whether this build may disable TLS certificate verification.
///
/// "Don't verify" requests (config, `app_data.json` allowlist, JS
/// `check_certificate: false`) are only honored in debug builds or when the
/// binary was explicitly compiled with `--features insecure-tls`. A release
/// launcher without the feature answers `false` to everything: the flag is
/// forced to "verify" no matter what a local attacker writes into the
/// app-data file or the script parameters, closing the MiTM path over the
/// ML-KEM key exchange.
#[must_use]
pub const fn insecure_tls_bypass_allowed() -> bool {
    cfg!(debug_assertions) || cfg!(feature = "insecure-tls")
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CertificateInfo {
    pub key: String,
    pub certificate: String,
    pub password: Option<String>,
    pub ciphers: Option<String>,
}

// Ensure only one initialization happens
static INIT: std::sync::Once = std::sync::Once::new();

pub fn init_tls(ciphers_list: Option<&str>) {
    INIT.call_once(|| {
        // Build a provider with your custom cipher list
        log::debug!("Initializing TLS with ciphers: {:?}", ciphers_list);
        let provider: CryptoProvider = ciphers::provider(ciphers_list); // Defaults to all ciphers if None
        // Install it as the global default
        provider
            .install_default()
            .expect("failed to install default provider");
    });
}

#[cfg(test)]
mod tests {
    use super::insecure_tls_bypass_allowed;

    #[test]
    fn bypass_availability_matches_build_mode() {
        // Exactly two ways in: a debug build, or an explicit feature request.
        // `cargo test --release` (no feature) pins the inert branch, and every
        // caller ANDs its request against this predicate, so "always verify"
        // holds for config, app_data and JS flags alike.
        assert_eq!(
            insecure_tls_bypass_allowed(),
            cfg!(debug_assertions) || cfg!(feature = "insecure-tls")
        );
    }
}
