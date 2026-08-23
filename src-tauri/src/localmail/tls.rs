//! Certificate-fingerprint verification for the explicit Localmail trust flow.

use std::sync::{Arc, Mutex};

use rustls::{
    DigitallySignedStruct, Error as TlsError, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
};
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use sha2::{Digest, Sha256};

/// Whether a TLS connection is discovering or enforcing a leaf-certificate fingerprint.
#[derive(Clone, Debug)]
pub(super) enum CertificateMode {
    /// Accept one presented leaf certificate and retain its fingerprint for user confirmation.
    Inspect,
    /// Accept only the exact fingerprint previously confirmed by the user.
    Pinned(String),
}

/// Rustls verifier that proves server key possession while applying Bottie's explicit pin policy.
#[derive(Debug)]
pub(super) struct CertificateVerifier {
    mode: CertificateMode,
    captured: Mutex<Option<String>>,
    crypto_provider: Arc<CryptoProvider>,
}

impl CertificateVerifier {
    /// Creates a verifier with the requested inspection or pinning behavior.
    pub(super) fn new(mode: CertificateMode) -> Arc<Self> {
        Arc::new(Self {
            mode,
            captured: Mutex::new(None),
            crypto_provider: Arc::new(rustls::crypto::ring::default_provider()),
        })
    }

    /// Returns the lowercase SHA-256 fingerprint captured from the leaf certificate.
    pub(super) fn captured_fingerprint(&self) -> Option<String> {
        self.captured.lock().ok()?.clone()
    }
}

impl ServerCertVerifier for CertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let fingerprint = certificate_fingerprint(end_entity.as_ref());
        if let Ok(mut captured) = self.captured.lock() {
            *captured = Some(fingerprint.clone());
        }
        match &self.mode {
            CertificateMode::Inspect => Ok(ServerCertVerified::assertion()),
            CertificateMode::Pinned(expected) if fingerprint.eq_ignore_ascii_case(expected) => {
                Ok(ServerCertVerified::assertion())
            }
            CertificateMode::Pinned(_) => Err(TlsError::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            )),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.crypto_provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.crypto_provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.crypto_provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// Hashes one DER leaf certificate into the user-confirmable pin representation.
fn certificate_fingerprint(certificate: &[u8]) -> String {
    Sha256::digest(certificate)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a stable server name for direct verifier policy tests.
    fn server_name() -> ServerName<'static> {
        ServerName::try_from("localmail.example").expect("test server name should parse")
    }

    #[test]
    fn inspection_captures_a_lowercase_sha256_fingerprint() {
        let certificate = CertificateDer::from(vec![1, 2, 3, 4]);
        let verifier = CertificateVerifier::new(CertificateMode::Inspect);
        verifier
            .verify_server_cert(&certificate, &[], &server_name(), &[], UnixTime::now())
            .expect("inspection should accept the presented certificate");

        let fingerprint = verifier.captured_fingerprint().expect("fingerprint");
        assert_eq!(fingerprint.len(), 64);
        assert!(
            fingerprint
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        );
    }

    #[test]
    fn pinned_mode_accepts_only_the_confirmed_certificate() {
        let certificate = CertificateDer::from(vec![4, 3, 2, 1]);
        let expected = certificate_fingerprint(certificate.as_ref());
        let matching = CertificateVerifier::new(CertificateMode::Pinned(expected));
        let mismatching = CertificateVerifier::new(CertificateMode::Pinned("0".repeat(64)));

        assert!(
            matching
                .verify_server_cert(&certificate, &[], &server_name(), &[], UnixTime::now())
                .is_ok()
        );
        assert!(
            mismatching
                .verify_server_cert(&certificate, &[], &server_name(), &[], UnixTime::now())
                .is_err()
        );
    }
}
