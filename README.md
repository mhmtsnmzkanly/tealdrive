# TealDrive

A Linux file manager with PAM login, a Rust/Axum server, an embedded browser UI,
a binary WebSocket protocol and a separate filesystem helper.

The network server runs as an unprivileged service account. In multi-user mode,
the root-owned setuid helper authenticates users, verifies signed identity tickets,
drops supplementary groups and UID/GID to the authenticated account, then confines
filesystem access with Landlock. Unix permissions still determine file access.
The server does not open managed user files itself.

## Build and verify

Requirements: Linux with Landlock ABI 3+ (normally kernel 6.2+), `openat2`,
Rust 1.85+ for the locked dependencies, and Node.js for browser contract tests.
PAM requires the host's `libpam.so.0` and `/etc/pam.d/tealdrive` at runtime.
The declared Rust minimum has not been tested with a separate 1.85 toolchain.

```sh
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
./target/release/tealdrive-server --config deployment/config.toml --check-config
```

See [Caddy deployment and acceptance tests](docs/DEPLOYMENT.md) before starting
multi-user mode. Example files are under [deployment/](deployment/).

## Current behavior

- PAM login, session cookies, CSRF-protected logout and administrator changes.
- Directory pagination, search, text viewing/editing, folders, rename, move/copy,
  trash/restore, optional permanent deletion and chmod, ZIP/TAR.GZ and extraction.
- Bounded, acknowledged uploads; HTTP streaming downloads with single byte ranges.
- Revocation of active WebSockets/downloads when sessions expire or are removed.
- Immutable per-operation policies restricted by a root-owned helper ceiling.
- Root quota reservations shared across helper processes, staged publication of
  growing outputs, and rollback of failed operations.
- Browser constants generated from Rust; interoperability tests run the shipped
  JavaScript codec against real Rust frames.

Defaults deny administrator access and optional destructive features. Configuration
must explicitly name administrators. Uploads are intentionally bounded to 1 MiB
per file by default (configurable up to 3 MiB); this version is not a resumable,
unbounded upload service. Text editing defaults to 128 KiB. HTTP downloads support
larger files; the older WebSocket download path is bounded to 768 KiB.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Configuration](docs/CONFIGURATION.md)
- [HTTP API](docs/API.md)
- [Binary protocol](docs/PROTOCOL.md)
- [Caddy deployment](docs/DEPLOYMENT.md)
- [Implementation and verification report](docs/STATUS.md)

Deployment readiness is limited by the verification described in the report:
real PAM credentials, setuid identity switching and the Caddy/browser acceptance
path must be exercised on the target host.
