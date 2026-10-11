# Configuration

Use [deployment/config.toml](../deployment/config.toml) and
[deployment/helper.toml](../deployment/helper.toml) as tested examples. TOML and JSON
are accepted. Omitted fields use defaults; unknown fields are rejected. Roots require
all fields shown in the examples except optional `quota_bytes`.

Lookup order: explicit `--config`, `TEALDRIVE_CONFIG`, `./tealdrive.toml`,
`./config.toml`, `/etc/tealdrive/config.toml`, `/etc/tealdrive/tealdrive.toml`, then
compiled defaults. An explicit missing/invalid file is an error. The selected path
is retained for reload, file watching and UI persistence.

```sh
tealdrive-server --config deployment/config.toml --check-config
tealdrive-server --config /var/lib/tealdrive/config.toml \
  --helper-bin /usr/local/libexec/tealdrive-helper
```

CLI options: `--config/-c`, `--bind/-b`, `--static-dir` (`--assets-dir` alias),
`--helper-bin`, `--check-config`, `--version/-v`, `--help/-h`. CLI overrides are
initial startup overrides; a subsequent file reload replaces them with file values.

## Sections and defaults

| Section | Fields / defaults |
|---|---|
| `server` | `bind_addr="127.0.0.1:3000"`, `behind_tls_proxy=true`, optional `static_dir` |
| `session` | `cookie_name="tealdrive_session"`, idle 1800 s, absolute 43200 s, `secure_cookies=true`, `same_site="Strict"` (`Lax` also allowed) |
| `security` | `production_wire_protocol="Tdrv1Binary"`, `allow_plain_ws_localhost_only=true`, `allowed_origins=[]` |
| `account_policy` | `reject_root=true`, `reject_system_accounts=true`, `reject_disabled_shells=true`, `minimum_regular_uid=1000`, `admin_users=[]` |
| `features` | `save_text_file`, `chmod_file`, `extract_archive`, `delete_permanently`, `chown`: all false |
| `rate_limit` | `max_login_failures=5`, `lockout_duration_seconds=300` |
| `helper` | `timeout_seconds=10` |
| `search` | `max_depth=20`, `default_max_results=100`, `max_results_limit=1000` |
| top-level | optional `trash_retention_days` (metadata setting; no scheduled automatic purge) |

Verify exact defaults in `src/config.rs` when customizing. `chown` is reserved and
unsupported in this version. There are no global `upload`, `download`, `rename` or
`trash` feature toggles: uploads are controlled per root, and writes by read-only
roots, Unix permissions and file policies.

Each `[[roots]]` has `root_id`, absolute `base_path`, `read_only`, `uploads_allowed`,
`hidden_files_allowed`, `is_web_root` and optional `quota_bytes`. IDs must be unique,
nonempty ASCII alphanumeric/underscore/hyphen. Paths cannot contain `..`. Validation
checks syntax, not mount availability or user permission. Defaults expose `/home`
as root id `home`; deploy an explicit, dedicated data root instead. Hidden policy
checks every path component, with internal trash handling exceptions.

Origins must be exact `https://host[:port]` or `http://host[:port]` values; wildcards
and path/query values are rejected. When no allow-list is set, browser Origin is
compared with the request Host and the configured HTTP/HTTPS scheme. Native clients
without Origin still require a session. Behind Caddy bind only to loopback; the
backend trusts proxy headers and is not intended for direct public exposure.

## Limits

Values are bytes or counts, not MiB strings.

| `limits` field | Default | Constraint |
|---|---:|---|
| `max_frame_payload` | 1048576 | 1..1048576 |
| `max_control_payload` | 262144 | 1..frame limit |
| `max_decompressed_control_payload` | 4194304 | decompression bound |
| `max_chunk_size` | 262144 | <=262144; chunk + 32-byte subheader must fit frame |
| `max_in_flight_upload` | 1048576 | 1..3145728; currently also whole-file upload limit |
| `max_directory_page_size` | 500 | <=500 |
| `default_directory_page_size` | 200 | <= maximum |
| `max_text_edit_size` | 131072 | <= half control payload limit |
| `max_preview_size` | 52428800 | preview-only size bound |

`max_upload_bytes` and `max_edit_text_bytes` are accepted aliases on a complete
configuration, but use canonical field names in partial patches. Configure a smaller
text limit if filenames/policy metadata need unusually large payloads. Directory
and search results can still hit control/IPC limits when metadata is unusually large.

## File policies and helper ceiling

`sensitive_policy.deny_edit_patterns` and `warn_read_patterns` use case-insensitive
filename patterns (exact names, `*.suffix` and `prefix.*`). Defaults include `.env`,
SSH keys, `*.pem`, `*.key`, SQL and dump files. Current managed-file reads deny
sensitive matches; the names of the lists do not imply an interactive warning UI.
`webroot_policy.block_executable_uploads=true` and `blocked_extensions` default to
PHP/CGI/script extensions. Web-root extraction is refused.

Elevated helpers always read `/etc/tealdrive/helper.toml`; server configuration is
only a requested policy beneath that ceiling. Both files must name the same root
ids and paths. To enable a feature, allow it in both files. To change roots or relax
a ceiling, an OS administrator must edit the root-owned helper file. A server PATCH
cannot update that file. Root-owned key/config/parent permissions are checked.

Runtime updates: admin PATCH/reload, SIGHUP and parent-directory watching. Nested
patches preserve omitted values; explicit JSON null clears `static_dir` or
`trash_retention_days`. Persistence is atomic with mode 0600. Keep the server file
in a service-owned directory, not the root-owned helper directory. A new session
policy applies to existing session checks, but already-issued helper tickets retain
their original expiry; login again if the trusted ticket lifetime was changed.

Environment: `TEALDRIVE_CONFIG`, `TEALDRIVE_AUDIT_PATH`; helper binary lookup also
accepts `TEALDRIVE_HELPER_PATH`. `TEALDRIVE_HELPER_ROOTS` is a non-elevated test utility
and is ignored by the setuid boundary. Helper subprocesses clear inherited environment.

## Audit storage and retention

The administrator settings UI exposes log path, maximum active-file size, archive
count and archive age. The server config accepts:

```toml
[audit]
path = "/var/lib/tealdrive/audit.jsonl"
max_file_bytes = 10485760
max_archives = 5
retention_days = 30
```

Defaults are 10 MiB, five archives and 30 days. If `path` is omitted at startup,
`TEALDRIVE_AUDIT_PATH` is used, then `/tmp/tealdrive-audit.jsonl`. An explicit config
path overrides the environment. A selected path must be absolute, use an existing
writable parent and refer to a regular file; final-component symlinks are rejected.
New log files use mode 0600. The administrator config file itself cannot be selected
as the audit target.

Rotation happens before an event would exceed `max_file_bytes`: `audit.jsonl.1`
is newest, then `.2`, etc. The active file is reopened internally; copytruncate or
service restart is not required. Records are never split; a single oversized event
can exceed the bound. Sizes can range from 1 KiB to 1 GiB; UI labels use MiB.
`max_archives` allows 0..1000 (zero discards rotated logs). Age is based on archive
mtime; optional retention accepts 1..36500 days. A JSON patch with
`{"audit":{"retention_days":null}}` disables the age limit while preserving count
retention. Use `retention_days = 0` in TOML to disable the age limit. Disabled age retention
serializes as 0 in both formats, preserving the setting across restarts.

Cleanup runs when events are written (at most once a minute, and immediately after
rotation/config change). No activity means no age cleanup. Only numeric archives
for the currently selected filename are managed. Changing the path starts writing
at the new location; previous logs are left intact and are not automatically moved
or deleted. File watching, SIGHUP, admin reload and PATCH apply settings live.
Failed target opening or persistence leaves the live sink/config unchanged.

The audit remains synchronous and its event coverage is unchanged. These settings
control storage, not which file operations are audited. Do not additionally rotate
the same file with an external tool while application rotation is enabled.
