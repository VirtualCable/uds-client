// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use rustls::crypto::CryptoProvider;

use crate::log;

pub mod ciphers;
// The TLS verification bypass machinery is compiled ONLY into insecure
// builds (debug or `--features insecure-tls`). A release binary built
// without the feature does not contain it at all: skipping certificate
// verification is not possible, not even by patching the call sites.
#[cfg(any(debug_assertions, feature = "insecure-tls"))]
pub mod noverify;

/// Whether this build skips TLS certificate verification entirely.
///
/// Insecure builds (debug or compiled with `--features insecure-tls`) never
/// verify certificates, on any connection: broker API and v4 tunnels alike.
/// The launcher shows a blocking warning dialog at startup for those builds,
/// so the user is told before anything else happens. Any other build always
/// verifies: there is no runtime knob left (no config, no `app_data.json`
/// entry, no JS `check_certificate` flag) that can turn verification off,
/// closing the MiTM path over the ML-KEM key exchange.
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
        // Insecure builds (this predicate) never verify certificates and show
        // a startup warning; secure builds don't even contain the bypass
        // code, so "always verify" holds no matter what the user does.
        assert_eq!(
            insecure_tls_bypass_allowed(),
            cfg!(debug_assertions) || cfg!(feature = "insecure-tls")
        );
    }
}
