# Audit coverage

The synchronous JSONL sink records browser/server events, not arbitrary changes
made through SSH, other programs or the operating system. File contents, passwords,
cookies, CSRF tokens and raw request payloads are excluded. Session identifiers
are SHA-256 hashes. Paths and user-agent fields are bounded; archive source lists
retain the first 100 entries and the original source count.

## Events and outcomes

- Authentication: login success/failure, account rejection, rate limiting, logout,
  explicit revocation, allowlist rejection and expired sessions. Policy removal
  produces `SessionRevoked` / `AccountNotAllowed`, correlated by session hash. A once-per-second maintenance task
  removes expired sessions and emits one expiry event per removed session.
- WebSocket: admission/connection, protocol handshake, malformed/text messages,
  rejected commands and disconnects. Session and request correlation, verified
  visitor IP and TCP peer are attached where available.
- Every dispatched command: directory listing, metadata, text preview/save,
  search, trash listing, folder creation, rename, copy/move, upload, download,
  trash/restore/permanent deletion, chmod, compression and extraction. Records
  contain `operation`, root/path/target where applicable, `command_started`
  and an outcome. Actual helper failure kinds distinguish denial, disabled
  features, missing targets and other failures. Mode, transfer/trash identifiers,
  source counts and helper read/write byte counts are captured when available.
- Transfers: upload opening, accepted chunks, completion, client abort and
  disconnect/shutdown cleanup. Download preparation is distinct from successful
  transport delivery. HTTP streaming and WebSocket downloads record completion
  or interruption and transferred bytes.
- HTTP: API request outcomes include method, endpoint, HTTP status and request ID.
  CSRF/admin rejection, invalid sessions and shutdown admission rejection are
  visible. Successful health/static asset traffic is excluded; errors are retained.
  Endpoint query strings and bodies are not logged.
- Administration: configuration patches/reloads and graceful server shutdown.

`Started`, `Success`, `Denied`, `Failed` and `Cancelled` are outcome statuses.
`phase` distinguishes command execution from transport delivery; a command can
complete its filesystem work before the connection fails. Upload abort is a
cancellation, not a successful file publication. `bytes_in` on command records
counts encoded payload bytes; `helper_bytes_read/written` report helper counters.
HTTP `bytes_out` counts bytes yielded to the HTTP transport; WebSocket download
bytes count chunk data successfully submitted to the socket. Neither is a durable
acknowledgement from the receiving browser. Transfer preparation does not imply
that the visitor received the file.

## Operational limits

Ordinary operations continue if audit writing fails. Every sink error increments
an in-process counter and writes a diagnostic to stderr/systemd journal, including
request ID and action. The `X-Tealdrive-Audit-Write-Failures` response header exposes
the count; administrators see it in Settings → Audit Logs. The count resets on
server restart. Fix the storage problem and inspect the journal; missing events
cannot be reconstructed from this counter. Shutdown explicitly checks emit/fsync.

A crash, power loss or SIGKILL can leave a start record without an outcome, and
can lose buffered OS writes. Logs are not tamper-proof or an independent event
store. Retention/rotation intentionally remove old archives. Cleanup happens on
writes. See CONFIGURATION.md for storage and trusted proxy configuration.
