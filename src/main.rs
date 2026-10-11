use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use tealdrive::audit::sink::JsonlAuditSink;
use tealdrive::config::AppConfig;
use tealdrive::errors::TealDriveError;
use tealdrive::helper::client::{default_helper_binary_path, HelperProcessClient};
use tealdrive::session::InMemorySessionStore;
use tealdrive::ws::server::{run_server, WsServerState};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ServerArgs {
    pub config_path: Option<PathBuf>,
    pub bind_addr: Option<String>,
    pub static_dir: Option<PathBuf>,
    pub helper_bin: Option<PathBuf>,
    pub check_config: bool,
    pub show_version: bool,
    pub show_help: bool,
}

impl ServerArgs {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut parsed = Self::default();
        let mut iter = args.into_iter().skip(1);
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-c" | "--config" => {
                    let val = iter
                        .next()
                        .ok_or_else(|| "--config requires a path argument".to_owned())?;
                    parsed.config_path = Some(PathBuf::from(val));
                }
                "-b" | "--bind" => {
                    let val = iter
                        .next()
                        .ok_or_else(|| "--bind requires an address argument".to_owned())?;
                    parsed.bind_addr = Some(val);
                }
                "--static-dir" | "--assets-dir" => {
                    let val = iter
                        .next()
                        .ok_or_else(|| "--static-dir / --assets-dir requires a path argument".to_owned())?;
                    parsed.static_dir = Some(PathBuf::from(val));
                }
                "--helper-bin" => {
                    let val = iter
                        .next()
                        .ok_or_else(|| "--helper-bin requires a path argument".to_owned())?;
                    parsed.helper_bin = Some(PathBuf::from(val));
                }
                "--check-config" => {
                    parsed.check_config = true;
                }
                "-v" | "--version" => {
                    parsed.show_version = true;
                }
                "-h" | "--help" => {
                    parsed.show_help = true;
                }
                unknown => {
                    return Err(format!("Unknown option: {unknown}"));
                }
            }
        }
        Ok(parsed)
    }

    pub fn print_help() {
        println!(
            "TealDrive Server v{}\n\
             Usage: tealdrive-server [OPTIONS]\n\n\
             Options:\n\
               -c, --config <PATH>       Path to TOML or JSON configuration file\n\
               -b, --bind <ADDR>         Override server listen address (e.g. 127.0.0.1:3000)\n\
                   --static-dir <PATH>   Path to static frontend assets directory\n\
                   --assets-dir <PATH>   Alias for --static-dir\n\
                   --helper-bin <PATH>   Path to tealdrive-helper binary\n\
                   --check-config        Validate configuration file and exit\n\
               -v, --version             Print version information\n\
               -h, --help                Print this help message",
            env!("CARGO_PKG_VERSION")
        );
    }
}

#[tokio::main]
async fn main() {
    let args = match ServerArgs::parse(std::env::args()) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("Error: {err}\nRun 'tealdrive-server --help' for usage instructions.");
            std::process::exit(1);
        }
    };

    if args.show_help {
        ServerArgs::print_help();
        return;
    }

    if args.show_version {
        println!("tealdrive-server {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if let Err(error) = run(args).await {
        eprintln!("tealdrive-server failed: {error}");
        std::process::exit(1);
    }
}

async fn run(args: ServerArgs) -> Result<(), TealDriveError> {
    let mut config = AppConfig::load_or_default(args.config_path.as_deref())?;

    if let Some(bind_override) = args.bind_addr {
        config.server.bind_addr = bind_override;
    }
    if let Some(static_dir) = args.static_dir {
        config.server.static_dir = Some(static_dir);
    }

    config.validate()?;

    if args.check_config {
        println!("Configuration is valid.");
        return Ok(());
    }

    let bind_addr = config
        .server
        .bind_addr
        .parse::<SocketAddr>()
        .map_err(|_| TealDriveError::Internal)?;

    let helper_path = args.helper_bin.unwrap_or_else(default_helper_binary_path);
    let helper_client = HelperProcessClient::new(
        helper_path,
        Duration::from_secs(config.helper.timeout_seconds),
    );
    let helper_client_clone = helper_client.clone();
    let audit_sink = JsonlAuditSink::open_default().map_err(|_| TealDriveError::Internal)?;
    let session_store = InMemorySessionStore::new(config.session.clone());
    let rate_limiter = tealdrive::rate_limit::login::InMemoryLoginRateLimiter::with_lockout(
        config.rate_limit.max_login_failures,
        config.rate_limit.lockout_duration_seconds,
    );
    let state = WsServerState::with_authenticator_and_path(
        config,
        args.config_path,
        session_store,
        helper_client,
        audit_sink,
        std::sync::Arc::new(tealdrive::auth::pam::PamAuthenticator::new()),
        rate_limiter,
    )
    .with_timeout_updater(move |d| helper_client_clone.set_timeout(d));

    let _watcher = tealdrive::ws::server::spawn_config_watcher(&state);

    #[cfg(unix)]
    {
        let config_clone = state.config.clone();
        let audit_sink_clone = state.audit_sink.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            if let Ok(mut sighup) = signal(SignalKind::hangup()) {
                while sighup.recv().await.is_some() {
                    eprintln!("Received SIGHUP, reloading configuration...");
                    match config_clone.reload() {
                        Ok(_) => {
                            let _ = audit_sink_clone.emit(&tealdrive::audit::event::AuditEvent::config_reload(
                                tealdrive::protocol::frame::RequestId::new(),
                                "sighup",
                                0,
                                0,
                                tealdrive::audit::event::AuditStatus::Success,
                            ));
                            eprintln!("Configuration reloaded successfully via SIGHUP.");
                        }
                        Err(err) => {
                            let mut ev = tealdrive::audit::event::AuditEvent::config_reload(
                                tealdrive::protocol::frame::RequestId::new(),
                                "sighup",
                                0,
                                0,
                                tealdrive::audit::event::AuditStatus::Failed,
                            );
                            ev.reason = Some(tealdrive::audit::event::AuditReason::InvalidConfig);
                            let _ = audit_sink_clone.emit(&ev);
                            eprintln!("Failed to reload configuration on SIGHUP: {err}");
                        }
                    }
                }
            }
        });
    }

    run_server(bind_addr, state).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_all_flags() {
        let args = vec![
            "tealdrive-server".to_owned(),
            "-c".to_owned(),
            "/etc/tealdrive.toml".to_owned(),
            "-b".to_owned(),
            "0.0.0.0:8080".to_owned(),
            "--static-dir".to_owned(),
            "/var/www/tealdrive".to_owned(),
            "--helper-bin".to_owned(),
            "/usr/lib/tealdrive/helper".to_owned(),
            "--check-config".to_owned(),
        ];

        let parsed = ServerArgs::parse(args).expect("parsed");
        assert_eq!(
            parsed.config_path,
            Some(PathBuf::from("/etc/tealdrive.toml"))
        );
        assert_eq!(parsed.bind_addr.as_deref(), Some("0.0.0.0:8080"));
        assert_eq!(
            parsed.static_dir,
            Some(PathBuf::from("/var/www/tealdrive"))
        );
        assert_eq!(
            parsed.helper_bin,
            Some(PathBuf::from("/usr/lib/tealdrive/helper"))
        );
        assert!(parsed.check_config);
        assert!(!parsed.show_help);
        assert!(!parsed.show_version);
    }

    #[test]
    fn test_parse_args_help_and_version() {
        let help_args = vec!["tealdrive-server".to_owned(), "-h".to_owned()];
        let parsed = ServerArgs::parse(help_args).expect("parsed");
        assert!(parsed.show_help);

        let ver_args = vec!["tealdrive-server".to_owned(), "--version".to_owned()];
        let parsed_ver = ServerArgs::parse(ver_args).expect("parsed");
        assert!(parsed_ver.show_version);
    }

    #[test]
    fn test_parse_args_missing_value_errors() {
        let args = vec!["tealdrive-server".to_owned(), "--config".to_owned()];
        assert!(ServerArgs::parse(args).is_err());
    }

    #[test]
    fn test_parse_args_unknown_option_errors() {
        let args = vec!["tealdrive-server".to_owned(), "--foo".to_owned()];
        assert!(ServerArgs::parse(args).is_err());
    }

    #[test]
    fn test_parse_args_assets_dir_alias() {
        let args = vec![
            "tealdrive-server".to_owned(),
            "--assets-dir".to_owned(),
            "/srv/frontend".to_owned(),
        ];
        let parsed = ServerArgs::parse(args).expect("parsed");
        assert_eq!(parsed.static_dir, Some(PathBuf::from("/srv/frontend")));
    }
}
