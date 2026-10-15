# TealDrive Comprehensive Security Audit & CVE Report

**Date**: 2026-10-15  
**Target Repository**: `tealdrive` (commit `d392eb1`)  
**Auditor**: Antigravity Automated Security Reasoning Engine  
**Classification**: Defensive Source Code Security Assessment  

---

## 1. Executive Summary & Audit Scope

A comprehensive, fine-grained security assessment was conducted across the entire TealDrive source repository, including:
- **Rust Backend Server & Control Plane**: HTTP/WebSocket routes, authentication, sessions, CSRF, admin endpoints, rate limiting, and audit logging.
- **Privileged Helper & Sandboxing**: `tealdrive-helper`, Landlock ABI 3 confinement, `openat2` path resolution, worker process isolation, POSIX identity lookups, and transactional file modifications.
- **Channel Encryption Crate**: `crates/channel` (Noise NK protocol, state machine, Curve25519, ChaCha20-Poly1305, SHA256).
- **Web Frontend & Client SDK**: UI static assets, DOM manipulation, HTML escaping, and TypeScript/JavaScript SDK client logic.
- **Dependency Tree & Supply Chain**: 233 crate dependencies audited against the RustSec Advisory Database (1,329 security advisories).

### Summary of Findings
| Category | Critical | High | Medium | Low | Info/Warn | Total |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Supply Chain / CVEs (RustSec)** | 0 | 0 | 1 | 3 | 2 | **6** |
| **Internal Codebase Flaws & Logic Bugs** | 0 | 1 | 1 | 2 | 1 | **5** |
| **Total** | **0** | **1** | **2** | **5** | **3** | **11** |

---

## 2. Supply Chain & Dependency CVEs

Running `cargo audit` against `Cargo.lock` (233 crate dependencies) identified 4 CVE advisories and 2 crate status warnings:

### [RUSTSEC-2026-0346] `rustls`: `Debug` Impls for Client Session Values Expose TLS Secrets
- **Severity**: Medium (CVSS 4.0)
- **Affected Crate**: `rustls` v0.23.45 (Patched in `>= 0.23.46`)
- **Description**: The `Debug` trait implementation for client session and connection values formats internal TLS session secrets into strings. If debug logs, panic backtraces, or audit formatters output these structs, cryptographic secrets may be written to server logs or monitoring outputs.
- **Remediation**: Upgrade `rustls` to `0.23.46` or newer via `cargo update -p rustls`.

### [RUSTSEC-2026-0348] `rustls`: Brotli Certificate Decompression Allocates Up to 1 GiB Pre-Authentication
- **Severity**: Low / Medium (CVSS 3.7, CWE-400)
- **Affected Crate**: `rustls` v0.23.45 (Patched in `>= 0.23.46`)
- **Description**: When certificate compression with Brotli is enabled, an unauthenticated TLS peer sending a compressed certificate chain can trigger decompression allocations of up to 1 GiB per handshake before authentication finishes. This presents a pre-auth Denial of Service (DoS) memory exhaustion vector against TLS listeners.
- **Remediation**: Upgrade `rustls` to `>= 0.23.46`.

### [RUSTSEC-2026-0347] `rustls`: Buffered Application Data Sent to Outer Server After ECH Rejection
- **Severity**: Low (CVSS 2.3)
- **Affected Crate**: `rustls` v0.23.45 (Patched in `>= 0.23.46`)
- **Description**: If Encrypted Client Hello (ECH) is rejected by a server, buffered application data could inadvertently be forwarded to the outer server during fallback negotiation.
- **Remediation**: Upgrade `rustls` to `>= 0.23.46`.

### [RUSTSEC-2026-0349] `rustls`: Unauthenticated ServerHello Can Downgrade Connection Key Exchange
- **Severity**: Low (CVSS 3.7)
- **Affected Crate**: `rustls` v0.23.45 (Patched in `>= 0.23.46`)
- **Description**: An unauthenticated network adversary injecting a malformed or manipulated ServerHello message can force later connections to downgrade from hybrid post-quantum key exchange to classical key exchange.
- **Remediation**: Upgrade `rustls` to `>= 0.23.46`.

### [RUSTSEC-2025-0134] `rustls-pemfile`: Unmaintained Crate Warning
- **Severity**: Warning / Informational
- **Affected Crate**: `rustls-pemfile` v2.2.0
- **Description**: Upstream maintainers have archived `rustls-pemfile`. Functionality has been merged into `rustls-pki-types`. While not an immediate vulnerability, unmaintained parsing crates are a future supply chain liability.
- **Remediation**: Migrate PEM parsing calls in `src/tls.rs` directly to `rustls-pki-types`.

### [RUSTSEC-2026-0190] `anyhow`: Unsoundness in `Error::downcast_mut()`
- **Severity**: Warning / Informational
- **Affected Crate**: `anyhow` v1.0.102
- **Description**: Calling `Error::downcast_mut()` under specific generic parameter conditions can violate Rust's aliasing rules. TealDrive only formats errors and does not downcast them mutably, so this poses zero active exploitability in the current implementation.

---

## 3. Internal Codebase Vulnerabilities & Logic Bugs

Audited from largest to smallest severity:

### Finding 1: Unbounded Memory Allocation in Login Rate Limiter (Denial of Service)
- **Severity**: High (CVSS 7.5 - CWE-400 / CWE-770)
- **File**: [`src/rate_limit/login.rs`](file:///root/tealdrive/src/rate_limit/login.rs#L16-L98)
- **Mechanics**:
  `InMemoryLoginRateLimiter` maintains failed login attempts in an unbounded hash map:
  ```rust
  pub struct InMemoryLoginRateLimiter {
      max_failures: u32,
      lockout_duration_seconds: u64,
      failures: HashMap<LoginRateLimitKey, (u32, u64)>,
  }
  ```
  `record_failure_at` inserts every `(username, ip)` key that fails authentication. Unlike `auth_bridges` (which enforces `bridges.retain(...)` and caps entries at 256), `InMemoryLoginRateLimiter` has **no eviction mechanism, no TTL pruner, and no capacity ceiling**.
- **Impact**:
  An unauthenticated remote attacker can spam `/api/login` with randomized usernames or forged client IP addresses. Because failed entries are never purged unless a valid login succeeds for that exact key, the map will grow continuously until memory is exhausted and the Linux Out-Of-Memory (OOM) killer terminates `tealdrive-server`.
- **Recommended Patch**:
  Add an eviction pruner and an entry cap to `record_failure_at`:
  ```rust
  if self.failures.len() >= 10000 {
      self.failures.retain(|_, (_, last)| now.saturating_sub(*last) <= lockout_duration_seconds);
  }
  ```

---

### Finding 2: Unchecked Potential Null Pointer Dereference in `getpwnam_r` Resolution
- **Severity**: Medium (CVSS 5.3 - CWE-476)
- **File**: [`src/helper/security.rs`](file:///root/tealdrive/src/helper/security.rs#L181-L190)
- **Mechanics**:
  In `account(username: &str)`:
  ```rust
  let string = |p: *const libc::c_char| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
  Ok(AuthSuccess {
      username: string(pwd.pw_name),
      uid: pwd.pw_uid,
      primary_gid: pwd.pw_gid,
      home_dir: Some(string(pwd.pw_dir).into()),
      shell: Some(string(pwd.pw_shell)),
      groups: Vec::new(),
  })
  ```
  In comparison, `account_by_uid(uid)` at line 206 explicitly validates:
  ```rust
  if code != 0 || result.is_null() || pwd.pw_name.is_null() { ... }
  ```
- **Impact**:
  In Unix environments where custom NSS modules (LDAP, SSSD, NIS) or system accounts have missing shell or home directory entries, `pwd.pw_dir` or `pwd.pw_shell` can be `NULL`. Invoking `CStr::from_ptr` on a `NULL` pointer causes instant Undefined Behavior (segmentation fault), crashing the process or worker thread.
- **Recommended Patch**:
  Make string conversion null-safe:
  ```rust
  let string = |p: *const libc::c_char| {
      if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
  };
  ```

---

### Finding 3: Incomplete HTML Escaping in Frontend (`escapeHtml` Omits Quotes)
- **Severity**: Low / Moderate (CWE-79 / CWE-116)
- **File**: [`src/ui/static/app.js`](file:///root/tealdrive/src/ui/static/app.js#L1020-L1022)
- **Mechanics**:
  The frontend HTML escaping helper is defined as:
  ```javascript
  function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
  ```
- **Impact**:
  It only escapes `&`, `<`, and `>`. It does not sanitize quotation marks (`"`, `'`) or backticks. While currently placed in element inner text (`<span>...</span>`), should any filename or query ever be rendered inside an HTML attribute (e.g. `<span title="${escapeHtml(name)}">` or `<input value="${escapeHtml(q)}">`), an attacker who uploads a file named `x" onmouseover="alert(1).txt` could break out of the attribute and execute arbitrary JavaScript (DOM XSS).
- **Recommended Patch**:
  Escape double quotes and single quotes:
  ```javascript
  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }
  ```

---

### Finding 4: Missing Cooperative Cancellation in Recursive Directory Search
- **Severity**: Low (Resource Waste / Responsiveness)
- **File**: [`src/helper/search.rs`](file:///root/tealdrive/src/helper/search.rs#L72-L155)
- **Mechanics**:
  In long-running helper operations (such as archive compression/extraction and file downloads), `crate::helper::cancellation::check()` is called regularly to detect client disconnects or aborted operations. In `search_files_with_roots`, the `while let Some((dir_path, dir_rel, depth)) = queue.pop()` loop traverses directory trees up to depth 20 without polling `cancellation::check()`.
- **Impact**:
  If a user starts a recursive search across a vast directory hierarchy and navigates away or disconnects, the helper worker thread continues traversing the filesystem and performing disk I/O until completion or finding 1,000 matches.
- **Recommended Patch**:
  Insert cancellation checks inside the directory loop:
  ```rust
  if crate::helper::cancellation::check().is_err() {
      return HelperResponse::permission_denied(request, "Search cancelled.");
  }
  ```

---

### Finding 5: Opportunistic Responder Key Acceptance in Web SDK Noise NK Handshake
- **Severity**: Low / Informational (Cryptographic Architecture)
- **Files**: [`src/ui/sdk/client.js`](file:///root/tealdrive/src/ui/sdk/client.js#L497-L508) & [`src/ws/channel.rs`](file:///root/tealdrive/src/ws/channel.rs#L36-L40)
- **Mechanics**:
  During the WebSocket encrypted channel handshake, the server broadcasts its static public key in the initial frame (`TDN1<32-byte-public-key>`). If `this._trustedServerKeys` is empty in the JavaScript client, the SDK accepts this public key without validation:
  ```javascript
  if (this._trustedServerKeys.length && !this._trustedServerKeys.some(pin => pin.toLowerCase() === hex)) fail("SERVER_KEY_MISMATCH");
  this._channel = new module.NoiseClient(key);
  ```
- **Impact**:
  The Noise NK protocol assumes the initiator already knows and trusts the static public key of the responder. Transmitting the key over the connection and accepting it unconditionally degrades the authentication guarantees of Noise NK to Trust-On-First-Use (TOFU) or opportunistic Diffie-Hellman. While the connection is wrapped in TLS (`protocol === "https:"`), it means Noise does not protect against a TLS-terminating adversary unless public key pinning is explicitly configured.
- **Recommended Patch**:
  Embed the server's public key fingerprint during the initial HTML bootstrap response so the frontend SDK automatically populates `_trustedServerKeys`.

---

## 4. Architectural Security Strengths Observed

The codebase exhibits exceptional defense-in-depth across the following critical boundaries:

1. **Kernel Sandboxing (Landlock & Process Isolation)**:
   - The privileged helper drops all ambient Linux capabilities via `PR_SET_NO_NEW_PRIVS`.
   - Dedicated child workers execute with real `setresuid`/`setresgid` identity isolation.
   - Landlock ABI 3 rules enforce strict read/write boundaries on allowable root directories.
2. **Symlink Traversal Defenses**:
   - File opens use `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`.
   - Atomic renames utilize `libc::SYS_renameat2` with `libc::RENAME_NOREPLACE` to prevent race condition overwrites (TOCTOU).
3. **Archive Security (Zip Slip & Zip Bomb Defenses)**:
   - Archive extraction (`src/helper/archive.rs`) inspects entry paths for `..`, absolute paths, symlinks, and hardlinks.
   - Total uncompressed output is strictly bounded to 500 MB and monitored against quota reservations.
4. **Session & Transport Hardening**:
   - Cookies enforce `HttpOnly`, `Secure`, and `SameSite=Strict`.
   - CSRF tokens are validated using constant-time comparison (`diff | (a ^ b)`).
   - WebSocket upgrades validate `Origin` and `Host` headers to prevent Cross-Site WebSocket Hijacking (CSWSH).
   - File downloads serve dangerous web types (`.html`, `.svg`, `.xml`) as `attachment` with `nosniff` and CSP headers to prevent stored XSS.

---

## 5. Prioritized Remediation Roadmap

1. **Immediate (Security Patch)**:
   - Run `cargo update -p rustls` to update `rustls` from `0.23.45` to `>= 0.23.46`, resolving all 4 RustSec CVEs.
   - Apply bounded memory retention to `InMemoryLoginRateLimiter` in `src/rate_limit/login.rs`.
2. **Short-Term (Code Hardening)**:
   - Update `escapeHtml` in `src/ui/static/app.js` to escape `"` and `'`.
   - Add null checks for `pwd.pw_dir` and `pwd.pw_shell` in `src/helper/security.rs`.
   - Add cooperative cancellation check in `src/helper/search.rs`.
3. **Long-Term (Supply Chain Hygiene)**:
   - Replace `rustls-pemfile` with `rustls-pki-types`.
   - Inject server Noise static key pin into the frontend bootstrap for strict Noise NK mutual assurance.
