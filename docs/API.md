# HTTP API

All routes are served by the backend behind Caddy. Authentication is an HttpOnly
session cookie (`tealdrive_session` by default); it is Secure and SameSite=Strict
by default. Use HTTPS for those defaults. JSON errors and HTTP status describe
failures; do not infer success from a completed fetch alone.

| Method | Route | Behavior |
|---|---|---|
| GET | `/api/health` | Liveness response; does not check PAM or helper readiness |
| POST | `/api/login` | JSON `username`, `password`; PAM authentication; sets cookie |
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
