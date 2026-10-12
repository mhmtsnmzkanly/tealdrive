# Configuration

Use [docs/deployment/config.toml](deployment/config.toml) and
[docs/deployment/helper.toml](deployment/helper.toml) as tested examples. TOML and JSON
are accepted. Omitted fields use defaults; unknown fields are rejected. Roots require
all fields shown in the examples except optional `quota_bytes`.

Lookup order: explicit `--config`, `TEALDRIVE_CONFIG`, `./tealdrive.toml`,
`./config.toml`, `/etc/tealdrive/config.toml`, `/etc/tealdrive/tealdrive.toml`, then
compiled defaults. An explicit missing/invalid file is an error. The selected path
is retained for reload, file watching and UI persistence.

```sh
tealdrive-server --config docs/deployment/config.toml --check-config
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
| `account_policy` | `reject_root=true`, `reject_system_accounts=true`, `reject_disabled_shells=true`, `minimum_regular_uid=1000`, `allowed_users=[]`, `admin_users=[]` |
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
retention_hours = 504
```

Defaults are 10 MiB, five archives and 504 hours (21 days). If `path` is omitted at startup,
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
mtime; optional retention accepts 1..876000 hours. Legacy `retention_days` remains readable
and takes precedence when explicitly configured. Patching one unit clears the other.
A JSON patch with `{"audit":{"retention_hours":null}}` disables the age limit while
preserving count retention. Use `retention_hours = 0` in TOML to disable the age limit. Disabled age retention
serializes as 0 in both formats, preserving the setting across restarts.

Cleanup runs when events are written (at most once a minute, and immediately after
rotation/config change). No activity means no age cleanup. Only numeric archives
for the currently selected filename are managed. Changing the path starts writing
at the new location; previous logs are left intact and are not automatically moved
or deleted. File watching, SIGHUP, admin reload and PATCH apply settings live.
Failed target opening or persistence leaves the live sink/config unchanged.

The audit remains synchronous. See AUDIT.md for current event coverage. These settings
control storage, not which file operations are audited. Do not additionally rotate
the same file with an external tool while application rotation is enabled.


## Trusted proxies and visitor IP

```toml
[proxy]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
client_ip_header = "XRealIp"
```

The default trust list is empty: the actual TCP peer is used. Supported header
selectors are `XForwardedFor`, `XRealIp`, and `CfConnectingIp`. Only an immediate
peer in the configured IP/CIDR list may supply the selected header. Unknown peers,
missing, malformed or duplicate single-value headers fall back to the TCP peer.
X-Forwarded-For is parsed from right to left, stopping at the first untrusted hop;
all tokens must be valid IPs. Limits: 128 trusted ranges, 32 forwarded hops and
8192 header bytes. IPv4 and IPv6 are supported. No DNS names are accepted.

Administrators may change this under Settings → Client IP / Trusted proxies or
PATCH `/api/admin/config` with `proxy`. Changes apply live and persist in TOML.
The IP is used consistently for audit records, sessions and login rate limiting.
`peer_ip` preserves the immediate TCP peer in request and command audit records.

For Cloudflare → Caddy → TealDrive, Caddy must trust only Cloudflare ranges and
resolve the visitor IP before overwriting the backend header:

```caddyfile
reverse_proxy 127.0.0.1:3000 {
    header_up X-Real-IP {client_ip}
}
```

Use Caddy's `trusted_proxies`, `trusted_proxies_strict`, and `client_ip_headers`
settings for the Cloudflare-facing hop. TealDrive trusts Caddy's loopback address,
not arbitrary Internet clients. Trusted local processes can supply this header
when connecting directly to the backend; keep the backend bound to loopback.
If directly behind Cloudflare, select `CfConnectingIp` and trust only its official
ranges. Keep those ranges current. Do not use `0.0.0.0/0` or `::/0` as trust lists.
See [Cloudflare HTTP headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)
and [Caddy reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).


## Login allowlist

Every network login requires an exact username in `account_policy.allowed_users`.
The default is an empty list and **denies all logins**. Existing installations must
add their intended users before upgrading both server and helper:

```toml
[account_policy]
allowed_users = ["alice"]
admin_users = ["alice"]
reject_root = true
reject_system_accounts = true
reject_disabled_shells = true
minimum_regular_uid = 1000
```

Set this list in the server TOML and the root-owned `/etc/tealdrive/helper.toml`.
Both lists must allow the user. Server administrators can narrow or change the
server list by checking eligible local users under Settings → Login access, or PATCH `/api/admin/config` with
`{"allowed_users":["alice"]}`. This does not widen the root-owned helper list.
Adding a user to `admin_users` alone does not grant login access. There are no
wildcards, implicit admin permissions, groups or UID-based allowlist expansion.
Lists contain at most 1024 nonempty usernames (256 bytes each); whitespace,
control characters and path/wildcard/separator characters are rejected.

Root, system-account UID and disabled-shell checks still apply to listed users.
Thus listing a service account does not override those restrictions. Do not
relax those existing account protections to make a service account eligible.
Unlisted accounts receive the same generic credential error as other failures;
audit records identify `AccountNotAllowed`. They are denied before PAM/helper
work; backend results are also checked against the latest server policy.

Removing a user blocks subsequent HTTP/WS commands immediately. Background
maintenance removes disallowed sessions within approximately one second; their
leases then cancel active work, and partial uploads are cleaned up. A
`SessionRevoked` audit event records the policy removal. The privileged helper
also checks its current root-owned allowlist when verifying cached identity
tickets, so removing a user there prevents their next helper operation even if
an old server session/ticket remains. Already completed operations are not undone.
An empty server list (including removing the last administrator) requires editing
the server config from the host to recover access.


### User picker and service filtering

Settings → Login access discovers local users from the bounded public
`/etc/passwd` and `/etc/shells` files; it does not read shadow/password data.
Only administrator sessions can request `/api/admin/users`. Responses are not
cached and contain username, UID and GID only, sorted by username.

New choices require a nonzero UID at or above `minimum_regular_uid`, a valid
username and a shell listed in `/etc/shells`; `false` and `nologin` are always
excluded even if listed there. Known service names such as caddy, www-data,
wwwdata, tealdrive, nginx, apache, database/Redis accounts and nobody are excluded
also when they have an unusually high UID and an interactive shell. If
`/etc/shells` is missing, common sh/bash/dash/zsh/fish paths are used as fallback.
At most 1024 choices are returned, with an explicit truncation flag.

This is a conservative local-account chooser, not proof that an account is a
human: custom service accounts can have ordinary UIDs, names and shells. Review
selections; PAM checks and both mandatory allowlists remain authoritative.
LDAP/NSS-only users and custom shells absent from `/etc/shells` are not discovered
by this local picker and can still be managed explicitly in the configurations.
The root-owned helper list must permit newly selected users as before.

Existing configured users outside the filtered choices remain visible as
remove-only entries. Saving other settings preserves them unless explicitly
unchecked; after removal they cannot be reselected through the picker. A failed
user discovery blocks settings saving rather than overwriting permissions with
an empty list.

## Root configuration and panel settings separation

Use split mode for new installations:

```sh
tealdrive-server --config /home/tealdrive/policy/server.toml \
  --runtime-config /home/tealdrive/config/settings.toml \
  --helper-bin /home/tealdrive/bin/tealdrive-helper
```

The first file is a complete `AppConfig` owned by root. Its parents must be root-owned,
not symlinks and not group/world writable. The service needs read access, but cannot
write this file or its parent. The runtime settings file is an optional partial
configuration in a service-owned directory; an absent file means no overrides.
Explicit invalid, oversized (>64 KiB), nonregular or final symlink settings fail loading.
An empty file is a valid empty overlay. CLI bind/static overrides are rejected in split mode.

```toml
# /home/tealdrive/config/settings.toml
quota_warning_percent = 80

[features]
save_text_file = true # only if permitted by root config and helper ceiling

[audit]
retention_hours = 504
max_file_bytes = 10485760
max_archives = 5

[session]
idle_timeout_seconds = 1800
```

Allowed overrides: quota warning, features, transfer/list/edit limits, audit size/count/age,
session idle/absolute timeouts, helper timeout, search limits and stricter login limiting.
Features and numeric limits cannot exceed root values. Root chooses the audit directory;
panel may change the filename within that exact directory. Retention cannot exceed a
root age ceiling or be disabled when root requires one. Panel cannot change accounts/admins,
roots, sensitive policies, cookie security/name, proxy trust, Origin/protocol or UI source.
Those fields are rejected even when their requested values equal current values.

`save=true` atomically persists only the runtime overlay (0600); root config is unchanged.
Both files are watched; SIGHUP and admin reload merge and validate them as one snapshot.
Invalid reload keeps current settings. A changed listening address requires restart and
is rejected by live reload. A tightened root ceiling conflicting with saved overrides
requires adjusting the overrides before reload. `--check-config` validates both files.
GET config includes `X-Tealdrive-Config-Mode: split`; the bundled UI disables critical
controls and submits only permitted settings. Legacy single-file mode remains available
for migration and retains its previous administrative surface; it is not the new
root/runtime protection model.
