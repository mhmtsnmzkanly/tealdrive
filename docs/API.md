# TealDrive HTTP REST & WebSocket API Reference

This document provides a comprehensive reference for the TealDrive HTTP REST API, media streaming endpoints, administrative management API, and WebSocket gateway.

---

## 1. Overview & Authentication

All HTTP and WebSocket requests in TealDrive are secured using session-based authentication:
- **Authentication Cookie:** `tdrv_session` (HTTP-only, configured with `SameSite=Lax` or `Strict`, and optional `Secure` flag for TLS).
- **CSRF Protection:** Modifying state requests support validation via the `X-CSRF-Token` header.
- **Rate Limiting:** Login endpoints enforce memory-backed lockout policies (`RateLimitConfig`) to prevent credential stuffing.

---

## 2. Authentication & Session Endpoints

### 2.1 Login
Authenticate user credentials and establish a new session.

- **Method:** `POST`
- **Path:** `/api/login`
- **Content-Type:** `application/json`

**Request Body:**
```json
{
  "username": "alice",
  "password": "correct_horse_battery_staple"
}
```

**Response (`200 OK`):**
- **Set-Cookie:** `tdrv_session=<session_id>; Path=/; HttpOnly; SameSite=Lax`
```json
{
  "username": "alice",
  "uid": 1000,
  "gid": 1000,
  "csrf_token": "a4f89d31-4821-4809-b684-297eb0981977",
  "expires_at": 1728729600
}
```

**Error Responses:**
- `401 Unauthorized`: Invalid username or password.
- `429 Too Many Requests`: Account temporarily locked due to consecutive failed attempts (`RateLimitConfig.max_login_failures`).

---

### 2.2 Logout
Terminate the active session and revoke the authentication cookie.

- **Method:** `POST`
- **Path:** `/api/logout`

**Response (`200 OK`):**
- **Set-Cookie:** `tdrv_session=; Path=/; Max-Age=0; HttpOnly`
```json
{
  "success": true
}
```

---

### 2.3 Get Current Session
Inspect active user session details and privileges.

- **Method:** `GET`
- **Path:** `/api/session`

**Response (`200 OK`):**
```json
{
  "username": "alice",
  "uid": 1000,
  "gid": 1000,
  "is_admin": false,
  "idle_expires_at": 1728646800,
  "absolute_expires_at": 1729248000
}
```

**Error Responses:**
- `401 Unauthorized`: No active session or expired cookie.

---

## 3. File Streaming & Preview Endpoints

### 3.1 Streaming File Download
Download a file with HTTP streaming and full partial content (`Range`) support.

- **Method:** `GET`
- **Path:** `/api/download`
- **Query Parameters:**
  - `root_id` (string, required): Identifier of the allowed root (e.g. `storage`).
  - `path` (string, required): Relative path to the target file.

**Headers Supported:**
- `Range: bytes=0-1048575`: Requests partial content (e.g. audio/video seeking or resumable downloads).

**Response (`200 OK` or `206 Partial Content`):**
- **Headers:**
  - `Content-Disposition: attachment; filename="dataset.csv"`
  - `Content-Type: application/octet-stream` (or inferred MIME type)
  - `Accept-Ranges: bytes`
  - `Content-Length: <bytes>`
  - `Content-Range: bytes <start>-<end>/<total>` (on 206 responses)

**Error Responses:**
- `401 Unauthorized`: Session missing or expired.
- `403 Forbidden`: Path denied by security policy, read-only root, or symlink escape.
- `404 Not Found`: File does not exist.

---

### 3.2 Safe File Preview
Stream inline media (images, audio, video, text) for in-browser rendering with security sandboxing.

- **Method:** `GET`
- **Path:** `/api/preview`
- **Query Parameters:**
  - `root_id` (string, required): Identifier of the allowed root.
  - `path` (string, required): Relative path to the preview target.
  - `preview` (boolean, optional): Set to `true` to request inline presentation.

**Security Enforcements:**
1. **Stored XSS Prevention:** Files with extensions `.html`, `.htm`, `.svg`, `.xml`, `.xhtml` are **forced** to `Content-Disposition: attachment` regardless of the `preview` parameter.
2. **CSP Sandbox:** All inline previews include a strict Content Security Policy:
   ```http
   Content-Security-Policy: sandbox; default-src 'none'; img-src 'self' data:; media-src 'self' data:; style-src 'unsafe-inline'
   ```

**Response (`200 OK`):**
- **Headers:**
  - `Content-Disposition: inline; filename="photo.jpg"`
  - `Content-Type: image/jpeg`
  - `Content-Security-Policy: sandbox; ...`

---

## 4. Administrative Configuration API

All endpoints under `/api/admin/` require that the caller's username is present in `account_policy.admin_users` (defaults to `["admin"]`). Non-admin users receive `403 Forbidden`.

### 4.1 Get Active Configuration
Retrieve the server's live running configuration.

- **Method:** `GET`
- **Path:** `/api/admin/config`

**Response (`200 OK`):**
```json
{
  "server": {
    "listen_address": "0.0.0.0:3000",
    "helper_bin_path": "/usr/local/bin/tealdrive-helper"
  },
  "session": {
    "cookie_name": "tdrv_session",
    "cookie_secure": false,
    "cookie_same_site": "Lax",
    "idle_timeout_seconds": 86400,
    "absolute_timeout_seconds": 604800
  },
  "rate_limit": {
    "max_login_failures": 5,
    "lockout_duration_seconds": 900
  },
  "limits": {
    "max_frame_payload": 1048576,
    "max_control_payload": 262144,
    "max_in_flight_upload": 2097152,
    "max_preview_size": 52428800
  },
  "features": {
    "save_text_file": true,
    "create_folder": true,
    "rename": true,
    "move_file": true,
    "copy_file": true,
    "delete_permanently": true,
    "chown": false
  },
  "account_policy": {
    "admin_users": ["admin"]
  },
  "trash_retention_days": 30
}
```

---

### 4.2 Dynamic Configuration Patch (Zero-Downtime Hot-Reload)
Dynamically update server parameters in memory with optional persistence to disk.

- **Method:** `PATCH`
- **Path:** `/api/admin/config`
- **Query Parameters:**
  - `save` (boolean, optional, default: `false`): If `true`, atomically writes updated configuration to the active `config.toml` file.
- **Content-Type:** `application/json`

**Request Body (`ConfigPatch`):**
```json
{
  "features": {
    "save_text_file": true,
    "delete_permanently": false
  },
  "trash_retention_days": 45,
  "rate_limit": {
    "max_login_failures": 10,
    "lockout_duration_seconds": 600
  },
  "limits": {
    "max_preview_size": 104857600
  }
}
```

**Response (`200 OK`):**
```json
{
  "status": "applied",
  "saved_to_disk": true
}
```

**Error Responses:**
- `401 Unauthorized`: Not logged in.
- `403 Forbidden`: User is not in `admin_users`.
- `400 Bad Request`: Invalid patch parameters.

---

### 4.3 Reload Configuration from Disk
Force an immediate reload and revalidation of the configuration file on disk.

- **Method:** `POST`
- **Path:** `/api/admin/config/reload`

**Response (`200 OK`):**
```json
{
  "status": "reloaded"
}
```

---

## 5. WebSocket Gateway (`/ws`)

- **Method:** `GET`
- **Path:** `/ws`
- **Headers:**
  - `Upgrade: websocket`
  - `Connection: Upgrade`
  - `Cookie: tdrv_session=<session_id>`

Upgrades standard HTTP connection to the bidirectional TealDrive V1 Binary Protocol. Detailed frame specs and message sequences are documented in [docs/PROTOCOL.md](PROTOCOL.md).

---

## 6. Frontend SPA & Static Asset Serving

- `GET /`: Serves the embedded single-page application `index.html`.
- `GET /assets/*`: Serves embedded static CSS, JS, fonts, and images.
- Unmatched paths fall back to `index.html` to support client-side browser routing.
