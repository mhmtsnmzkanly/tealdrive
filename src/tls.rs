//! Standard rustls TLS, with Root-managed key/certificate paths and no HTTP fallback.
use crate::{config::ServerConfig, errors::TealDriveError};
use std::{
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
fn pem(path: &Path, private: bool) -> std::io::Result<Vec<u8>> {
    for parent in path.ancestors().skip(1) {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.nlink() != 1
        || metadata.mode() & (if private { 0o027 } else { 0o022 }) != 0
    {
        return Err(std::io::ErrorKind::PermissionDenied.into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(bytes)
}
pub async fn load(
    config: &ServerConfig,
) -> Result<Option<axum_server::tls_rustls::RustlsConfig>, TealDriveError> {
    if !config.direct_tls() {
        return Ok(None);
    }
    let cert = pem(
        config
            .tls_cert_path
            .as_deref()
            .ok_or(TealDriveError::Validation)?,
        false,
    )
    .map_err(|_| TealDriveError::Validation)?;
    let key = zeroize::Zeroizing::new(
        pem(
            config
                .tls_key_path
                .as_deref()
                .ok_or(TealDriveError::Validation)?,
            true,
        )
        .map_err(|_| TealDriveError::Validation)?,
    );
    let certificates = rustls_pemfile::certs(&mut cert.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| TealDriveError::Validation)?;
    let mut keys = Vec::new();
    for item in rustls_pemfile::read_all(&mut key.as_slice()) {
        match item.map_err(|_| TealDriveError::Validation)? {
            rustls_pemfile::Item::Pkcs1Key(key) => {
                keys.push(rustls::pki_types::PrivateKeyDer::Pkcs1(key))
            }
            rustls_pemfile::Item::Pkcs8Key(key) => {
                keys.push(rustls::pki_types::PrivateKeyDer::Pkcs8(key))
            }
            rustls_pemfile::Item::Sec1Key(key) => {
                keys.push(rustls::pki_types::PrivateKeyDer::Sec1(key))
            }
            _ => {}
        }
    }
    if keys.len() != 1 {
        return Err(TealDriveError::Validation);
    }
    let mut server = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| TealDriveError::Validation)?
    .with_no_client_auth()
    .with_single_cert(certificates, keys.pop().ok_or(TealDriveError::Validation)?)
    .map_err(|_| TealDriveError::Validation)?;
    server.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Some(axum_server::tls_rustls::RustlsConfig::from_config(
        std::sync::Arc::new(server),
    )))
}
