use rustls::ClientConfig;
use rustls::{client::danger::ServerCertVerifier, pki_types::CertificateDer};

use crate::log;

#[derive(Debug)]
pub struct NoVerifySsl;

impl NoVerifySsl {
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}

impl ServerCertVerifier for NoVerifySsl {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        log::debug!("Skipping server verification");
        // log::debug!("End entity: {:?}", _end_entity);
        // log::debug!("Intermediates: {:?}", _intermediates);
        // log::debug!("Server name: {:?}", _server_name);
        // log::debug!("OCSP response: {:?}", _ocsp_response);
        // log::debug!("Now: {:?}", _now);
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        log::debug!("Skipping TLS 1.2 signature verification");
        // log::debug!("Message: {:?}", _message);
        // log::debug!("Cert: {:?}", _cert);
        // log::debug!("DSS: {:?}", _dss);
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        log::debug!("Skipping TLS 1.3 signature verification");
        // log::debug!("Message: {:?}", _message);
        // log::debug!("Cert: {:?}", _cert);
        // log::debug!("DSS: {:?}", _dss);
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        log::debug!("Supported verify schemes");
        vec![
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
            rustls::SignatureScheme::RSA_PKCS1_SHA1,
            rustls::SignatureScheme::ECDSA_SHA1_Legacy,
        ]
    }
}

pub fn client_config() -> std::sync::Arc<ClientConfig> {
    std::sync::Arc::new(
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(NoVerifySsl::new())
            .with_no_client_auth(),
    )
}
