# TealDrive

A Linux web file manager with a Rust/Axum server, a protected filesystem helper,
an embedded Turkish/English UI and a framework-independent browser SDK.

The server runs as an unprivileged `tealdrive` account. Root-only configuration,
account metadata and authority remain behind the helper. File operations run in a
fresh worker with permanent user UID/GID, no_new_privs and mandatory Landlock.
Normal Unix permissions still apply. Existing Linux accounts use their NSS home;
virtual accounts receive separate locked/nologin Linux UID/GID identities.

## Build and verify

Linux Landlock ABI3+/openat2, Rust1.85+, Node.js for SDK/browser tests and working
PAM/NSS are required. Mode000 chmod recovery needs Linux6.6+/fchmodat2. Rust1.85
compatibility, real PAM/setuid/kernel isolation and Chromium/TLS were tested.

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release --bins
```

Four binaries: server, helper, Wizard and ctl. Installation centers on
`/home/tealdrive`; Wizard creates trusted PATH shortcuts. See
[deployment](docs/DEPLOYMENT.md) before installing. Caddy is optional: direct TLS,
NGINX/Apache or another correctly configured TLS proxy can be used.

## Features

- PAM-first web enrollment, separate Argon2id web password, scoped grants and
  persistent login limits; suspend/logout/revoke invalidate access.
- Public `auth/files/admin` SDK with types, optional managed browser state,
  filters/sort/pagination, events/condition observers and transfer progress/cancel.
- File CRUD, revision-aware editing, interactive chmod, search, archives,
  trash/restore and policy-gated permanent deletion.
- Root/admin/personal access policies; protected own history and conditional undo
  for move/rename/trash/chmod with configurable user preference.
- Virtual tenant budgets and optional native Linux UID quotas, user-charged
  operation logs, bounded lifecycle cleanup and dedicated storage provisioning.
- Root-approved custom UI packages, embedded rescue UI, HTTPS/WSS and graceful
  shutdown notification preserving unsaved editor text.
- Offline install/update/repair/rollback/backup/restore/uninstall and separate ctl.

Uploads default to 1 GiB with a configurable 64 GiB hard ceiling; account quota,
staging budget and available disk space also apply. Uploads use bounded chunks and
can resume within the same session (30-minute default window); server restart does
not restore a pending upload. HTTP downloads stream with Range support. Text edits
default to 128 KiB. Root may create a fresh virtual global administrator with a
random temporary password and mandatory first password change. The service Linux
account remains locked/nologin and separate from every file identity.

Backend TLS supports live certificate refresh and Root-managed proxy trust renewal.
An optional standard Noise NK channel encrypts WebSocket application messages in
addition to TLS; REST and native HTTP downloads remain TLS-only. See the
[security scope](docs/intgr/TRANSPORT_SECURITY.md) and
[current delivery report](docs/intgr/FINAL_REPORT.md).

## Documentation

[Current component index](docs/intgr/README.md), [SDK guide](docs/intgr/SDK.md),
[API examples](sdk-api.js), [configuration](docs/intgr/CONFIGURATION.md),
[Wizard](docs/intgr/WIZARD.md), [architecture](docs/ARCHITECTURE.md),
[test evidence](docs/intgr/TEST_RESULTS.md) and [delivery report](docs/intgr/FINAL_REPORT.md).
Older design drafts are retained under `docs/intgr/history` with explicit labels.
