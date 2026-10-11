# TealDrive Configuration Reference

This guide provides a comprehensive reference for configuring TealDrive via `config.toml`, environment variables, CLI options, and runtime dynamic patches.

---

## 1. Overview & Loading Mechanisms

TealDrive is configured primarily using a TOML file. The configuration is parsed using Serde and can be hot-reloaded dynamically at runtime.

### Specifying the Configuration File
Start `tealdrive-server` with the `-c` or `--config` flag:

```bash
tealdrive-server -c /etc/tealdrive/config.toml
```

### Validating Configuration
You can check your configuration file for syntax errors or invalid paths before launching the server:

```bash
tealdrive-server -c /etc/tealdrive/config.toml --check-config
```

---

## 2. Complete `config.toml` Reference

Below is a complete, annotated configuration file displaying all available sections and default values.

```toml
# ==============================================================================
# SERVER CONFIGURATION
# ==============================================================================
[server]
# TCP address and port for the HTTP/WebSocket server.
# Can be overridden via the CLI flag: -b <ADDR>
listen_address = "0.0.0.0:3000"

# Optional path to static web frontend directory.
# If omitted or None, the embedded SPA frontend is served.
# Can be overridden via: --static-dir <PATH>
# static_dir = "/var/www/tealdrive-ui"

# Path to the companion filesystem helper binary.
# In production, this binary is typically owned by root with setuid (04755).
helper_bin_path = "/usr/local/bin/tealdrive-helper"


# ==============================================================================
# SESSION & AUTHENTICATION COOKIES
# ==============================================================================
[session]
# Name of the HTTP session cookie.
cookie_name = "tdrv_session"

# Set to true when serving behind HTTPS/TLS to enable the 'Secure' cookie attribute.
cookie_secure = false

# Cookie SameSite attribute: "Lax", "Strict", or "None".
cookie_same_site = "Lax"

# Inactivity timeout in seconds before a session expires (default: 24 hours).
idle_timeout_seconds = 86400

# Hard absolute session expiration in seconds regardless of activity (default: 7 days).
absolute_timeout_seconds = 604800


# ==============================================================================
# RATE LIMITING & BRUTE-FORCE PROTECTION
# ==============================================================================
[rate_limit]
# Maximum consecutive failed login attempts before an account is locked.
max_login_failures = 5

# Duration of temporary account lockout in seconds (default: 15 minutes).
lockout_duration_seconds = 900


# ==============================================================================
# HELPER IPC SETTINGS
# ==============================================================================
[helper]
# Timeout in seconds for IPC filesystem operations executed by the helper.
timeout_seconds = 30


# ==============================================================================
# SEARCH CONFIGURATION
# ==============================================================================
[search]
# Maximum directory recursion depth for safe file search.
max_depth = 20

# Default maximum number of results returned if unspecified by client.
default_max_results = 100

# Hard upper limit on maximum results returned in a single search query.
max_results_limit = 1000


# ==============================================================================
# ADMINISTRATIVE & ACCESS POLICIES
# ==============================================================================
[account_policy]
# Usernames allowed to access /api/admin/ endpoints and modify server settings.
# If empty, defaults to ["admin"].
admin_users = ["admin"]

# Optional number of days to retain files in trash before automated cleanup.
# Set to null or omit to disable automatic trash purging.
trash_retention_days = 30

# Allowed CORS origins for REST endpoints (empty allows all local/same-origin).
cors_allowed_origins = []


# ==============================================================================
# FEATURE FLAGS (OPERATION GATING)
# ==============================================================================
[features]
# Allow inline text file editing and saving via WebSocket.
save_text_file = true

# Allow creating new directories.
create_folder = true

# Allow renaming files and directories.
rename = true

# Allow moving files and directories.
move_file = true

# Allow copying files and directories.
copy_file = true

# Allow permanent deletion of files from trash.
delete_permanently = true

# Allow changing POSIX ownership (chown) via helper (typically false).
chown = false


# ==============================================================================
# PROTOCOL & RESOURCE LIMITS
# ==============================================================================
[limits]
# Maximum allowable size in bytes for a single WebSocket frame (1 MiB default).
max_frame_payload = 1048576

# Maximum size in bytes for MessagePack control payloads (256 KiB default).
max_control_payload = 262144

# Maximum decompressed payload size for compressed control messages (4 MiB).
max_decompressed_control_payload = 4194304

# Maximum size in bytes for an individual chunk transfer (256 KiB default).
max_chunk_size = 262144

# Maximum in-flight upload buffer size (aliases: max_upload_bytes) (1 MiB default).
max_in_flight_upload = 1048576

# Directory listing pagination limits.
max_directory_page_size = 500
default_directory_page_size = 200

# Maximum file size permitted for inline text editing (5 MiB default).
# (aliases: max_edit_text_bytes)
max_text_edit_size = 5242880

# Maximum file size permitted for inline media preview (50 MiB default).
max_preview_size = 52428800


# ==============================================================================
# SENSITIVE FILE PROTECTION POLICIES
# ==============================================================================
[sensitive_policy]
# File extensions blocked from text reading.
blocked_read_extensions = ["pem", "key", "id_rsa", "pfx", "p12"]

# File extensions blocked from creation/modification.
blocked_write_extensions = ["exe", "so", "dll", "dylib"]

# Relative path patterns blocked from any operation.
blocked_patterns = [
  "**/.*",
  "**/.git/**",
  "**/.env*",
  "**/id_rsa*",
  "**/shadow",
  "**/passwd"
]

# Patterns explicitly exempted from block rules.
allowed_patterns = []


# ==============================================================================
# STORAGE ROOTS (DIRECTORY JAILS)
# ==============================================================================
# Define one or more storage mounts accessible through TealDrive.

[[roots]]
# Unique root identifier referenced in API queries and WebSocket requests.
root_id = "storage"

# Absolute path on the host system. Must exist and be accessible.
base_path = "/var/tealdrive/storage"

# If true, modifications (upload, delete, rename, edit) are strictly forbidden.
read_only = false

# If false, file uploads to this root are rejected.
uploads_allowed = true

# If true, hidden files/directories (starting with '.') can be listed and accessed.
hidden_files_allowed = false

# If true, marks root as web-accessible; restricts executable file uploads.
is_web_root = false

# Optional storage quota in bytes (e.g., 10 GiB = 10737418240).
# Omit or set to null for unlimited storage.
quota_bytes = 10737418240

[[roots]]
root_id = "shared"
base_path = "/var/tealdrive/shared"
read_only = true
uploads_allowed = false
hidden_files_allowed = false
is_web_root = false
quota_bytes = null
```

---

## 3. Dynamic Runtime Configuration (Hot-Reloading)

TealDrive supports dynamic hot-reloading without service restarts:

### Automatic Filesystem Watcher
When `tealdrive-server` is started with `-c <PATH>`, it attaches a filesystem watcher (`notify`) to the configuration file. Whenever you edit and save `config.toml`, the server:
1. Re-parses and validates the updated file.
2. Atomically replaces the active in-memory configuration using `arc-swap`.
3. In-flight requests continue seamlessly with zero dropped packets or downtime.

### Admin REST API Patching
Parameters can also be updated via `PATCH /api/admin/config`:

```bash
curl -X PATCH http://127.0.0.1:3000/api/admin/config?save=true \
  -H "Cookie: tdrv_session=$SESSION_ID" \
  -H "Content-Type: application/json" \
  -d '{
    "features": {
      "delete_permanently": false
    },
    "rate_limit": {
      "max_login_failures": 8
    },
    "trash_retention_days": 60
  }'
```

Setting `save=true` automatically rewrites the persistent `config.toml` file with the updated values.

---

## 4. Production Deployment Checklist

1. **Reverse Proxy & HTTPS:**
   - Always terminate TLS using Nginx, Caddy, or an AWS ALB in production.
   - Set `session.cookie_secure = true`.
   - Configure WebSocket forwarding (`Upgrade: $http_upgrade`, `Connection: "Upgrade"`).
2. **Helper Privileges:**
   - Compile `tealdrive-helper` with `cargo build --release`.
   - Assign ownership and setuid:
     ```bash
     sudo chown root:root /usr/local/bin/tealdrive-helper
     sudo chmod 4755 /usr/local/bin/tealdrive-helper
     ```
3. **Restrict Storage Roots:**
   - Ensure `base_path` directories are dedicated to TealDrive data storage and do not overlap with critical system directories (`/etc`, `/root`, `/bin`).
4. **Admin Accounts:**
   - Restrict `account_policy.admin_users` to designated administrative logins.
