//! Shared network plumbing for the transport adapters: DNS resolution (the
//! only place names are resolved; callers then dial a checked address) and
//! the rustls client configuration (ring provider only, system trust via
//! rustls-platform-verifier, or the script's `tls ca_file` roots).

use crate::domain::transports::TlsMaterial;
use crate::domain::{ErrorKind, RivetError, RivetResult};
use rustls::ClientConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls_platform_verifier::BuilderVerifierExt;
use std::net::IpAddr;
use std::sync::Arc;

/// Resolve `host:port` once through the system resolver.
pub async fn resolve(host: &str, port: u16) -> RivetResult<Vec<IpAddr>> {
    let addrs = tokio::net::lookup_host((host, port)).await.map_err(|e| {
        RivetError::new(
            ErrorKind::Dns,
            "dns.resolve",
            format!("cannot resolve {host}: {e}"),
        )
    })?;
    let mut ips: Vec<IpAddr> = Vec::new();
    for a in addrs {
        if !ips.contains(&a.ip()) {
            ips.push(a.ip());
        }
    }
    Ok(ips)
}

fn tls_err(msg: impl std::fmt::Display) -> RivetError {
    RivetError::new(
        ErrorKind::Tls,
        "tls.config",
        format!("TLS setup failed: {msg}"),
    )
}

/// Build a client config. Validation always stays on: either the platform
/// verifier (system trust store) or exactly the roots from `tls ca_file`.
pub fn client_config(m: &TlsMaterial, alpn: &[&[u8]]) -> RivetResult<Arc<ClientConfig>> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(tls_err)?;
    let builder = match &m.ca_pem {
        Some(pem) => {
            let mut roots = rustls::RootCertStore::empty();
            for cert in CertificateDer::pem_slice_iter(pem) {
                roots
                    .add(cert.map_err(|_| tls_err("tls ca_file is not PEM certificates"))?)
                    .map_err(tls_err)?;
            }
            if roots.is_empty() {
                return Err(tls_err("tls ca_file holds no certificate"));
            }
            builder.with_root_certificates(roots)
        }
        None => builder.with_platform_verifier().map_err(tls_err)?,
    };
    let mut cfg = match (&m.cert_pem, &m.key_pem) {
        (Some(cert), Some(key)) => {
            let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(cert)
                .collect::<Result<_, _>>()
                .map_err(|_| tls_err("tls cert_file is not PEM certificates"))?;
            // Never echo key material: the message names the option only.
            let key = PrivateKeyDer::from_pem_slice(key)
                .map_err(|_| tls_err("tls key_file is not a PEM private key"))?;
            builder
                .with_client_auth_cert(chain, key)
                .map_err(|_| tls_err("tls cert_file and key_file do not form a usable identity"))?
        }
        (None, None) => builder.with_no_client_auth(),
        _ => {
            return Err(RivetError::validation(
                "validation.tls",
                "`tls cert_file` and `tls key_file` must be given together",
            ));
        }
    };
    cfg.alpn_protocols = alpn.iter().map(|p| p.to_vec()).collect();
    Ok(Arc::new(cfg))
}

/// SNI / verification name: `tls server_name` or the URL host.
pub fn server_name(host: &str, m: &TlsMaterial) -> RivetResult<ServerName<'static>> {
    let name = m.server_name.clone().unwrap_or_else(|| {
        host.trim_start_matches('[')
            .trim_end_matches(']')
            .to_string()
    });
    ServerName::try_from(name.clone())
        .map_err(|_| tls_err(format!("`{name}` is not a valid TLS server name")))
}

/// Errors from establishing a TLS session.
pub fn handshake_err(e: std::io::Error) -> RivetError {
    RivetError::new(
        ErrorKind::Tls,
        "tls.handshake",
        format!("TLS handshake failed: {e}"),
    )
}

/// Errors from dialing.
pub fn connect_err(target: &str, e: std::io::Error) -> RivetError {
    let code = match e.kind() {
        std::io::ErrorKind::ConnectionRefused => "connection.refused",
        std::io::ErrorKind::TimedOut => "connection.timeout",
        _ => "connection.failed",
    };
    RivetError::new(
        ErrorKind::Connection,
        code,
        format!("cannot connect to {target}: {e}"),
    )
}
