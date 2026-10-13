> Yeni entegrasyon için [public SDK](intgr/SDK.md), [tipler](../src/ui/sdk/client.d.ts) ve [örnekler](../sdk-api.js) kullanılır. Aşağıdaki HTTP tablosunun PAM login kısmı legacy uyumluluktur. Web enrollment/password/login `/ws/auth` ve one-use cookie bridge üzerinden, file/admin işlemleri SDK üzerinden çağrılır. `/api/audit`, `/api/undo`, `/api/undo/preferences` protected own-account kapsamındadır; ayrıntılar intgr/AUDIT.md ve UNDO.md içindedir.

# HTTP API

All routes are served by the backend with direct TLS or an optional TLS proxy. Authentication is an HttpOnly
session cookie (`tealdrive_session` by default); it is Secure and SameSite=Strict
by default. Use HTTPS for those defaults. JSON errors and HTTP status describe
failures; do not infer success from a completed fetch alone.

| Method | Route | Behavior |
|---|---|---|
| GET | `/api/health` | Liveness response; does not check PAM or helper readiness |
| POST | `/api/login` | Legacy PAM mode only; web-account mode uses SDK WS authentication |
| GET | `/api/session` | Authentication state, identity, `is_admin`, stable CSRF token and roots |
| POST | `/api/logout` | Remove session and expire cookie; requires CSRF when authenticated |
| GET | `/api/download?root_id=home&path=file.txt` | Authenticated attachment stream; single HTTP Range supported |
| GET | `/api/preview?root_id=home&path=image.png` | Bounded preview; active content is an attachment with sandbox CSP |
| GET | `/api/admin/config` | Administrator-only current server configuration |
| PATCH | `/api/admin/config?save=true` | Administrator + CSRF; validated partial patch, optional persistence |
| POST | `/api/admin/config/reload` | Administrator + CSRF; reload selected config file |
| GET | `/ws` | Session-bound upgrade; exact-origin policy; binary TDRV protocol |

Login accepts `{"username":"alice","password":"..."}` and returns username,
UID/GID, `csrf_token` and configured root summaries. Session responses include
`authenticated`, `is_admin`, nullable identity fields and the same token. Root
`used_bytes` is currently null; do not display it as zero. Root summaries describe
server settings; the helper ceiling and Unix permissions may further restrict them.
An empty administrator list grants nobody administrative rights, and names are
case-sensitive.

Send the session's `X-CSRF-Token` on logout, configuration PATCH and reload.
Missing or incorrect tokens return 403. Read-only GET routes do not require it.
Login failure returns a generic 401; username/IP lockout returns 429.

A patch such as `{"features":{"save_text_file":true}}` changes only that field.
Unknown configuration fields are rejected. `save=true` requires an existing selected
configuration path with a writable parent directory; disk errors return 500 and
leave the live configuration unchanged. Invalid values return 400. Bind address
changes require restart; the PATCH schema does not expose the listener setting.

Downloads return 200 or 206 with `Content-Length`, `Accept-Ranges` and appropriate
`Content-Disposition`. Invalid ranges return 416; unauthenticated reads return 401.
Paths must be root-relative; traversal, symlinks, forbidden hidden components and
sensitive files are rejected. Preview size is independent of the text-edit limit.
An error after streaming starts terminates the body rather than rewriting its status.

The browser UI is embedded at `/`, `/index.html`, `/style.css`, `/app.js` and
`/favicon.svg`. Unknown non-API paths return the SPA; unknown `/api/*` paths return
404. An optional static directory overrides those assets and must provide its own
compatible application bundle. The shipped bundle includes generated protocol
constants; an override is not automatically rewritten.


## Audit correlation and proxy settings

API responses expose `X-Tealdrive-Audit-Write-Failures`, the number of audit write
failures since process start. Successful health polling is not audited.
Administrators can PATCH `proxy.trusted_proxies` (IP/CIDR strings) and
`proxy.client_ip_header` (`XForwardedFor`, `XRealIp`, `CfConnectingIp`), with normal
session/CSRF checks. Invalid proxy ranges reject the configuration transaction.
See CONFIGURATION.md and AUDIT.md for trust rules, event phases and limitations.


## Login access

PATCH `/api/admin/config` accepts `allowed_users: string[]`, stored as
`account_policy.allowed_users`. GET returns the list under `account_policy`.
Only administrator sessions with valid CSRF may change it. An empty list disables
all login access. The helper's root-owned list remains an additional mandatory
restriction. Unlisted users receive HTTP 401 `INVALID_CREDENTIALS` even with a
correct password. Existing sessions cannot make further requests after removal.


## Local user choices

GET `/api/admin/users` requires an administrator session and returns
`{"users":[{"username":"alice","uid":1000,"gid":1000}],"truncated":false}`.
It includes eligible local accounts only, excludes system/service accounts and
returns `Cache-Control: no-store`. Discovery failure returns HTTP 503 with
`USER_DISCOVERY_UNAVAILABLE`; unauthenticated/non-admin requests return 401/403.
Selecting a choice does not create an OS account or widen the helper allowlist.
