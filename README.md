# TealDrive

A high-performance, security-hardened, self-hosted web drive and file management server written in asynchronous Rust.

TealDrive combines a modern Single Page Application (SPA) frontend, an unprivileged HTTP/WebSocket network daemon, and a privileged, isolated companion helper daemon to deliver enterprise-grade file access with POSIX fidelity and defense-in-depth isolation.

---

## Key Features

- **Privilege Separation Architecture:** Network handling and protocol parsing run in an unprivileged user space (`tealdrive-server`), while actual filesystem mutations are performed by a strictly validated privileged helper (`tealdrive-helper`).
- **TealDrive V1 Binary WebSocket Protocol:** High-throughput multiplexed frame protocol over WebSockets with 32-byte binary headers, MessagePack serialization, structured error frames, and streaming transfers.
- **Integrated Web UI:** Embedded modern Single Page Application (SPA) with responsive directory navigation, media lightbox preview, file uploads with resume support, search, and admin dashboards.
- **Zero-Downtime Dynamic Configuration:** Runtime hot-reloading with filesystem watcher (`notify`), thread-safe lock-free configuration swapping (`arc-swap`), and an administrative REST API.
- **Granular Policy Engine:** Root-based path containment, multi-user trash isolation, quota enforcement, sensitive file filtering, and hidden file access gates.
- **Security Hardened by Default:**
  - Strict protection against path traversal and symlink escapes (canonicalization verification).
  - SUID/SGID bit stripping on permission modifications (`chmod` masked to `0o0777`).
  - Strict Content Security Policy (CSP) sandbox headers on previews and forced attachment download for dangerous web assets (`.html`, `.svg`, `.xml`).
  - Cumulative archive extraction limit (500 MB) preventing Zip/Tar decompression bombs.
  - Rate limiting on authentication to prevent brute-force attacks.
  - Multi-user trash separation (users can only access, restore, or delete their own items).
- **HTTP Streaming & Range Requests:** Resumable chunked file downloads with `Accept-Ranges: bytes` support and video/audio seek support.
- **Structured Audit Logging:** Asynchronous audit sink recording security events, authentication attempts, and file operations in JSON Lines format.

---

## Architecture Overview

```
                      +-----------------------------+
                      |   Web Browser / Client UI   |
                      +-----------------------------+
                                     |
                     HTTP REST / Streaming / WebSockets
                                     v
                      +-----------------------------+
                      |      tealdrive-server       |
                      |   (Unprivileged Daemon)     |
                      | - Axum Web Server & SPA     |
                      | - Session Auth & Policy     |
                      | - WebSocket V1 Protocol     |
                      | - Dynamic Hot-Reloading     |
                      +-----------------------------+
                                     |
                       Length-prefixed MsgPack IPC
                           (stdin/stdout / socket)
                                     v
                      +-----------------------------+
                      |      tealdrive-helper       |
                      |  (Privileged Helper Daemon) |
                      | - POSIX Filesystem Driver   |
                      | - Quota & Archive Engine    |
                      | - User-isolated Trash       |
                      | - Strict Canonicalization   |
                      +-----------------------------+
                                     |
                               Filesystem
```

For detailed architectural documentation, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Quickstart

### Prerequisites

- **Rust:** Rust 1.78 or newer (`cargo`, `rustc`).
- **Operating System:** Linux (recommended for full POSIX privilege separation) or Unix-compatible system.

### Building from Source

Clone the repository and build using Cargo:

```bash
git clone https://github.com/your-org/tealdrive.git
cd tealdrive

# Build release binaries
cargo build --release
```

This compiles two binaries in `target/release/`:
1. `tealdrive-server`: The primary unprivileged web daemon.
2. `tealdrive-helper`: The filesystem helper daemon.

### Setting Up Helper Privileges (Linux)

In a production deployment with multiple Linux users, `tealdrive-helper` runs with elevated permissions (e.g., `setuid root`) so it can impersonate authenticated users' POSIX UIDs/GIDs when writing files:

```bash
sudo chown root:root target/release/tealdrive-helper
sudo chmod 4755 target/release/tealdrive-helper
```

> **Note:** In single-user or development environments, `tealdrive-server` can execute `tealdrive-helper` directly without `setuid`.

### Running the Server

Start the server pointing to a configuration file:

```bash
./target/release/tealdrive-server -c config.toml
```

Or run with default development settings:

```bash
cargo run --bin tealdrive-server -- -b 127.0.0.1:3000
```

Access the Web UI by opening `http://127.0.0.1:3000` in your web browser.

---

## Command Line Usage

```
TealDrive Server v0.1.0
Usage: tealdrive-server [OPTIONS]

Options:
  -c, --config <PATH>       Path to TOML or JSON configuration file
  -b, --bind <ADDR>         Override server listen address (e.g. 127.0.0.1:3000)
      --static-dir <PATH>   Path to static frontend assets directory
      --assets-dir <PATH>   Alias for --static-dir
      --helper-bin <PATH>   Path to tealdrive-helper binary
      --check-config        Validate configuration file syntax and exit
  -v, --version             Print version information
  -h, --help                Print this help message
```

---

## Configuration Example

Below is a standard `config.toml` example:

```toml
[server]
listen_address = "0.0.0.0:3000"
helper_bin_path = "/usr/local/bin/tealdrive-helper"

[session]
cookie_name = "tdrv_session"
cookie_secure = false
cookie_same_site = "Lax"
idle_timeout_seconds = 86400
absolute_timeout_seconds = 604800

[rate_limit]
max_login_failures = 5
lockout_duration_seconds = 900

[account_policy]
admin_users = ["admin"]

[[roots]]
root_id = "storage"
base_path = "/var/tealdrive/storage"
read_only = false
uploads_allowed = true
hidden_files_allowed = false
quota_bytes = 10737418240 # 10 GiB

[features]
save_text_file = true
create_folder = true
rename = true
move_file = true
copy_file = true
delete_permanently = true
chown = false

[limits]
max_frame_payload = 1048576       # 1 MiB
max_in_flight_upload = 2097152    # 2 MiB
max_preview_size = 52428800       # 50 MiB
```

For full configuration specifications, see [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

---

## Testing & Quality Assurance

The codebase includes an exhaustive test suite with over 470 unit, integration, and end-to-end tests:

```bash
# Run all unit, integration, and E2E tests
cargo test --all-targets

# Run Clippy static analysis with all warnings denied
cargo clippy --all-targets -- -D warnings
```

---

## Documentation Index

- [Architecture Guide](docs/ARCHITECTURE.md) - Two-tier security model, IPC mechanisms, policy engine, and hot-reload.
- [Protocol Specification](docs/PROTOCOL.md) - TealDrive V1 Binary WebSocket frame layout, MessagePack formats, and handshake lifecycle.
- [REST & WebSocket API Reference](docs/API.md) - Authentication, streaming download, preview, admin endpoints, and WebSocket operations.
- [Configuration Reference](docs/CONFIGURATION.md) - Exhaustive reference for `config.toml`, runtime flags, and dynamic patches.

---

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
