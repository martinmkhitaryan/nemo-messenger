//! TLS 1.3 + pinned Ed25519 certificates for the S2S hop (ADR-0032).

use std::sync::Arc;

use nemo_wire::ids::KEY_LEN;
use rcgen::{CertificateParams, DnType, KeyPair, PKCS_ED25519};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, Error as TlsError, ServerConfig,
    SignatureScheme,
};
use tokio::sync::Mutex;
use x509_parser::prelude::FromDer;

use crate::home::HomeServer;

pub const ALPN: &[u8] = b"nemo-s2s/1";

/// PKCS#8 v1 wrapping an Ed25519 seed (RFC 8410).
pub fn ed25519_pkcs8(seed: &[u8; KEY_LEN]) -> Vec<u8> {
    let mut out = vec![
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
        0x20,
    ];
    out.extend_from_slice(seed);
    out
}

pub fn install_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn self_signed(home: &HomeServer) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), String> {
    let pkcs8 = ed25519_pkcs8(&home.signing_key().to_bytes());
    let key_pair = KeyPair::from_pkcs8_der_and_sign_algo(&pkcs8.clone().into(), &PKCS_ED25519)
        .map_err(|e| e.to_string())?;
    let host = if home.bundle().host.is_empty() {
        "localhost".into()
    } else {
        home.bundle().host.clone()
    };
    let mut params = CertificateParams::new(vec![host]).map_err(|e| e.to_string())?;
    params
        .distinguished_name
        .push(DnType::CommonName, "nemo-s2s");
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| e.to_string())?;
    let presented = ed25519_spki(cert.der()).map_err(|e| e.to_string())?;
    if presented != home.sign_public() {
        return Err("self-signed cert SPKI does not match server signing key".into());
    }
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8));
    Ok((cert.der().clone(), key))
}

pub fn ed25519_spki(der: &CertificateDer<'_>) -> Result<[u8; KEY_LEN], TlsError> {
    let (_, cert) = x509_parser::certificate::X509Certificate::from_der(der.as_ref())
        .map_err(|_| TlsError::General("x509 parse".into()))?;
    let bits = cert.public_key().subject_public_key.as_ref();
    if bits.len() != KEY_LEN {
        return Err(TlsError::General("spki length".into()));
    }
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(bits);
    Ok(out)
}

fn schemes() -> Vec<SignatureScheme> {
    vec![SignatureScheme::ED25519]
}

fn verify_sig(
    message: &[u8],
    cert: &CertificateDer<'_>,
    dss: &DigitallySignedStruct,
) -> Result<HandshakeSignatureValid, TlsError> {
    rustls::crypto::verify_tls13_signature(
        message,
        cert,
        dss,
        &rustls::crypto::ring::default_provider().signature_verification_algorithms,
    )
}

struct PinnedClient {
    home: Arc<Mutex<HomeServer>>,
}

impl std::fmt::Debug for PinnedClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PinnedClient")
    }
}

impl ClientCertVerifier for PinnedClient {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        let pk = ed25519_spki(end_entity)?;
        let home = self
            .home
            .try_lock()
            .map_err(|_| TlsError::General("home lock".into()))?;
        if home.peer_id_for_sign_key(&pk).is_some() {
            Ok(ClientCertVerified::assertion())
        } else {
            Err(TlsError::General("unpinned client".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_sig(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }

    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct PinnedServer {
    expect: [u8; KEY_LEN],
}

impl ServerCertVerifier for PinnedServer {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let pk = ed25519_spki(end_entity)?;
        if pk == self.expect {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::General("server pin mismatch".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Err(TlsError::PeerIncompatible(
            rustls::PeerIncompatible::Tls12NotOffered,
        ))
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_sig(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        schemes()
    }
}

pub async fn server_config(home: Arc<Mutex<HomeServer>>) -> Result<Arc<ServerConfig>, String> {
    install_provider();
    let guard = home.lock().await;
    let (cert, key) = self_signed(&guard)?;
    drop(guard);
    let verifier = Arc::new(PinnedClient { home });
    let mut cfg = ServerConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| e.to_string())?
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)
        .map_err(|e| e.to_string())?;
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(cfg))
}

pub fn client_config(
    home: &HomeServer,
    peer_sign: [u8; KEY_LEN],
) -> Result<Arc<ClientConfig>, String> {
    install_provider();
    let (cert, key) = self_signed(home)?;
    let mut cfg = ClientConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedServer { expect: peer_sign }))
        .with_client_auth_cert(vec![cert], key)
        .map_err(|e| e.to_string())?;
    cfg.alpn_protocols = vec![ALPN.to_vec()];
    Ok(Arc::new(cfg))
}
