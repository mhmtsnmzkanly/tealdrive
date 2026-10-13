//! Root-only certificate diagnostics. No private key material is printed.
use super::*;
pub(super) fn status() -> Result<()> {
    let cfg = config()?;
    if !cfg.server.direct_tls() {
        return print(&serde_json::json!({"directTls":false}));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(crate::tls::load(&cfg.server))?;
    let cert = cfg
        .server
        .tls_cert_path
        .as_deref()
        .ok_or("Missing certificate")?;
    let executable = Path::new("/usr/bin/openssl");
    trusted_directory(executable.parent().ok_or("Utility parent")?)?;
    let metadata = std::fs::symlink_metadata(executable)?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err("Untrusted certificate utility".into());
    }
    let output = Command::new(executable)
        .args([
            "x509",
            "-noout",
            "-startdate",
            "-enddate",
            "-fingerprint",
            "-sha256",
            "-in",
        ])
        .arg(cert)
        .env_clear()
        .output()?;
    if !output.status.success() {
        return Err("Certificate inspection failed".into());
    }
    let expiry = Command::new(executable)
        .args(["x509", "-noout", "-checkend", "2592000", "-in"])
        .arg(cert)
        .env_clear()
        .output()?;
    print(&serde_json::json!({
        "directTls":true,
        "certificatePath":cert,
        "pairValidated":true,
        "certificate":String::from_utf8(output.stdout)?,
        "renewWithin30Days":!expiry.status.success(),
        "refreshSeconds":60,
        "proxyTrustMustBeUpdatedSeparately":true
    }))
}
