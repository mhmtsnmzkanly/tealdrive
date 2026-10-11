# TealDrive Architecture Guide

This document describes the architectural design, security model, privilege separation mechanisms, and runtime internals of TealDrive.

---

## 1. System Overview

TealDrive is built around a **two-tier defense-in-depth architecture**. The system is divided into two distinct operating processes:

1. **`tealdrive-server` (Unprivileged Web Daemon):**
   - Implemented using Tokio and Axum.
   - Handles public network listeners (HTTP, WebSocket), TLS termination, user authentication, session state management, and request routing.
   - Contains the frontend single-page application (SPA) and serves static web assets.
   - Evaluates the declarative security policy before delegating any filesystem operations.
   - Does **not** require root privileges and should run under a dedicated unprivileged system account (e.g., `tealdrive:tealdrive`).

2. **`tealdrive-helper` (Privileged Companion Helper):**
   - A standalone, minimal POSIX filesystem driver.
   - Executed either as a persistent worker or spawned on demand via IPC.
   - Runs with elevated privileges (e.g. `setuid root` or privileged capabilities) in multi-user deployments to switch to the authenticating user's UID/GID.
   - Independently re-validates all paths, root boundaries, symlink targets, permission bits, and quotas before touching the physical disk.

```
+-------------------------------------------------------------------------+
|                               Client UI                                 |
|                       (Browser / REST / WebSocket)                      |
+-------------------------------------------------------------------------+
                                     |
                                     | HTTP / WebSocket
                                     v
+-------------------------------------------------------------------------+
|                  UNPRIVILEGED TIER: tealdrive-server                    |
|                                                                         |
|  +--------------------+  +--------------------+  +--------------------+ |
|  |     Axum Router    |  |   Session Store    |  | Rate Limiter (Mem) | |
|  |  HTTP & WS /ws     |  | (Idle/Abs Expiry)  |  |  (Login Lockout)   | |
|  +--------------------+  +--------------------+  +--------------------+ |
|                                                                         |
|  +--------------------+  +--------------------+  +--------------------+ |
|  |  WebSocket Engine  |  |   Policy Engine    |  | Dynamic Hot-Reload | |
|  | (Frame Dispatcher) |  | (Roots/Sensitive)  |  |  (ArcSwap+Watcher) | |
|  +--------------------+  +--------------------+  +--------------------+ |
|                                     |                                   |
|                      HelperClient Trait Abstraction                     |
+-------------------------------------------------------------------------+
                                     |
                                     | Length-Prefixed MessagePack IPC
                                     | (stdin/stdout or Unix Domain Socket)
                                     v
+-------------------------------------------------------------------------+
|                   PRIVILEGED TIER: tealdrive-helper                     |
|                                                                         |
|  +--------------------+  +--------------------+  +--------------------+ |
|  | Path Resolution &  |  |   Quota Manager    |  | POSIX Filesystem   | |
|  | Symlink Jail Check |  | (Usage Accounting) |  |   (Read/Write)     | |
|  +--------------------+  +--------------------+  +--------------------+ |
|  +--------------------+  +--------------------+  +--------------------+ |
|  |  User-Isolated     |  |   Archive Engine   |  | SUID Bit Stripping | |
|  |   Trash System     |  | (500MB Zip Bomb)   |  |   (0o0777 Mask)    | |
|  +--------------------+  +--------------------+  +--------------------+ |
+-------------------------------------------------------------------------+
                                     |
                                     v
                       Host Filesystem / Storage Mounts
```

---

## 2. Two-Tier Privilege Separation Model

### Why Privilege Separation?

In traditional web drives, the web daemon runs either as `root` (risking total host compromise if a memory corruption or traversal vulnerability occurs in the HTTP parser) or as a single unprivileged user (making multi-user POSIX file ownership and permissions impossible).

TealDrive separates untrusted network input parsing from disk access:

| Concern | `tealdrive-server` | `tealdrive-helper` |
| :--- | :--- | :--- |
| **Network Access** | Binds to TCP sockets (`0.0.0.0:3000`) | No network sockets open |
| **User Privileges** | Unprivileged (`uid != 0`, e.g., `1001`) | Privileged (`setuid root` or capability-enabled) |
| **Execution Scope** | HTTP, WebSockets, Cookie Auth, SPA | Purely filesystem I/O |
| **Compromise Impact** | Attacker cannot escape allowed roots or execute arbitrary root commands | Constrained by rigid IPC command enum and path validators |

### Helper IPC Contract

Communication between `tealdrive-server` and `tealdrive-helper` occurs via a length-prefixed binary stream using MessagePack serialization:

1. **Wire Framing:**
   - 4 bytes: `u32` Big-Endian payload length.
   - N bytes: MessagePack serialized `HelperRequest` or `HelperResponse`.
2. **`HelperRequest` Model:**
   - `request_id`: UUIDv4 tracking request-response correlation.
   - `username`, `uid`, `gid`: The authenticated caller's identity.
   - `command`: Strictly typed enum (`ListDirectory`, `CreateFolder`, `MoveFile`, `CopyFile`, `MoveToTrash`, `RestoreFromTrash`, `ListTrash`, `DeletePermanently`, `ChmodFile`, `CompressZip`, `CompressTarGz`, `ExtractArchive`, `SearchFiles`, `SaveTextFile`, `ReadTextFile`).
   - `root_id`: The identifier of the configured root directory.
   - `relative_path`: Cleaned relative path within the root.
   - `target_relative_path`: Destination path for rename, move, and copy operations.
   - `limits`: Per-operation limits (e.g. maximum read size).
3. **`HelperResponse` Model:**
   - `request_id`: Matches request UUID.
   - `success`: Boolean indicating operation outcome.
   - `error_kind`: Standardized error classification (`NotFound`, `PermissionDenied`, `PathEscapesRoot`, `PolicyDenied`, `FileTooLarge`, etc.).
   - `payload`: Strongly typed operation result.

---

## 3. Defense-in-Depth Security Architecture

TealDrive implements multi-layered security controls to safeguard the host system:

### 3.1. Path Canonicalization & Jail Containment
All operations are restricted to explicitly configured roots (`AllowedRoot`).
- Path components are validated to forbid null bytes (`\0`), absolute path prefixes, and relative directory traversal (`..`).
- The base directory of each root is canonicalized at startup or config load.
- Before opening, moving, or deleting any file, the target path is canonicalized and verified using `starts_with(&root.canonical_base)`.
- If a symlink resolves to a target outside the root boundary, `TealDriveError::SymlinkEscape` is returned immediately.

### 3.2. Move & Copy Target Symlink Escape Defense
When moving or copying files to a destination path `target_rel`:
- The helper inspects the target's parent directory before invoking `fs::rename` or `fs::copy`.
- The parent directory must exist, must **not** be a symlink, and must canonicalize strictly within the root base.
- If the destination target already exists, overwriting is blocked unless explicitly permitted.

### 3.3. SUID & SGID Bit Stripping (`chmod`)
To prevent local privilege escalation when the helper runs with elevated privileges:
- Both `tealdrive-server` and `tealdrive-helper` inspect the requested mode bits.
- Any request with SetUID (`0o4000`), SetGID (`0o2000`), or Sticky (`0o1000`) bits set (`mode & 0o7000 != 0`) is rejected with `PolicyDenied`.
- The allowed permissions mask is strictly clamped to `0o0777` (standard read, write, execute permissions for user, group, and others).

### 3.4. Decompression Bomb & Archive Escape Defense
The archive extraction engine (`src/helper/archive.rs`):
- Enforces a cumulative extraction threshold: `MAX_ARCHIVE_EXTRACT_BYTES = 500 MB`.
- Uses bounded stream readers (`take()`) to avoid memory or disk exhaustion.
- Rejects any archive member containing parent traversal (`..`) or symlinks.
- Verifies that intermediate created directories are genuine directories and not symlinks pointing elsewhere.

### 3.5. Multi-User Trash Isolation
Deleted items are moved to `.tealdrive-trash/files/<uuid>` with metadata stored in `.tealdrive-trash/info/<uuid>.msgpack`:
- Metadata records the original file path, display name, deletion timestamp, and the owner's `uid`.
- When listing trash items (`ListTrash`), entries belonging to other UIDs are filtered out.
- When restoring (`RestoreFromTrash`) or deleting permanently (`DeletePermanently`), the helper asserts `entry.uid == request.uid || request.uid == 0`. Attempts to modify another user's deleted files return `PermissionDenied`.

### 3.6. Stored XSS & Preview CSP Sandboxing
When users request file previews via `/api/preview`:
- Dangerous active web content (`.html`, `.htm`, `.svg`, `.xml`, `.xhtml`) is forced to `Content-Disposition: attachment` to prevent arbitrary JavaScript execution in the origin context.
- Inline previewable files (images, audio, video, text) are served with a restrictive Content Security Policy:
  ```http
  Content-Security-Policy: sandbox; default-src 'none'; img-src 'self' data:; media-src 'self' data:; style-src 'unsafe-inline'
  ```

### 3.7. Storage Quotas
Each `AllowedRoot` can optionally specify a `quota_bytes` threshold:
- Upload chunk transfers track written bytes in real time.
- The helper verifies available quota before committing staged files to their final location.
- Operations that exceed quota return `TealDriveError::QuotaExceeded`.

---

## 4. Dynamic Runtime Hot-Reloading

TealDrive supports **zero-downtime dynamic configuration reload**:

```
+--------------------+            +------------------------+
|  config.toml Disk  |            |  Admin PATCH / Reload  |
+--------------------+            +------------------------+
          |                                   |
          | notify (FS Watcher)               | HTTP Handler
          v                                   v
+----------------------------------------------------------+
|                  Configuration Manager                   |
|                                                          |
| 1. Validate TOML syntax & Root paths                     |
| 2. Apply ConfigPatch to loaded AppConfig                 |
| 3. Atomic swap: ArcSwap<AppConfig>::store(new_config)    |
| 4. Update in-memory Helper, Search & RateLimit handles   |
+----------------------------------------------------------+
          |
          v
+----------------------------------------------------------+
|              Active Requests & Connections               |
|  - Read lock-free configuration via config.load()        |
|  - In-flight operations complete without disruption      |
|  - Future operations immediately observe new settings    |
+----------------------------------------------------------+
```

1. **Lock-Free Configuration Swapping:**
   - The central server state holds `Arc<ArcSwap<AppConfig>>`.
   - Incoming requests access the active configuration via `state.config.load()` in constant time without lock contention.
2. **Filesystem Watcher:**
   - A background thread monitors the active `config.toml` file via `notify::RecommendedWatcher`.
   - Disk modifications trigger automated validation and atomic configuration swapping.
3. **Admin Configuration API:**
   - Authenticated administrators (`admin_users`) can update settings via `PATCH /api/admin/config?save=true`.
   - Settings are dynamically updated in memory and atomically persisted to disk.

---

## 5. Audit Logging Architecture

All security-sensitive operations generate structured audit records:

- **Sink:** `JsonlAuditSink` writes asynchronous JSON Lines formatted events to the configured audit log.
- **Audited Events:**
  - `Authentication`: Login successes, login failures, logouts, session expirations.
  - `PolicyDecision`: Allowed or denied file access attempts, with specific denial reasons.
  - `FileOperations`: Create folder, rename, move, copy, upload, download, delete, chmod, archive, and extract.
- **Audit Record Schema:**
  ```json
  {
    "timestamp": 1728643200,
    "event_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "session_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d",
    "username": "alice",
    "uid": 1000,
    "action": "MoveFile",
    "root_id": "storage",
    "path": "documents/report.pdf",
    "target_path": "archive/report.pdf",
    "status": "success",
    "ip": "192.168.1.50"
  }
  ```
