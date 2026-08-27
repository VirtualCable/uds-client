// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::HashSet;
use std::sync::{Arc, LazyLock, RwLock};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, aws_lc_rs};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use rustls_native_certs::load_native_certs;
use sha2::{Digest, Sha256};

use crate::log;

static TRUSTED: LazyLock<RwLock<HashSet<String>>> = LazyLock::new(RwLock::default);
static LAST_REJECTED: LazyLock<RwLock<Option<String>>> = LazyLock::new(RwLock::default);

pub fn fingerprint(cert: &CertificateDer<'_>) -> String {
    Sha256::digest(cert.as_ref())
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<String>>()
        .join(":")
}

pub fn set_trusted(fingerprints: impl IntoIterator<Item = String>) {
    let mut trusted = TRUSTED.write().unwrap();
    *trusted = fingerprints.into_iter().collect();
}

pub fn trust(fingerprint: &str) {
    TRUSTED.write().unwrap().insert(fingerprint.to_string());
}

pub fn is_trusted(fingerprint: &str) -> bool {
    TRUSTED.read().unwrap().contains(fingerprint)
}

/// Fingerprint of the last certificate rejected by the verifier, so the caller can
/// show it to the user and ask whether to trust it.
pub fn last_rejected() -> Option<String> {
    LAST_REJECTED.read().unwrap().clone()
}

#[derive(Debug)]
pub struct PinnedVerifier {
    // None only if the system has no usable root certificates, where the pinned
    // fingerprints are the only way to accept a server.
    inner: Option<Arc<WebPkiServerVerifier>>,
}

impl PinnedVerifier {
    fn no_roots_error() -> rustls::Error {
        rustls::Error::General("No root certificates available".to_string())
    }
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fingerprint = fingerprint(end_entity);

        if is_trusted(&fingerprint) {
            log::debug!("Accepting pinned certificate {}", fingerprint);
            return Ok(ServerCertVerified::assertion());
        }

        let verified = match &self.inner {
            Some(inner) => inner.verify_server_cert(
                end_entity,
                intermediates,
                server_name,
                ocsp_response,
                now,
            ),
            None => Err(Self::no_roots_error()),
        };

        if let Err(e) = &verified {
            log::debug!("Certificate {} rejected: {}", fingerprint, e);
            *LAST_REJECTED.write().unwrap() = Some(fingerprint);
        }

        verified
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        match &self.inner {
            Some(inner) => inner.verify_tls12_signature(message, cert, dss),
            None => Err(Self::no_roots_error()),
        }
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        match &self.inner {
            Some(inner) => inner.verify_tls13_signature(message, cert, dss),
            None => Err(Self::no_roots_error()),
        }
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        match &self.inner {
            Some(inner) => inner.supported_verify_schemes(),
            None => aws_lc_rs::default_provider()
                .signature_verification_algorithms
                .supported_schemes(),
        }
    }
}

fn native_root_store() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    let certs_result = load_native_certs();

    for err in certs_result.errors {
        log::warn!("Failed to load a native certificate: {}", err);
    }

    for cert in certs_result.certs {
        root_store.add(cert).unwrap_or_else(|e| {
            log::warn!("Failed to add a native certificate to root store: {:?}", e);
        });
    }

    root_store
}

// Uses the provider installed by init_tls when there is one, but never installs a new
// one as a side effect: init_tls must stay the only place that sets the process default.
fn crypto_provider() -> Arc<CryptoProvider> {
    CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(aws_lc_rs::default_provider()))
}

pub fn verifier() -> Arc<PinnedVerifier> {
    let inner =
        WebPkiServerVerifier::builder_with_provider(Arc::new(native_root_store()), crypto_provider())
            .build()
            .map_err(|e| log::error!("Could not build the certificate verifier: {}", e))
            .ok();

    Arc::new(PinnedVerifier { inner })
}

pub fn client_config() -> ClientConfig {
    ClientConfig::builder_with_provider(crypto_provider())
        .with_safe_default_protocol_versions()
        .expect("failed to configure TLS protocol versions")
        .dangerous()
        .with_custom_certificate_verifier(verifier())
        .with_no_client_auth()
}

#[cfg(test)]
mod tests {
    use super::*;

    use rcgen::generate_simple_self_signed;
    use serial_test::serial;

    fn self_signed(name: &str) -> CertificateDer<'static> {
        let cert = generate_simple_self_signed(vec![name.to_string()]).unwrap();
        CertificateDer::from(cert.cert.der().to_vec())
    }

    fn verify(cert: &CertificateDer<'_>) -> Result<ServerCertVerified, rustls::Error> {
        crate::tls::init_tls(None);
        verifier().verify_server_cert(
            cert,
            &[],
            &ServerName::try_from("localhost").unwrap(),
            &[],
            UnixTime::now(),
        )
    }

    #[test]
    fn fingerprint_is_a_sha256_of_the_der() {
        let cert = self_signed("localhost");
        let fingerprint = fingerprint(&cert);

        assert_eq!(fingerprint.len(), 32 * 3 - 1);
        assert_eq!(fingerprint.matches(':').count(), 31);
        assert_eq!(fingerprint, fingerprint.to_uppercase());
    }

    #[test]
    fn different_certificates_have_different_fingerprints() {
        assert_ne!(
            fingerprint(&self_signed("localhost")),
            fingerprint(&self_signed("localhost"))
        );
    }

    #[test]
    #[serial]
    fn untrusted_certificate_is_rejected_and_remembered() {
        let cert = self_signed("localhost");
        set_trusted([]);

        assert!(verify(&cert).is_err());
        assert_eq!(last_rejected(), Some(fingerprint(&cert)));
    }

    #[test]
    #[serial]
    fn trusted_fingerprint_is_accepted() {
        let cert = self_signed("localhost");
        set_trusted([]);

        assert!(verify(&cert).is_err());

        trust(&fingerprint(&cert));

        assert!(verify(&cert).is_ok());
    }

    #[test]
    #[serial]
    fn trusting_one_certificate_does_not_trust_another() {
        let trusted = self_signed("localhost");
        let other = self_signed("localhost");
        set_trusted([fingerprint(&trusted)]);

        assert!(verify(&trusted).is_ok());
        assert!(verify(&other).is_err());
    }

    #[test]
    #[serial]
    fn set_trusted_replaces_previous_fingerprints() {
        let cert = self_signed("localhost");
        set_trusted([fingerprint(&cert)]);
        set_trusted(["AA:BB".to_string()]);

        assert!(!is_trusted(&fingerprint(&cert)));
        assert!(is_trusted("AA:BB"));
    }
}
