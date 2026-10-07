// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

// Converts the cryptic, transport-level TLS error strings produced by rustls
// (e.g. "invalid peer certificate: NotValidForName") into a single, actionable
// sentence the user can act on without reading the source code. `host` is
// included so the message names the exact endpoint that failed verification.

const HOST_UNKNOWN: &str = "<unknown host>";

/// Returns `true` if the message looks like a TLS-level error reported by
/// rustls or reqwest's TLS stack.
pub(crate) fn looks_like_tls_error(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("tls")
        || lower.contains("ssl")
        || lower.contains("certificate")
        || lower.contains("verify")
        || lower.contains("x509")
        || lower.contains("handshake")
}

/// Maps a rustls/reqwest TLS error string to an actionable message that
/// includes the host the client was trying to reach. Non-TLS errors are
/// returned prefixed with `"TLS: "` only if the input already smells like
/// TLS, otherwise the original message is returned unchanged so other
/// failure modes are not silenced.
pub fn classify(message: &str, host: &str) -> String {
    let host_label = if host.is_empty() { HOST_UNKNOWN } else { host };
    let lower = message.to_lowercase();

    if !looks_like_tls_error(message) {
        return message.to_string();
    }

    if lower.contains("notvalidforname") {
        return format!(
            "TLS: the server's certificate is not valid for {host_label}; \
             the certificate's Subject Alternative Name (SAN) does not include \
             this host. Use the hostname the certificate was issued for, or \
             reissue the certificate with {host_label} in its SAN."
        );
    }

    if lower.contains("unknownissuer") {
        return format!(
            "TLS: the server's certificate was issued by an untrusted \
             certificate authority ({host_label}). Install the CA in this \
             computer's trust store, or use a certificate signed by a \
             public CA."
        );
    }

    if lower.contains("selfsigned") {
        return format!(
            "TLS: the server uses a self-signed certificate ({host_label}). \
             Install its certificate authority in this computer's trust \
             store."
        );
    }

    if lower.contains("expired") {
        return format!(
            "TLS: the server's certificate is expired ({host_label}). \
             Renew the certificate on the server."
        );
    }

    if lower.contains("notvalidyet") {
        return format!(
            "TLS: the server's certificate is not yet valid ({host_label}). \
             Check the server clock and the certificate's notBefore date."
        );
    }

    if lower.contains("handshake") {
        return format!("TLS: handshake failed ({host_label}): {message}");
    }

    format!("TLS: {message} (host: {host_label})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_tls_message_is_returned_verbatim() {
        let raw = "connection refused";
        assert_eq!(classify(raw, "broker.example.com"), raw);
    }

    #[test]
    fn unknown_issuer_is_actionable() {
        let msg = classify(
            "invalid peer certificate: UnknownIssuer",
            "broker.example.com",
        );
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("untrusted"), "got: {msg}");
        assert!(msg.contains("broker.example.com"), "got: {msg}");
        assert!(!msg.contains("UnknownIssuer"));
    }

    #[test]
    fn not_valid_for_name_is_actionable() {
        let msg = classify("invalid peer certificate: NotValidForName", "192.168.15.69");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("Subject Alternative Name"), "got: {msg}");
        assert!(msg.contains("192.168.15.69"), "got: {msg}");
        assert!(!msg.contains("NotValidForName"));
    }

    #[test]
    fn expired_is_actionable() {
        let msg = classify("invalid peer certificate: Expired", "broker.example.com");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("expired"), "got: {msg}");
        assert!(msg.contains("broker.example.com"), "got: {msg}");
    }

    #[test]
    fn not_valid_yet_is_actionable() {
        let msg = classify(
            "invalid peer certificate: NotValidYet",
            "broker.example.com",
        );
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("not yet valid"), "got: {msg}");
    }

    #[test]
    fn self_signed_is_actionable() {
        let msg = classify("invalid peer certificate: SelfSigned", "broker.example.com");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("self-signed"), "got: {msg}");
    }

    #[test]
    fn generic_handshake_includes_host() {
        let msg = classify("TLS handshake failed: oh no", "broker.example.com");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("broker.example.com"), "got: {msg}");
    }

    #[test]
    fn unknown_tls_variant_keeps_message_and_includes_host() {
        let msg = classify(
            "invalid peer certificate: SomeNewVariant",
            "broker.example.com",
        );
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("SomeNewVariant"), "got: {msg}");
        assert!(msg.contains("broker.example.com"), "got: {msg}");
    }

    #[test]
    fn empty_host_falls_back_to_placeholder() {
        let msg = classify("invalid peer certificate: UnknownIssuer", "");
        assert!(msg.contains(HOST_UNKNOWN), "got: {msg}");
    }

    #[test]
    fn classifies_match_is_case_insensitive() {
        let msg = classify(
            "INVALID PEER CERTIFICATE: NOTVALIDFORNAME",
            "broker.example.com",
        );
        assert!(msg.contains("Subject Alternative Name"), "got: {msg}");
    }

    #[test]
    fn plain_handshake_keyword_is_classified() {
        // reqwest wraps rustls and the source chain often surfaces only the
        // word "handshake" without the rustls variant name.
        let msg = classify(
            "error sending request: handshake error",
            "broker.example.com",
        );
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("broker.example.com"), "got: {msg}");
    }
}
