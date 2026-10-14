//! Root-only resumable self-signed backend renewal. The proxy adapter is optional.
//! Trust both identities before publishing; leave overlap in place on any failure.
use super::*;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::DirBuilderExt;
const STAGE: &str = "/home/tealdrive/tls/renewal";
const TRUST: &str = "/home/tealdrive/policy/origin-trust.pem";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Renewal {
    version: u8,
    name: String,
    cert: PathBuf,
    key: PathBuf,
    phase: u8,
}
fn name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn read(path: &Path, private: bool) -> Result<Vec<u8>> {
    trusted_directory(path.parent().ok_or("Invalid TLS path")?)?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file()
        || meta.uid() != 0
        || meta.nlink() != 1
        || meta.mode() & if private { 0o027 } else { 0o022 } != 0
        || meta.len() > 1024 * 1024
    {
        return Err("Untrusted TLS renewal file".into());
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("Oversized TLS renewal file".into());
    }
    Ok(bytes)
}
fn utility(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    trusted_directory(Path::new(program).parent().ok_or("Invalid utility")?)?;
    let meta = std::fs::symlink_metadata(program)?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
        return Err("Untrusted TLS utility".into());
    }
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .output()?;
    if !output.status.success() {
        return Err(
            "TLS/proxy utility failed; overlapping trust and renewal state retained".into(),
        );
    }
    Ok(output.stdout)
}
fn text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| "Non-UTF8 TLS path".into())
}
fn validate_pair(cfg: &AppConfig, cert: &Path, key: &Path) -> Result<()> {
    let mut server = cfg.server.clone();
    server.tls_cert_path = Some(cert.into());
    server.tls_key_path = Some(key.into());
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(crate::tls::load(&server))?;
    Ok(())
}
fn save(directory: &Path, state: &Renewal) -> Result<()> {
    atomic(
        &directory.join("state.json"),
        &serde_json::to_vec(state)?,
        0o600,
        0,
        0,
    )
}
fn prepare_at(cfg: &AppConfig, directory: &Path, name: &str) -> Result<Renewal> {
    if !name_valid(name) || !cfg.server.direct_tls() {
        return Err("A DNS server name and existing backend TLS pair are required".into());
    }
    let cert = cfg
        .server
        .tls_cert_path
        .clone()
        .ok_or("TLS certificate missing")?;
    let key = cfg.server.tls_key_path.clone().ok_or("TLS key missing")?;
    if directory.exists() {
        trusted_directory(directory)?;
        let state: Renewal = serde_json::from_slice(&read(&directory.join("state.json"), true)?)?;
        if state.version != 1
            || state.name != name
            || state.cert != cert
            || state.key != key
            || state.phase > 2
        {
            return Err("Pending renewal differs; inspect before retrying".into());
        }
        validate_pair(cfg, &directory.join("new.crt"), &directory.join("new.key"))?;
        return Ok(state);
    }
    validate_pair(cfg, &cert, &key)?;
    trusted_directory(directory.parent().ok_or("Invalid renewal directory")?)?;
    std::fs::DirBuilder::new().mode(0o700).create(directory)?;
    // Failed preparation is unpublished; it can be safely discarded and retried.
    let result = (|| {
        atomic(
            &directory.join("old.crt"),
            &read(&cert, false)?,
            0o600,
            0,
            0,
        )?;
        atomic(&directory.join("old.key"), &read(&key, true)?, 0o600, 0, 0)?;
        let new_key = directory.join("new.key");
        let new_cert = directory.join("new.crt");
        utility(
            "/usr/bin/openssl",
            &[
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-days",
                "365",
                "-subj",
                &format!("/CN={name}"),
                "-addext",
                &format!("subjectAltName=DNS:{name}"),
                "-keyout",
                text(&new_key)?,
                "-out",
                text(&new_cert)?,
            ],
        )?;
        std::fs::set_permissions(&new_key, std::fs::Permissions::from_mode(0o600))?;
        validate_pair(cfg, &new_cert, &new_key)?;
        utility(
            "/usr/bin/openssl",
            &[
                "x509",
                "-noout",
                "-checkhost",
                name,
                "-in",
                text(&new_cert)?,
            ],
        )?;
        let state = Renewal {
            version: 1,
            name: name.into(),
            cert,
            key,
            phase: 0,
        };
        save(directory, &state)?;
        Ok(state)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(directory);
    }
    result
}
/// Rewrites only a single explicit trust-pool line. Complex configurations fail closed.
fn proxy_config(source: &str, old: &Path, trust: &Path) -> Result<String> {
    let mut count = 0;
    let mut output = String::new();
    for line in source.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.len() == 3
            && words[0] == "tls_trust_pool"
            && words[1] == "file"
            && (words[2] == text(old)? || words[2] == text(trust)?)
        {
            count += 1;
            output.push_str(&format!(
                "{}tls_trust_pool file {}\n",
                &line[..line.len() - line.trim_start().len()],
                text(trust)?
            ));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if count != 1 {
        return Err(
            "Caddy adapter requires exactly one explicit matching tls_trust_pool file line".into(),
        );
    }
    Ok(output)
}
fn caddy_reload(path: &Path, old: &Path, trust: &Path) -> Result<()> {
    let original = read(path, true)?;
    let content = proxy_config(std::str::from_utf8(&original)?, old, trust)?;
    let meta = std::fs::symlink_metadata(path)?;
    // Adapt first without publishing the changed config. Never log adapted JSON:
    // other sites can contain credentials.
    let temporary = path
        .parent()
        .ok_or("Caddy parent")?
        .join(format!(".tealdrive-caddy-{}", uuid::Uuid::new_v4()));
    atomic(&temporary, content.as_bytes(), 0o600, 0, 0)?;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(temporary.clone());
    let adapted = utility(
        "/usr/bin/caddy",
        &[
            "adapt",
            "--config",
            text(&temporary)?,
            "--adapter",
            "caddyfile",
        ],
    )?;
    let json: serde_json::Value = serde_json::from_slice(&adapted)?;
    let address = json
        .pointer("/admin/listen")
        .and_then(|v| v.as_str())
        .ok_or("Caddy adapter requires explicit protected Unix admin socket")?;
    if !address.starts_with("unix//") {
        return Err("Caddy admin must use a protected Unix socket".into());
    }
    let socket_path = Path::new(
        address
            .strip_prefix("unix/")
            .ok_or("Invalid admin socket")?,
    );
    let parent = std::fs::symlink_metadata(socket_path.parent().ok_or("Socket parent")?)?;
    if !parent.is_dir() || parent.mode() & 0o077 != 0 {
        return Err("Caddy admin socket directory must be private".into());
    }
    utility(
        "/usr/bin/caddy",
        &[
            "validate",
            "--config",
            text(&temporary)?,
            "--adapter",
            "caddyfile",
        ],
    )?;
    atomic(
        path,
        content.as_bytes(),
        meta.mode() & 0o777,
        meta.uid(),
        meta.gid(),
    )?;
    if let Err(error) = utility(
        "/usr/bin/caddy",
        &[
            "reload",
            "--config",
            text(path)?,
            "--adapter",
            "caddyfile",
            "--address",
            address,
        ],
    ) {
        atomic(path, &original, meta.mode() & 0o777, meta.uid(), meta.gid())?;
        return Err(error);
    }
    Ok(())
}
fn peer_is_new(cfg: &AppConfig, state: &Renewal, new_cert: &Path) -> Result<bool> {
    let addr: std::net::SocketAddr = cfg.server.bind_addr.parse()?;
    if !addr.ip().is_loopback() {
        return Err("TLS renewal probe requires loopback backend bind".into());
    }
    // Independent TLS verification plus exact DER identity comparison. This probe
    // must finish within five seconds even if a local listener stops responding.
    let script="import ssl,socket,sys; c=ssl.create_default_context(cafile=sys.argv[3]); s=c.wrap_socket(socket.create_connection((sys.argv[1],int(sys.argv[2])),timeout=5),server_hostname=sys.argv[4]); actual=s.getpeercert(binary_form=True); expected=ssl.PEM_cert_to_DER_cert(open(sys.argv[3]).read()); s.close(); sys.exit(0 if actual==expected else 1)";
    let status = Command::new("/usr/bin/python3")
        .args([
            "-c",
            script,
            &addr.ip().to_string(),
            &addr.port().to_string(),
            text(new_cert)?,
            &state.name,
        ])
        .env_clear()
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    Ok(status.success())
}
pub(super) fn prepare(name: &str) -> Result<()> {
    let _lock = crate::accounts::virtuals::lock()?;
    let cfg = config()?;
    prepare_at(&cfg, Path::new(STAGE), name)?;
    let mut bundle = read(&Path::new(STAGE).join("old.crt"), false)?;
    bundle.push(b'\n');
    bundle.extend(read(&Path::new(STAGE).join("new.crt"), false)?);
    atomic(Path::new(TRUST), &bundle, 0o644, 0, 0)?;
    print(
        &serde_json::json!({"prepared":true,"publicTrustBundle":TRUST,"published":false,"next":"Configure and reload proxy trust using this bundle before activate; Caddy adapter is optional"}),
    )
}
pub(super) fn activate(name: &str, caddy: Option<&Path>) -> Result<()> {
    let _lock = crate::accounts::virtuals::lock()?;
    let cfg = config()?;
    let directory = Path::new(STAGE);
    let mut state = prepare_at(&cfg, directory, name)?;
    let new_cert = directory.join("new.crt");
    let new_key = directory.join("new.key");
    let mut bundle = read(&directory.join("old.crt"), false)?;
    bundle.push(b'\n');
    bundle.extend(read(&new_cert, false)?);
    atomic(Path::new(TRUST), &bundle, 0o644, 0, 0)?;
    // Without the adapter the operator explicitly invokes activate after their
    // own nginx/Apache/custom proxy has loaded the generated public trust bundle.
    if let Some(path) = caddy {
        caddy_reload(path, &state.cert, Path::new(TRUST))?;
    }
    state.phase = 1;
    save(directory, &state)?;
    let meta = std::fs::symlink_metadata(&state.key)?;
    read(&state.key, true)?;
    read(&state.cert, false)?;
    atomic(&state.key, &read(&new_key, true)?, 0o640, 0, meta.gid())?;
    atomic(&state.cert, &read(&new_cert, false)?, 0o644, 0, 0)?;
    validate_pair(&cfg, &state.cert, &state.key)?;
    state.phase = 2;
    save(directory, &state)?;
    let mut verified = false;
    for _ in 0..38 {
        if peer_is_new(&cfg, &state, &new_cert)? {
            verified = true;
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    if !verified {
        return Err("New backend identity not yet observed; overlapping trust retained. Retry activate after fixing the listener".into());
    }
    // Retain overlap for generic proxies until their explicit reload; Caddy can
    // finish automatically. Old trust is public, never an extra private key.
    if let Some(path) = caddy {
        atomic(Path::new(TRUST), &read(&new_cert, false)?, 0o644, 0, 0)?;
        if let Err(error) = caddy_reload(path, &state.cert, Path::new(TRUST)) {
            atomic(Path::new(TRUST), &bundle, 0o644, 0, 0)?;
            return Err(error);
        }
    }
    std::fs::remove_dir_all(directory)?;
    std::fs::File::open(directory.parent().ok_or("Renewal parent")?)?.sync_all()?;
    print(
        &serde_json::json!({"renewed":true,"backendIdentityVerified":true,"proxyAdapter":if caddy.is_some(){"caddy"}else{"external"},"oldTrustRetained":caddy.is_none()}),
    )
}
pub(super) fn renew(name: &str, caddy: &Path) -> Result<()> {
    let cfg = config()?;
    let cert = cfg
        .server
        .tls_cert_path
        .as_deref()
        .ok_or("Backend TLS required")?;
    if !Path::new(STAGE).exists() {
        validate_pair(
            &cfg,
            cert,
            cfg.server
                .tls_key_path
                .as_deref()
                .ok_or("Missing TLS key")?,
        )?;
        // checkend exit 1 is expected for near-expiry certificates.
        let output = Command::new("/usr/bin/openssl")
            .args(["x509", "-noout", "-checkend", "2592000", "-in", text(cert)?])
            .env_clear()
            .output()?;
        if output.status.success() {
            return print(
                &serde_json::json!({"renewed":false,"reason":"certificate valid beyond 30 days"}),
            );
        }
    }
    activate(name, Some(caddy))
}
pub(super) fn automatic(name: &str, caddy: &Path) -> Result<()> {
    if !name_valid(name)
        || !caddy.is_absolute()
        || !text(caddy)?
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err("Invalid automatic renewal arguments".into());
    }
    read(caddy, true)?;
    trusted_directory(Path::new("/etc/systemd/system"))?;
    let unit=format!("[Unit]\nDescription=Tealdrive backend certificate renewal\nAfter=network-online.target tealdrive.service\n\n[Service]\nType=oneshot\nUser=root\nUMask=0077\nExecStart=/home/tealdrive/bin/tealdrive-wizard tls renew {name} --caddy {}\nTimeoutStartSec=300\n",text(caddy)?);
    atomic(
        Path::new("/etc/systemd/system/tealdrive-tls-renewal.service"),
        unit.as_bytes(),
        0o644,
        0,
        0,
    )?;
    atomic(Path::new("/etc/systemd/system/tealdrive-tls-renewal.timer"),b"[Unit]\nDescription=Daily Tealdrive backend TLS renewal check\n\n[Timer]\nOnCalendar=daily\nRandomizedDelaySec=3600\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",0o644,0,0)?;
    command("/usr/bin/systemctl", &["daemon-reload"])?;
    command(
        "/usr/bin/systemctl",
        &["enable", "--now", "tealdrive-tls-renewal.timer"],
    )?;
    print(
        &serde_json::json!({"automaticRenewal":true,"timer":"tealdrive-tls-renewal.timer","thresholdDays":30}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit Root and openssl acceptance; creates only a private temporary directory"]
    fn actual_preparation_validates_pair_resume_and_name_without_publishing() {
        assert_eq!(unsafe { libc::getuid() }, 0);
        let directory =
            Path::new("/root").join(format!(".tealdrive-renewal-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let cert = directory.join("active.crt");
        let key = directory.join("active.key");
        utility(
            "/usr/bin/openssl",
            &[
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:P-256",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-keyout",
                text(&key).unwrap(),
                "-out",
                text(&cert).unwrap(),
            ],
        )
        .unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut cfg = AppConfig::default();
        cfg.server.tls_cert_path = Some(cert.clone());
        cfg.server.tls_key_path = Some(key.clone());
        cfg.server.require_tls = Some(true);
        cfg.server.behind_tls_proxy = false;
        let original = std::fs::read(&cert).unwrap();
        let stage = directory.join("renewal");
        prepare_at(&cfg, &stage, "localhost").unwrap();
        assert_eq!(std::fs::read(&cert).unwrap(), original);
        let candidate = std::fs::read(stage.join("new.crt")).unwrap();
        assert_ne!(candidate, original);
        prepare_at(&cfg, &stage, "localhost").unwrap();
        assert_eq!(std::fs::read(stage.join("new.crt")).unwrap(), candidate);
        assert!(prepare_at(&cfg, &stage, "different.local").is_err());
        assert_eq!(
            std::fs::metadata(stage.join("new.key")).unwrap().mode() & 0o777,
            0o600
        );
        assert!(prepare_at(&cfg, &directory.join("bad"), "localhost;id").is_err());
    }
    #[test]
    fn validates_dns_names_and_restricts_caddy_rewrite() {
        for name in ["localhost", "tealdrive-origin.local", "nmr.novastrum.dev"] {
            assert!(name_valid(name));
        }
        for name in [
            "",
            "-host",
            "host-",
            "a..b",
            "a/b",
            "host\ncommand",
            "host;touch",
            "*.example",
        ] {
            assert!(!name_valid(name));
        }
        let old = Path::new("/old.pem");
        let trust = Path::new("/trust.pem");
        let source="transport http {\n  tls_trust_pool file /old.pem\n  tls_server_name example.local\n}\n";
        assert_eq!(
            proxy_config(source, old, trust).unwrap(),
            source.replace("/old.pem", "/trust.pem")
        );
        assert!(proxy_config("tls_trust_pool file /other.pem", old, trust).is_err());
        assert!(proxy_config(&format!("{source}{source}"), old, trust).is_err());
        assert!(proxy_config("tls_trust_pool file /old.pem /other.pem", old, trust).is_err());
    }
}
