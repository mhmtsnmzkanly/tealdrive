pub const INDEX_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="status" content="ok">
  <title>TealDrive - Modern Two-Tier File Manager</title>
  <link rel="icon" type="image/svg+xml" href="/favicon.svg">
  <link rel="stylesheet" href="/style.css">
</head>
<body>
  <div id="app" class="app-layout">
    <!-- Top Navigation Header -->
    <header class="app-header">
      <div class="header-brand">
        <svg class="brand-icon" viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
          <line x1="12" y1="11" x2="12" y2="17"></line>
          <line x1="9" y1="14" x2="15" y2="14"></line>
        </svg>
        <span class="brand-title">TealDrive</span>
        <span class="brand-badge">Privileged Two-Tier</span>
      </div>

      <div class="header-controls">
        <div class="control-group">
          <label for="rootSelect">Storage Root:</label>
          <select id="rootSelect" class="form-select"></select>
        </div>

        <div id="userInfo" class="user-info hidden">
          <span class="user-badge" id="userBadge">user</span>
          <button id="settingsBtn" class="btn btn-secondary btn-sm" title="Server Settings">Settings</button>
          <button id="logoutBtn" class="btn btn-secondary btn-sm" title="Sign Out">Sign Out</button>
        </div>
      </div>
    </header>

    <!-- Breadcrumb & Action Toolbar -->
    <nav class="toolbar">
      <div class="breadcrumb-container" id="breadcrumbs">
        <span class="breadcrumb-item root" data-path="">Home</span>
      </div>

      <div class="action-buttons">
        <button id="refreshBtn" class="btn btn-secondary" title="Refresh listing">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 4 23 10 17 10"></polyline><polyline points="1 20 1 14 7 14"></polyline><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path></svg>
          Refresh
        </button>
        <button id="newFolderBtn" class="btn btn-secondary">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path><line x1="12" y1="11" x2="12" y2="17"></line><line x1="9" y1="14" x2="15" y2="14"></line></svg>
          New Folder
        </button>
        <button id="newFileBtn" class="btn btn-secondary">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="12" y1="18" x2="12" y2="12"></line><line x1="9" y1="15" x2="15" y2="15"></line></svg>
          New File
        </button>
        <button id="uploadBtn" class="btn btn-primary">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
          Upload
        </button>
        <button id="trashBtn" class="btn btn-secondary" title="View Trash Bin">
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
          Trash Bin
        </button>
        <div class="search-box" style="display:inline-flex;align-items:center;gap:4px;margin-left:0.5rem;">
          <input type="text" id="searchInput" placeholder="Search files..." style="padding:4px 8px;font-size:0.85rem;border-radius:4px;border:1px solid var(--slate-700);background:var(--slate-800);color:var(--text-main);width:150px;">
          <button id="searchClearBtn" class="btn btn-secondary btn-sm hidden" title="Clear Search">&times;</button>
        </div>
      </div>
    </nav>

    <!-- Main Content Area: File Table -->
    <main class="content-container">
      <div id="dropzoneOverlay" class="dropzone-overlay hidden">
        <div class="dropzone-box">
          <svg viewBox="0 0 24 24" width="48" height="48" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
          <p>Drop files here to upload</p>
        </div>
      </div>

      <div class="table-card">
        <table class="file-table" id="fileTable">
          <thead>
            <tr>
              <th class="col-name">Name</th>
              <th class="col-size">Size</th>
              <th class="col-mode">Mode</th>
              <th class="col-modified">Modified</th>
              <th class="col-actions">Actions</th>
            </tr>
          </thead>
          <tbody id="fileTableBody">
            <tr class="empty-row">
              <td colspan="5">Loading directory contents...</td>
            </tr>
          </tbody>
        </table>
      </div>
    </main>

    <!-- Upload Progress Banner -->
    <div id="uploadProgressContainer" class="upload-progress-container hidden">
      <div class="progress-info">
        <span id="uploadFileName">Uploading file...</span>
        <span id="uploadPercent">0%</span>
      </div>
      <div class="progress-bar-bg">
        <div id="uploadProgressBar" class="progress-bar-fill" style="width: 0%"></div>
      </div>
    </div>

    <!-- Status Footer -->
    <footer class="app-footer">
      <div class="footer-status">
        <span class="status-dot connected" id="wsStatusDot"></span>
        <span id="wsStatusText">Connecting...</span>
      </div>
      <div class="footer-meta">
        <span>TealDrive Safe Sandboxed Environment &copy; 2026</span>
      </div>
    </footer>
  </div>

  <!-- Hidden File Input for Upload -->
  <input type="file" id="fileInput" class="hidden">

  <!-- MODALS -->

  <!-- 1. Login Modal -->
  <div id="loginModal" class="modal-backdrop hidden">
    <div class="modal-card">
      <div class="modal-header">
        <div class="brand-brand">
          <span class="brand-title">Sign in to TealDrive</span>
        </div>
      </div>
      <form id="loginForm" class="modal-body">
        <div id="loginError" class="alert-error hidden"></div>
        <div class="form-group">
          <label for="usernameInput">Username</label>
          <input type="text" id="usernameInput" class="form-input" required autocomplete="username" autofocus>
        </div>
        <div class="form-group">
          <label for="passwordInput">Password</label>
          <input type="password" id="passwordInput" class="form-input" required autocomplete="current-password">
        </div>
        <div class="modal-actions">
          <button type="submit" class="btn btn-primary btn-block">Sign In</button>
        </div>
      </form>
    </div>
  </div>

  <!-- 2. Text Editor / Viewer Modal -->
  <div id="editorModal" class="modal-backdrop hidden">
    <div class="modal-card modal-lg">
      <div class="modal-header">
        <h3 id="editorTitle">Edit File</h3>
        <button class="modal-close" id="editorCloseBtn">&times;</button>
      </div>
      <div class="modal-body editor-body">
        <textarea id="editorTextarea" class="code-editor" spellcheck="false"></textarea>
      </div>
      <div class="modal-footer">
        <span class="file-meta-hint" id="editorMetaHint"></span>
        <div class="modal-actions">
          <button id="editorCancelBtn" class="btn btn-secondary">Cancel</button>
          <button id="editorSaveBtn" class="btn btn-primary">Save Changes</button>
        </div>
      </div>
    </div>
  </div>

  <!-- 3. Prompt (New Folder / Rename) Modal -->
  <div id="promptModal" class="modal-backdrop hidden">
    <div class="modal-card">
      <div class="modal-header">
        <h3 id="promptTitle">Prompt</h3>
        <button class="modal-close" id="promptCloseBtn">&times;</button>
      </div>
      <form id="promptForm" class="modal-body">
        <div class="form-group">
          <label id="promptLabel" for="promptInput">Name</label>
          <input type="text" id="promptInput" class="form-input" required>
        </div>
        <div class="modal-actions">
          <button type="button" id="promptCancelBtn" class="btn btn-secondary">Cancel</button>
          <button type="submit" class="btn btn-primary">Confirm</button>
        </div>
      </form>
    </div>
  </div>

  <!-- 4. Permissions (Chmod) Modal -->
  <div id="chmodModal" class="modal-backdrop hidden">
    <div class="modal-card">
      <div class="modal-header">
        <h3>File Permissions</h3>
        <button class="modal-close" id="chmodCloseBtn">&times;</button>
      </div>
      <form id="chmodForm" class="modal-body">
        <p class="file-target-name" id="chmodTargetName"></p>
        <p class="muted">Select permissions or enter an octal code. Both stay in sync.</p>
        <table class="permission-grid">
          <thead><tr><th scope="col">Who</th><th scope="col">Read</th><th scope="col">Write</th><th scope="col">Execute</th></tr></thead>
          <tbody><tr><th scope="row">Owner</th><td><input type="checkbox" class="chmod-bit" data-bit="256" aria-label="Owner: Read"></td><td><input type="checkbox" class="chmod-bit" data-bit="128" aria-label="Owner: Write"></td><td><input type="checkbox" class="chmod-bit" data-bit="64" aria-label="Owner: Execute"></td></tr>
<tr><th scope="row">Group</th><td><input type="checkbox" class="chmod-bit" data-bit="32" aria-label="Group: Read"></td><td><input type="checkbox" class="chmod-bit" data-bit="16" aria-label="Group: Write"></td><td><input type="checkbox" class="chmod-bit" data-bit="8" aria-label="Group: Execute"></td></tr>
<tr><th scope="row">Others</th><td><input type="checkbox" class="chmod-bit" data-bit="4" aria-label="Others: Read"></td><td><input type="checkbox" class="chmod-bit" data-bit="2" aria-label="Others: Write"></td><td><input type="checkbox" class="chmod-bit" data-bit="1" aria-label="Others: Execute"></td></tr>
</tbody>
        </table>
        <div class="form-group">
          <label for="chmodOctalInput">Permission code</label>
          <input type="text" id="chmodOctalInput" class="form-input" pattern="0?[0-7]{3}" maxlength="4" inputmode="numeric" autocomplete="off" aria-describedby="chmodPreview chmodError" required>
          <p id="chmodPreview" class="permission-preview" aria-live="polite"></p>
          <p id="chmodError" class="permission-error" role="status"></p>
        </div>
        <div class="modal-actions">
          <button type="button" id="chmodCancelBtn" class="btn btn-secondary">Cancel</button>
          <button type="submit" id="chmodApplyBtn" class="btn btn-primary">Apply Permissions</button>
        </div>
      </form>
    </div>
  </div>

  <!-- 5. Archive (Compress / Extract) Modal -->
  <div id="archiveModal" class="modal-backdrop hidden">
    <div class="modal-card">
      <div class="modal-header">
        <h3 id="archiveTitle">Create Archive</h3>
        <button class="modal-close" id="archiveCloseBtn">&times;</button>
      </div>
      <form id="archiveForm" class="modal-body">
        <div class="form-group">
          <label for="archiveNameInput">Archive Name (e.g. bundle.zip or backup.tar.gz):</label>
          <input type="text" id="archiveNameInput" class="form-input" required>
        </div>
        <div class="modal-actions">
          <button type="button" id="archiveCancelBtn" class="btn btn-secondary">Cancel</button>
          <button type="submit" class="btn btn-primary">Compress</button>
        </div>
      </form>
    </div>
  </div>

  <!-- 6. Trash Bin Modal -->
  <div id="trashModal" class="modal-backdrop hidden">
    <div class="modal-card modal-lg">
      <div class="modal-header">
        <h3>Trash Bin</h3>
        <button class="modal-close" id="trashCloseBtn">&times;</button>
      </div>
      <div class="modal-body">
        <p class="muted">Items moved to trash can be restored or permanently removed.</p>
        <div class="table-card">
          <table class="file-table" id="trashTable">
            <thead>
              <tr>
                <th>Original Name</th>
                <th>Original Path</th>
                <th>Deleted Time</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody id="trashTableBody">
              <tr class="empty-row"><td colspan="4">Trash bin is empty.</td></tr>
            </tbody>
          </table>
        </div>
      </div>
      <div class="modal-footer">
        <button type="button" id="trashDoneBtn" class="btn btn-secondary">Close</button>
      </div>
    </div>
  </div>

  <!-- 7. Lightbox Preview Modal -->
  <div id="lightboxModal" class="modal-backdrop hidden">
    <div class="modal-card modal-lightbox">
      <div class="modal-header">
        <h3 id="lightboxTitle">File Preview</h3>
        <button class="modal-close" id="lightboxCloseBtn">&times;</button>
      </div>
      <div class="modal-body lightbox-body">
        <div id="lightboxContent" class="lightbox-content"></div>
      </div>
      <div class="modal-footer">
        <a id="lightboxDownloadBtn" href="#" class="btn btn-secondary" target="_blank">Download</a>
        <button type="button" id="lightboxDoneBtn" class="btn btn-secondary">Close</button>
      </div>
    </div>
  </div>

  <!-- 8. Server Settings Modal -->
  <div id="settingsModal" class="modal-backdrop hidden">
    <div class="modal-card modal-lg">
      <div class="modal-header">
        <h3>Server Settings &amp; Dynamic Config</h3>
        <button class="modal-close" id="settingsCloseBtn">&times;</button>
      </div>
      <div class="modal-body settings-body">
        <div class="settings-section">
          <h4>Feature Gates</h4>
          <div class="settings-grid">
            <label><input type="checkbox" id="cfgFeatEdit"> Allow File Editing</label>
            <label><input type="checkbox" id="cfgFeatDelete"> Allow Permanent Deletion</label>
            <label><input type="checkbox" id="cfgFeatChmod"> Allow Chmod Permissions</label>
            <label><input type="checkbox" id="cfgFeatArchive"> Allow Archive Extraction</label>
          </div>
        </div>

        <div class="settings-section">
          <h4>Retention &amp; Limits</h4>
          <div class="settings-row">
            <label for="cfgTrashRetention">Trash Retention (automatic cleanup unavailable):</label>
            <input type="number" id="cfgTrashRetention" class="form-input" min="0" max="365" disabled>
          </div>
          <div class="settings-row">
            <label for="cfgMaxUploadBytes">Max Upload (MB):</label>
            <input type="number" id="cfgMaxUploadBytes" class="form-input" min="1" max="3">
          </div>
          <div class="settings-row">
            <label for="cfgMaxEditTextBytes">Max Text Edit (KB):</label>
            <input type="number" id="cfgMaxEditTextBytes" class="form-input" min="1" max="128">
          </div>
          <div class="settings-row">
            <label for="cfgMaxPreviewBytes">Max Preview / Inline (MB):</label>
            <input type="number" id="cfgMaxPreviewBytes" class="form-input" min="1" max="1024">
          </div>
        </div>

        <div class="settings-section">
          <h4>Rate Limiting &amp; Session</h4>
          <div class="settings-row">
            <label for="cfgRateLimitMaxFailures">Max Login Failures:</label>
            <input type="number" id="cfgRateLimitMaxFailures" class="form-input" min="1" max="100">
          </div>
          <div class="settings-row">
            <label for="cfgRateLimitLockoutSecs">Lockout Duration (Seconds):</label>
            <input type="number" id="cfgRateLimitLockoutSecs" class="form-input" min="1" max="86400">
          </div>
          <div class="settings-row">
            <label for="cfgSessionIdleMins">Session Idle Timeout (Minutes):</label>
            <input type="number" id="cfgSessionIdleMins" class="form-input" min="1" max="10080">
          </div>
        </div>

        <div class="settings-section">
          <h4>Helper Engine &amp; Search</h4>
          <div class="settings-row">
            <label for="cfgHelperTimeoutSecs">Helper Process Timeout (Seconds):</label>
            <input type="number" id="cfgHelperTimeoutSecs" class="form-input" min="1" max="300">
          </div>
          <div class="settings-row">
            <label for="cfgSearchMaxDepth">Search Max Depth (Levels):</label>
            <input type="number" id="cfgSearchMaxDepth" class="form-input" min="1" max="100">
          </div>
          <div class="settings-row">
            <label for="cfgSearchMaxResults">Search Default Max Results:</label>
            <input type="number" id="cfgSearchMaxResults" class="form-input" min="1" max="5000">
          </div>
        </div>

        <div class="settings-section">
          <label style="display:flex;align-items:center;gap:6px;cursor:pointer;">
            <input type="checkbox" id="cfgPersistDisk" checked> Persist changes to disk config file
          </label>
        </div>
      </div>
      <div class="modal-footer">
        <button type="button" id="cfgReloadBtn" class="btn btn-secondary">Reload from Disk</button>
        <div class="modal-actions">
          <button type="button" id="settingsCancelBtn" class="btn btn-secondary">Close</button>
          <button type="button" id="cfgSaveBtn" class="btn btn-primary">Save Settings</button>
        </div>
      </div>
    </div>
  </div>

  <!-- Toast Notification Container -->
  <div id="toastContainer" class="toast-container"></div>

  <script src="/app.js"></script>
</body>
</html>
"##;

pub const STYLE_CSS: &str = r##"
:root {
  --teal-50: #f0fdfa;
  --teal-100: #ccfbf1;
  --teal-500: #14b8a6;
  --teal-600: #0d9488;
  --teal-700: #0f766e;
  --slate-900: #0f172a;
  --slate-800: #1e293b;
  --slate-700: #334155;
  --slate-600: #475569;
  --slate-300: #cbd5e1;
  --slate-100: #f1f5f9;
  --text-main: #f8fafc;
  --text-muted: #94a3b8;
  --danger: #ef4444;
  --danger-hover: #dc2626;
  --success: #10b981;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  --border-radius: 8px;
  --shadow-lg: 0 10px 25px -5px rgba(0, 0, 0, 0.4), 0 8px 10px -6px rgba(0, 0, 0, 0.3);
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: var(--font-sans);
  background-color: var(--slate-900);
  color: var(--text-main);
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.hidden {
  display: none !important;
}

.app-layout {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
}

/* Header */
.app-header {
  background-color: var(--slate-800);
  border-bottom: 1px solid var(--slate-700);
  padding: 0.75rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.header-brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.brand-icon {
  color: var(--teal-500);
}

.brand-title {
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-main);
  letter-spacing: -0.02em;
}

.brand-badge {
  background-color: rgba(20, 184, 166, 0.15);
  color: var(--teal-500);
  font-size: 0.75rem;
  padding: 0.15rem 0.5rem;
  border-radius: 9999px;
  border: 1px solid rgba(20, 184, 166, 0.3);
}

.header-controls {
  display: flex;
  align-items: center;
  gap: 1.25rem;
}

.control-group {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  color: var(--text-muted);
}

.form-select, .form-input {
  background-color: var(--slate-900);
  border: 1px solid var(--slate-700);
  color: var(--text-main);
  border-radius: var(--border-radius);
  padding: 0.4rem 0.75rem;
  font-size: 0.875rem;
  outline: none;
  transition: border-color 0.15s ease;
}

.form-select:focus, .form-input:focus {
  border-color: var(--teal-500);
}

.user-info {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.user-badge {
  background-color: var(--slate-700);
  padding: 0.25rem 0.6rem;
  border-radius: 9999px;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--teal-100);
}

/* Toolbar */
.toolbar {
  background-color: rgba(30, 41, 59, 0.5);
  border-bottom: 1px solid var(--slate-700);
  padding: 0.75rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 1rem;
}

.breadcrumb-container {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.95rem;
  flex-wrap: wrap;
}

.breadcrumb-item {
  color: var(--teal-500);
  cursor: pointer;
  text-decoration: none;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
}

.breadcrumb-item:hover {
  background-color: rgba(20, 184, 166, 0.1);
}

.breadcrumb-sep {
  color: var(--text-muted);
}

.action-buttons {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

/* Buttons */
.btn {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.45rem 0.85rem;
  font-size: 0.875rem;
  font-weight: 500;
  border-radius: var(--border-radius);
  border: none;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-primary {
  background-color: var(--teal-600);
  color: #fff;
}

.btn-primary:hover {
  background-color: var(--teal-500);
}

.btn-secondary {
  background-color: var(--slate-700);
  color: var(--text-main);
}

.btn-secondary:hover {
  background-color: var(--slate-600);
}

.btn-danger {
  background-color: var(--danger);
  color: #fff;
}

.btn-danger:hover {
  background-color: var(--danger-hover);
}

.btn-sm {
  padding: 0.25rem 0.5rem;
  font-size: 0.8rem;
}

.btn-block {
  width: 100%;
  justify-content: center;
  padding: 0.6rem;
}

/* Content & Table */
.content-container {
  flex: 1;
  padding: 1.5rem;
  position: relative;
  overflow-y: auto;
}

.table-card {
  background-color: var(--slate-800);
  border: 1px solid var(--slate-700);
  border-radius: var(--border-radius);
  overflow: hidden;
}

.file-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
  font-size: 0.9rem;
}

.file-table th {
  background-color: rgba(15, 23, 42, 0.4);
  padding: 0.75rem 1rem;
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1px solid var(--slate-700);
}

.file-table td {
  padding: 0.75rem 1rem;
  border-bottom: 1px solid rgba(51, 65, 85, 0.5);
  vertical-align: middle;
}

.file-table tr:hover td {
  background-color: rgba(51, 65, 85, 0.3);
}

.col-name { width: 45%; }
.col-size { width: 12%; }
.col-mode { width: 10%; }
.col-modified { width: 18%; }
.col-actions { width: 15%; text-align: right; }

.item-name {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  cursor: pointer;
  color: var(--text-main);
  text-decoration: none;
}

.item-name:hover {
  color: var(--teal-500);
}

.item-icon {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
}

.row-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.35rem;
}

.empty-row td {
  text-align: center;
  padding: 2.5rem 1rem;
  color: var(--text-muted);
}

/* Dropzone Overlay */
.dropzone-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(15, 23, 42, 0.85);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}

.dropzone-box {
  border: 2px dashed var(--teal-500);
  border-radius: 12px;
  padding: 3rem;
  text-align: center;
  color: var(--teal-100);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

/* Progress bar */
.upload-progress-container {
  background-color: var(--slate-800);
  border-top: 1px solid var(--slate-700);
  padding: 0.75rem 1.5rem;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: 0.8rem;
  margin-bottom: 0.35rem;
  color: var(--text-muted);
}

.progress-bar-bg {
  background-color: var(--slate-900);
  border-radius: 9999px;
  height: 6px;
  overflow: hidden;
}

.progress-bar-fill {
  background-color: var(--teal-500);
  height: 100%;
  transition: width 0.15s ease;
}

/* Footer */
.app-footer {
  background-color: var(--slate-800);
  border-top: 1px solid var(--slate-700);
  padding: 0.5rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-muted);
}

.footer-status {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background-color: var(--slate-600);
}

.status-dot.connected {
  background-color: var(--success);
}

.status-dot.disconnected {
  background-color: var(--danger);
}

/* Modals */
.modal-backdrop {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(15, 23, 42, 0.75);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 1rem;
}

.modal-card {
  background-color: var(--slate-800);
  border: 1px solid var(--slate-700);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  width: 100%;
  max-width: 440px;
  overflow: hidden;
}

.modal-lg {
  max-width: 800px;
  width: 90%;
  height: 80vh;
  display: flex;
  flex-direction: column;
}

.modal-lightbox {
  max-width: 900px;
  width: 90%;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
}

.lightbox-body {
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--slate-900);
  padding: 1rem;
  overflow: auto;
  max-height: calc(85vh - 130px);
}

.lightbox-content {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

.lightbox-content img {
  max-width: 100%;
  max-height: 70vh;
  object-fit: contain;
  border-radius: 6px;
  box-shadow: var(--shadow);
}

.lightbox-content video, .lightbox-content audio {
  max-width: 100%;
  border-radius: 6px;
}

.modal-header {
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--slate-700);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.modal-close {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 1.5rem;
  cursor: pointer;
}

.modal-close:hover {
  color: var(--text-main);
}

.modal-body {
  padding: 1.25rem;
}

.editor-body {
  flex: 1;
  display: flex;
  padding: 0;
  overflow: hidden;
}

.code-editor {
  width: 100%;
  height: 100%;
  background-color: #0b1120;
  color: #e2e8f0;
  border: none;
  padding: 1rem;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  line-height: 1.5;
  outline: none;
  resize: none;
}

.modal-footer {
  padding: 0.75rem 1.25rem;
  border-top: 1px solid var(--slate-700);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.file-meta-hint {
  font-size: 0.75rem;
  color: var(--text-muted);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  margin-bottom: 1rem;
}

.form-group label {
  font-size: 0.85rem;
  color: var(--text-muted);
}

.alert-error {
  background-color: rgba(239, 68, 68, 0.15);
  border: 1px solid rgba(239, 68, 68, 0.4);
  color: #fca5a5;
  padding: 0.6rem 0.75rem;
  border-radius: var(--border-radius);
  font-size: 0.85rem;
  margin-bottom: 1rem;
}

.file-target-name {
  font-family: var(--font-mono);
  font-size: 0.85rem;
  color: var(--teal-500);
  margin-bottom: 0.75rem;
}

/* Toast Container */
.toast-container {
  position: fixed;
  bottom: 2rem;
  right: 2rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  z-index: 200;
}

.toast {
  background-color: var(--slate-800);
  border: 1px solid var(--slate-700);
  color: var(--text-main);
  padding: 0.75rem 1rem;
  border-radius: var(--border-radius);
  box-shadow: var(--shadow-lg);
  font-size: 0.875rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  animation: slideIn 0.2s ease-out;
}

.toast-success { border-left: 4px solid var(--success); }
.toast-error { border-left: 4px solid var(--danger); }

@keyframes slideIn {
  from { transform: translateX(100%); opacity: 0; }
  to { transform: translateX(0); opacity: 1; }
}

/* Settings Modal Styles */
.settings-body {
  overflow-y: auto;
  max-height: calc(80vh - 140px);
}

.settings-section {
  background-color: var(--slate-900);
  border: 1px solid var(--slate-700);
  border-radius: var(--border-radius);
  padding: 1rem;
  margin-bottom: 1rem;
}

.settings-section h4 {
  margin-top: 0;
  margin-bottom: 0.75rem;
  color: var(--teal-100);
  font-size: 0.95rem;
  border-bottom: 1px solid var(--slate-700);
  padding-bottom: 0.4rem;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 0.6rem;
}

.settings-grid label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.85rem;
  cursor: pointer;
  color: var(--text-main);
}

.settings-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.5rem;
  gap: 1rem;
}

.settings-row label {
  font-size: 0.85rem;
  color: var(--text-muted);
}

.settings-row .form-input {
  width: 140px;
}

.permission-grid { width:100%; border-collapse:collapse; margin:16px 0 20px; font-size:14px; }
.permission-grid th,.permission-grid td { padding:12px 4px; text-align:center; border-bottom:1px solid var(--border-color, #334155); }
.permission-grid th:first-child { text-align:left; }
.permission-grid input { width:20px; height:20px; accent-color:var(--teal-500); cursor:pointer; }
.permission-preview { font-family:monospace; letter-spacing:2px; margin:8px 0; }
.permission-error { color:var(--danger); font-size:13px; min-height:20px; }
.permission-grid + .form-group { margin-top:16px; }
"##;

pub const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">
  <rect width="32" height="32" rx="6" fill="#0f766e"/>
  <path d="M6 9a2 2 0 0 1 2-2h4.5l2 2.5H24a2 2 0 0 1 2 2V23a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V9z" fill="#14b8a6"/>
  <circle cx="16" cy="18" r="2.5" fill="#ffffff"/>
</svg>"##;

pub const APP_JS: &str = r##"
// TealDrive Frontend Client SPA
(function() {
  'use strict';

  // State
  let state = {
    session: null,
    csrfToken: '',
    roots: [],
    currentRoot: null,
    currentPath: '',
    ws: null,
    wsConnected: false,
    requestId: 1,
    pendingRequests: new Map(),
    currentEditorFile: null,
    chmodTarget: null,
    archiveSources: [],
  };

  // DOM Elements
  const el = {
    rootSelect: document.getElementById('rootSelect'),
    userInfo: document.getElementById('userInfo'),
    userBadge: document.getElementById('userBadge'),
    logoutBtn: document.getElementById('logoutBtn'),
    breadcrumbs: document.getElementById('breadcrumbs'),
    refreshBtn: document.getElementById('refreshBtn'),
    newFolderBtn: document.getElementById('newFolderBtn'),
    newFileBtn: document.getElementById('newFileBtn'),
    uploadBtn: document.getElementById('uploadBtn'),
    trashBtn: document.getElementById('trashBtn'),
    fileInput: document.getElementById('fileInput'),
    fileTableBody: document.getElementById('fileTableBody'),
    wsStatusDot: document.getElementById('wsStatusDot'),
    wsStatusText: document.getElementById('wsStatusText'),
    toastContainer: document.getElementById('toastContainer'),
    // Modals
    loginModal: document.getElementById('loginModal'),
    loginForm: document.getElementById('loginForm'),
    usernameInput: document.getElementById('usernameInput'),
    passwordInput: document.getElementById('passwordInput'),
    loginError: document.getElementById('loginError'),
    editorModal: document.getElementById('editorModal'),
    editorTitle: document.getElementById('editorTitle'),
    editorTextarea: document.getElementById('editorTextarea'),
    editorCloseBtn: document.getElementById('editorCloseBtn'),
    editorCancelBtn: document.getElementById('editorCancelBtn'),
    editorSaveBtn: document.getElementById('editorSaveBtn'),
    editorMetaHint: document.getElementById('editorMetaHint'),
    promptModal: document.getElementById('promptModal'),
    promptTitle: document.getElementById('promptTitle'),
    promptLabel: document.getElementById('promptLabel'),
    promptInput: document.getElementById('promptInput'),
    promptForm: document.getElementById('promptForm'),
    promptCloseBtn: document.getElementById('promptCloseBtn'),
    promptCancelBtn: document.getElementById('promptCancelBtn'),
    chmodModal: document.getElementById('chmodModal'),
    chmodForm: document.getElementById('chmodForm'),
    chmodCloseBtn: document.getElementById('chmodCloseBtn'),
    chmodCancelBtn: document.getElementById('chmodCancelBtn'),
    chmodTargetName: document.getElementById('chmodTargetName'),
    chmodOctalInput: document.getElementById('chmodOctalInput'),
    chmodBits: [...document.querySelectorAll('#chmodForm .chmod-bit')],
    chmodPreview: document.getElementById('chmodPreview'),
    chmodError: document.getElementById('chmodError'),
    chmodApplyBtn: document.getElementById('chmodApplyBtn'),
    archiveModal: document.getElementById('archiveModal'),
    archiveForm: document.getElementById('archiveForm'),
    archiveCloseBtn: document.getElementById('archiveCloseBtn'),
    archiveCancelBtn: document.getElementById('archiveCancelBtn'),
    archiveNameInput: document.getElementById('archiveNameInput'),
    trashModal: document.getElementById('trashModal'),
    trashCloseBtn: document.getElementById('trashCloseBtn'),
    trashDoneBtn: document.getElementById('trashDoneBtn'),
    trashTableBody: document.getElementById('trashTableBody'),
    lightboxModal: document.getElementById('lightboxModal'),
    lightboxTitle: document.getElementById('lightboxTitle'),
    lightboxContent: document.getElementById('lightboxContent'),
    lightboxCloseBtn: document.getElementById('lightboxCloseBtn'),
    lightboxDoneBtn: document.getElementById('lightboxDoneBtn'),
    lightboxDownloadBtn: document.getElementById('lightboxDownloadBtn'),
    uploadProgressContainer: document.getElementById('uploadProgressContainer'),
    uploadFileName: document.getElementById('uploadFileName'),
    uploadPercent: document.getElementById('uploadPercent'),
    uploadProgressBar: document.getElementById('uploadProgressBar'),
    searchInput: document.getElementById('searchInput'),
    searchClearBtn: document.getElementById('searchClearBtn'),
    settingsBtn: document.getElementById('settingsBtn'),
    settingsModal: document.getElementById('settingsModal'),
    settingsCloseBtn: document.getElementById('settingsCloseBtn'),
    settingsCancelBtn: document.getElementById('settingsCancelBtn'),
    cfgSaveBtn: document.getElementById('cfgSaveBtn'),
    cfgReloadBtn: document.getElementById('cfgReloadBtn'),
    cfgFeatEdit: document.getElementById('cfgFeatEdit'),
    cfgFeatDelete: document.getElementById('cfgFeatDelete'),
    cfgFeatChmod: document.getElementById('cfgFeatChmod'),
    cfgFeatArchive: document.getElementById('cfgFeatArchive'),
    cfgTrashRetention: document.getElementById('cfgTrashRetention'),
    cfgMaxUploadBytes: document.getElementById('cfgMaxUploadBytes'),
    cfgMaxEditTextBytes: document.getElementById('cfgMaxEditTextBytes'),
    cfgMaxPreviewBytes: document.getElementById('cfgMaxPreviewBytes'),
    cfgRateLimitMaxFailures: document.getElementById('cfgRateLimitMaxFailures'),
    cfgRateLimitLockoutSecs: document.getElementById('cfgRateLimitLockoutSecs'),
    cfgSessionIdleMins: document.getElementById('cfgSessionIdleMins'),
    cfgHelperTimeoutSecs: document.getElementById('cfgHelperTimeoutSecs'),
    cfgSearchMaxDepth: document.getElementById('cfgSearchMaxDepth'),
    cfgSearchMaxResults: document.getElementById('cfgSearchMaxResults'),
    cfgPersistDisk: document.getElementById('cfgPersistDisk'),
  };

  const {MsgPack,encodeFrame,decodeFrame,encodeChunk,uuidBytes,uuidKey}=TealDriveWire;
  const MT=TdrvContract.messages;
  // --- WebSocket Connection ---
  function connectWebSocket() {
    if (state.ws) {
      try { state.ws.close(); } catch(e) {}
    }

    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${proto}//${window.location.host}/ws`;
    el.wsStatusDot.className = 'status-dot';
    el.wsStatusText.textContent = 'Connecting to /ws...';

    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    state.ws = ws;

    ws.onopen = function() { el.wsStatusText.textContent='Negotiating protocol...'; };

    ws.onmessage = function(ev) {
      try { handleServerFrame(decodeFrame(ev.data)); } catch(error) {showToast(error.message,'error');ws.close();}
    };

    ws.onclose = function() {
      state.wsConnected = false;
      el.wsStatusDot.className = 'status-dot disconnected';
      el.wsStatusText.textContent = 'Disconnected';
      for(const pending of state.pendingRequests.values()){clearTimeout(pending.timer);pending.reject(new Error("Connection closed"));}
      state.pendingRequests.clear();
      setTimeout(checkSession, 3000);
    };

    ws.onerror = function() {
      state.wsConnected = false;
      el.wsStatusDot.className = 'status-dot disconnected';
      el.wsStatusText.textContent = 'Connection error';
    };
  }

  function handleServerFrame(frame) {
    const {messageType,payload,requestId}=frame;
    const pending=state.pendingRequests.get(requestId);
    const failed=[MT.Error,MT.OperationFailed,MT.PermissionDenied,MT.PolicyDenied,MT.ValidationError].includes(messageType);
    if(pending){clearTimeout(pending.timer);state.pendingRequests.delete(requestId);
      if(failed)pending.reject(new Error(payload.error?.safe_message||payload.safe_message||'Operation failed'));
      else pending.resolve(payload);
    }
    switch(messageType){
      case MT.ServerHello:
        state.serverHello=payload;
        state.ws.send(encodeFrame(MT.ClientHello,{
          protocol_version:1,supported_encodings:['MessagePack','RawBinary'],supported_compressions:['None'],
          max_frame_size:payload.max_frame_size,max_control_payload_size:payload.max_control_payload_size,max_chunk_size:payload.max_chunk_size,
          feature_flags:payload.feature_flags
        },crypto.randomUUID()));
        break;
      case MT.ProtocolReady:
        state.protocolLimits=payload.limits;state.wsConnected=true;
        el.wsStatusDot.className='status-dot connected';el.wsStatusText.textContent='Connected';loadDirectory(state.currentPath);break;
      case MT.DirectoryList:renderDirectoryList(payload);break;
      case MT.SearchResults:renderSearchResults(payload);break;
      case MT.TrashList:renderTrashList(payload);break;
      case MT.TextFileContent:openEditor(payload);break;
      case MT.OperationDone:if(!pending){showToast(payload.safe_message||'Operation succeeded','success');loadDirectory(state.currentPath);}break;
      default:if(failed&&!pending)showToast(payload.error?.safe_message||payload.safe_message||'Server error','error');
    }
  }
  function requestWs(messageType,payload,raw=false){
    return new Promise((resolve,reject)=>{
      if(!state.wsConnected||!state.ws||state.ws.readyState!==WebSocket.OPEN){reject(new Error('Connection is not ready'));return;}
      const id=crypto.randomUUID();const key=uuidKey(id);
      const timer=setTimeout(()=>{state.pendingRequests.delete(key);reject(new Error('Operation timed out'));},60000);
      state.pendingRequests.set(key,{resolve,reject,timer});
      try{state.ws.send(encodeFrame(messageType,payload,id,raw));}
      catch(error){clearTimeout(timer);state.pendingRequests.delete(key);reject(error);}
    });
  }
  function sendWsRequest(messageType,payload){
    requestWs(messageType,payload).then(()=>{if([MT.SaveTextFile,MT.CreateFolder,MT.RenameFile,MT.MoveToTrash,MT.RestoreFromTrash,MT.DeletePermanently,MT.ChmodFile].includes(messageType)){showToast('Operation completed','success');loadDirectory(state.currentPath);}}).catch(error=>showToast(error.message,'error'));
  }

  // --- Directory Listing & Navigation ---
  function loadDirectory(path, cursor = null) {
    state.currentPath = path || '';
    renderBreadcrumbs();
    el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Loading files...</td></tr>';
    sendWsRequest(MT.ListDirectory, {
      root_id: state.currentRoot,
      relative_path: state.currentPath,cursor,limit:state.protocolLimits.default_directory_page_size,include_hidden:false,sort:"NameAsc",filter:{query:null,file_kind:null,filter:"All"},
    });
  }

  function renderBreadcrumbs() {
    el.breadcrumbs.innerHTML = '';
    const rootItem = document.createElement('span');
    rootItem.className = 'breadcrumb-item root';
    rootItem.textContent = state.currentRoot || 'Root';
    rootItem.onclick = () => loadDirectory('');
    el.breadcrumbs.appendChild(rootItem);

    if (!state.currentPath) return;

    const parts = state.currentPath.split('/').filter(Boolean);
    let accum = '';
    for (let i = 0; i < parts.length; i++) {
      const sep = document.createElement('span');
      sep.className = 'breadcrumb-sep';
      sep.textContent = '/';
      el.breadcrumbs.appendChild(sep);

      accum += (accum ? '/' : '') + parts[i];
      const targetPath = accum;
      const partItem = document.createElement('span');
      partItem.className = 'breadcrumb-item';
      partItem.textContent = parts[i];
      partItem.onclick = () => loadDirectory(targetPath);
      el.breadcrumbs.appendChild(partItem);
    }
  }

  function renderDirectoryList(data) {
    el.fileTableBody.innerHTML = '';
    const entries = (data.entries || []).map(item=>({...item,is_dir:item.file_type==='Directory'}));

    if (entries.length === 0) {
      el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Empty directory</td></tr>';
      return;
    }

    if(data.has_more && data.next_cursor) {
      const row=document.createElement('tr');const cell=document.createElement('td');cell.colSpan=5;
      const previous=document.createElement('button');previous.textContent='First page';previous.onclick=()=>loadDirectory(state.currentPath);
      const next=document.createElement('button');next.textContent='Next page';next.onclick=()=>loadDirectory(state.currentPath,data.next_cursor);
      cell.append(previous,next);row.appendChild(cell);el.fileTableBody.appendChild(row);
    }
    // Sort: directories first, then alphabetical
    entries.sort((a, b) => {
      if (a.is_dir !== b.is_dir) return b.is_dir ? 1 : -1;
      return a.name.localeCompare(b.name);
    });

    for (const item of entries) {
      const tr = document.createElement('tr');

      // Name & Icon
      const tdName = document.createElement('td');
      const nameWrapper = document.createElement('a');
      nameWrapper.className = 'item-name';
      nameWrapper.href = '#';

      const icon = item.is_dir ? '📁' : getFileIcon(item.name);
      nameWrapper.innerHTML = `<span>${icon}</span> <span>${escapeHtml(item.name)}</span>`;
      nameWrapper.onclick = (e) => {
        e.preventDefault();
        const subPath = state.currentPath ? `${state.currentPath}/${item.name}` : item.name;
        if (item.is_dir) {
          loadDirectory(subPath);
        } else if (isMediaFile(item.name)) {
          openLightbox(subPath);
        } else {
          viewOrEditFile(subPath);
        }
      };
      tdName.appendChild(nameWrapper);

      // Size
      const tdSize = document.createElement('td');
      tdSize.textContent = item.is_dir ? '-' : formatBytes(item.size);

      // Mode
      const tdMode = document.createElement('td');
      tdMode.textContent = item.mode ? '0' + item.mode.toString(8) : '-';

      // Modified
      const tdMod = document.createElement('td');
      tdMod.textContent = item.modified ? formatDate(item.modified) : '-';

      // Actions
      const tdAct = document.createElement('td');
      tdAct.className = 'row-actions';
      const itemRelPath = state.currentPath ? `${state.currentPath}/${item.name}` : item.name;

      if (!item.is_dir) {
        const dlBtn = createActionButton('Download', () => {
          const url = `/api/download?root_id=${encodeURIComponent(state.currentRoot)}&path=${encodeURIComponent(itemRelPath)}`;
          window.location.href = url;
        });
        tdAct.appendChild(dlBtn);
        if (isMediaFile(item.name)) {
          const prevBtn = createActionButton('Preview', () => openLightbox(itemRelPath));
          tdAct.appendChild(prevBtn);
        } else {
          const editBtn = createActionButton('Edit', () => viewOrEditFile(itemRelPath));
          tdAct.appendChild(editBtn);
        }
      }

      const chmodBtn = createActionButton('Permissions', () => openChmodModal(itemRelPath, item.mode));
      tdAct.appendChild(chmodBtn);

      const renameBtn = createActionButton('Rename', () => openRenameModal(item.name, itemRelPath));
      tdAct.appendChild(renameBtn);

      const trashBtn = createActionButton('Trash', () => moveToTrash(itemRelPath), 'btn-danger');
      tdAct.appendChild(trashBtn);

      tr.appendChild(tdName);
      tr.appendChild(tdSize);
      tr.appendChild(tdMode);
      tr.appendChild(tdMod);
      tr.appendChild(tdAct);
      el.fileTableBody.appendChild(tr);
    }
  }

  function renderSearchResults(data) {
    el.fileTableBody.innerHTML = '';
    const matches = data.matches || [];

    if (matches.length === 0) {
      el.fileTableBody.innerHTML = `<tr class="empty-row"><td colspan="5">No files matching "${escapeHtml(data.query || '')}"</td></tr>`;
      return;
    }

    for (const match of matches) {
      const tr = document.createElement('tr');
      const item = {...match.entry,is_dir:match.entry.file_type==='Directory'};
      const relPath = match.relative_path;

      // Name & Icon
      const tdName = document.createElement('td');
      const nameWrapper = document.createElement('a');
      nameWrapper.className = 'item-name';
      nameWrapper.href = '#';

      const icon = item.is_dir ? '📁' : getFileIcon(item.name);
      nameWrapper.innerHTML = `<span>${icon}</span> <span>${escapeHtml(relPath)}</span>`;
      nameWrapper.onclick = (e) => {
        e.preventDefault();
        if (item.is_dir) {
          loadDirectory(relPath);
        } else if (isMediaFile(item.name)) {
          openLightbox(relPath);
        } else {
          viewOrEditFile(relPath);
        }
      };
      tdName.appendChild(nameWrapper);

      // Size
      const tdSize = document.createElement('td');
      tdSize.textContent = item.is_dir ? '-' : formatBytes(item.size);

      // Mode
      const tdMode = document.createElement('td');
      tdMode.textContent = item.mode ? '0' + item.mode.toString(8) : '-';

      // Modified
      const tdMod = document.createElement('td');
      tdMod.textContent = item.modified ? formatDate(item.modified) : '-';

      // Actions
      const tdAct = document.createElement('td');
      tdAct.className = 'row-actions';

      if (!item.is_dir) {
        const dlBtn = createActionButton('Download', () => {
          const url = `/api/download?root_id=${encodeURIComponent(state.currentRoot)}&path=${encodeURIComponent(relPath)}`;
          window.location.href = url;
        });
        tdAct.appendChild(dlBtn);
        if (isMediaFile(item.name)) {
          const prevBtn = createActionButton('Preview', () => openLightbox(relPath));
          tdAct.appendChild(prevBtn);
        } else {
          const editBtn = createActionButton('Edit', () => viewOrEditFile(relPath));
          tdAct.appendChild(editBtn);
        }
      }

      const trashBtn = createActionButton('Trash', () => moveToTrash(relPath), 'btn-danger');
      tdAct.appendChild(trashBtn);

      tr.appendChild(tdName);
      tr.appendChild(tdSize);
      tr.appendChild(tdMode);
      tr.appendChild(tdMod);
      tr.appendChild(tdAct);
      el.fileTableBody.appendChild(tr);
    }
  }

  function createActionButton(text, onClick, extraClass = '') {
    const btn = document.createElement('button');
    btn.className = `btn btn-secondary btn-sm ${extraClass}`;
    btn.textContent = text;
    btn.onclick = (e) => {
      e.stopPropagation();
      onClick();
    };
    return btn;
  }

  // --- Media & Lightbox Preview ---
  function isImageFile(filename) {
    const ext = filename.split('.').pop().toLowerCase();
    return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico'].includes(ext);
  }

  function isVideoFile(filename) {
    const ext = filename.split('.').pop().toLowerCase();
    return ['mp4', 'webm', 'ogg'].includes(ext);
  }

  function isAudioFile(filename) {
    const ext = filename.split('.').pop().toLowerCase();
    return ['mp3', 'wav'].includes(ext);
  }

  function isPdfFile(filename) {
    return filename.toLowerCase().endsWith('.pdf');
  }

  function isMediaFile(filename) {
    return isImageFile(filename) || isVideoFile(filename) || isAudioFile(filename) || isPdfFile(filename);
  }

  function openLightbox(relPath) {
    el.lightboxTitle.textContent = relPath;
    const url = `/api/preview?root_id=${encodeURIComponent(state.currentRoot)}&path=${encodeURIComponent(relPath)}`;
    const dlUrl = `/api/download?root_id=${encodeURIComponent(state.currentRoot)}&path=${encodeURIComponent(relPath)}`;
    el.lightboxDownloadBtn.href = dlUrl;

    el.lightboxContent.innerHTML = '';
    if (isImageFile(relPath)) {
      const img = document.createElement('img');
      img.src = url;
      img.alt = relPath;
      el.lightboxContent.appendChild(img);
    } else if (isVideoFile(relPath)) {
      const video = document.createElement('video');
      video.src = url;
      video.controls = true;
      video.autoplay = false;
      el.lightboxContent.appendChild(video);
    } else if (isAudioFile(relPath)) {
      const audio = document.createElement('audio');
      audio.src = url;
      audio.controls = true;
      audio.autoplay = false;
      el.lightboxContent.appendChild(audio);
    } else if (isPdfFile(relPath)) {
      const iframe = document.createElement('iframe');
      iframe.src = url;
      iframe.style.width = '100%';
      iframe.style.height = '65vh';
      iframe.style.border = 'none';
      el.lightboxContent.appendChild(iframe);
    } else {
      el.lightboxContent.innerHTML = '<p class="muted">Cannot preview this file type directly.</p>';
    }

    el.lightboxModal.classList.remove('hidden');
  }

  function closeLightbox() {
    el.lightboxContent.innerHTML = '';
    el.lightboxModal.classList.add('hidden');
  }

  // --- File Editor ---
  function viewOrEditFile(relPath) {
    if (isMediaFile(relPath)) {
      openLightbox(relPath);
      return;
    }
    state.currentEditorFile = relPath;
    el.editorTitle.textContent = `Loading ${relPath}...`;
    el.editorTextarea.value = '';
    el.editorModal.classList.remove('hidden');
    sendWsRequest(MT.ReadTextFile, {
      root_id: state.currentRoot,
      relative_path: relPath,
      max_bytes: 128 * 1024,
    });
  }

  function openEditor(payload) {
    el.editorTitle.textContent = state.currentEditorFile;
    el.editorTextarea.value = payload.content || '';
    el.editorMetaHint.textContent = `${payload.content ? payload.content.length : 0} bytes | Modified: ${formatDate(payload.modified)}`;
    el.editorSaveBtn.onclick = () => {
      sendWsRequest(MT.SaveTextFile, {
        root_id: state.currentRoot,
        relative_path: state.currentEditorFile,
        content: el.editorTextarea.value,
        expected_modified: payload.modified,
      });
      el.editorModal.classList.add('hidden');
    };
  }

  // --- File Operations ---
  function moveToTrash(relPath) {
    if (!confirm(`Move "${relPath}" to trash?`)) return;
    sendWsRequest(MT.MoveToTrash, {
      root_id: state.currentRoot,
      relative_path: relPath,
    });
  }

  function openTrashModal() {
    el.trashTableBody.innerHTML = '<tr class="empty-row"><td colspan="4">Loading trash...</td></tr>';
    el.trashModal.classList.remove('hidden');
    sendWsRequest(MT.ListTrash, {
      root_id: state.currentRoot,
    });
  }

  function renderTrashList(payload) {
    el.trashTableBody.innerHTML = '';
    const items = payload.items || [];
    if (items.length === 0) {
      el.trashTableBody.innerHTML = '<tr class="empty-row"><td colspan="4">Trash bin is empty.</td></tr>';
      return;
    }

    for (const item of items) {
      const tr = document.createElement('tr');

      const tdName = document.createElement('td');
      tdName.innerHTML = `<span>🗑️</span> <span>${escapeHtml(item.display_name)}</span>`;

      const tdPath = document.createElement('td');
      tdPath.textContent = item.original_path || '-';

      const tdSize = document.createElement('td');
      tdSize.textContent = formatBytes(item.size);

      const tdAct = document.createElement('td');
      tdAct.className = 'row-actions';

      const restoreBtn = createActionButton('Restore', () => restoreFromTrash(item.trash_id));
      tdAct.appendChild(restoreBtn);

      const deleteBtn = createActionButton('Delete Permanently', () => deletePermanentlyFromTrash(item.trash_id), 'btn-danger');
      tdAct.appendChild(deleteBtn);

      tr.appendChild(tdName);
      tr.appendChild(tdPath);
      tr.appendChild(tdSize);
      tr.appendChild(tdAct);
      el.trashTableBody.appendChild(tr);
    }
  }

  function restoreFromTrash(trashId) {
    sendWsRequest(MT.RestoreFromTrash, {
      trash_id: trashId,
      target_root_id: state.currentRoot,
      target_relative_path: null,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function deletePermanentlyFromTrash(trashId) {
    if (!confirm('Are you sure you want to permanently delete this item? This action cannot be undone.')) return;
    sendWsRequest(MT.DeletePermanently, {
      root_id: state.currentRoot,
      relative_path: `.tealdrive-trash/files/${trashId}`,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function openRenameModal(oldName, relPath) {
    openPrompt('Rename Item', 'New Name', oldName, (newName) => {
      if (!newName || newName === oldName) return;
      const targetRel = state.currentPath ? `${state.currentPath}/${newName}` : newName;
      sendWsRequest(MT.RenameFile, {
        root_id: state.currentRoot,
        relative_path: relPath,
        new_name: newName,
      });
    });
  }

  function syncChmodFromCode() {
    const value=el.chmodOctalInput.value.trim();
    const valid=/^0?[0-7]{3}$/.test(value);
    el.chmodOctalInput.setAttribute('aria-invalid',String(!valid));
    el.chmodOctalInput.setCustomValidity(valid?'':'Enter three octal digits (0–7), optionally preceded by 0.');
    el.chmodApplyBtn.disabled=!valid;
    el.chmodError.textContent=valid?'':'Enter a valid code such as 0644 or 0755 (digits 0–7).';
    if(!valid){el.chmodPreview.textContent='';return null;}
    const mode=parseInt(value,8);
    for(const checkbox of el.chmodBits)checkbox.checked=Boolean(mode&Number(checkbox.dataset.bit));
    el.chmodPreview.textContent=el.chmodBits.map((checkbox,index)=>checkbox.checked?'rwx'[index%3]:'-').join('');
    return mode;
  }
  function syncChmodFromCheckboxes() {
    const mode=el.chmodBits.reduce((value,checkbox)=>value|(checkbox.checked?Number(checkbox.dataset.bit):0),0);
    el.chmodOctalInput.value='0'+mode.toString(8).padStart(3,'0');
    syncChmodFromCode();
  }
  function openChmodModal(relPath, currentMode) {
    state.chmodTarget = relPath;
    el.chmodTargetName.textContent = relPath;
    const mode=(currentMode??0o644)&0o777;
    el.chmodOctalInput.value='0'+mode.toString(8).padStart(3,'0');
    syncChmodFromCode();
    el.chmodModal.classList.remove('hidden');
  }

  // --- Prompts & Modals ---
  let promptCallback = null;
  function openPrompt(title, label, defaultValue, callback) {
    el.promptTitle.textContent = title;
    el.promptLabel.textContent = label;
    el.promptInput.value = defaultValue || '';
    promptCallback = callback;
    el.promptModal.classList.remove('hidden');
    el.promptInput.focus();
  }

  // --- Upload Flow ---
  async function handleFileUpload(file) {
    if(!file||state.uploading)return;
    const limit=state.protocolLimits?.max_in_flight_upload||0;
    if(file.size>limit){showToast(`File exceeds server upload limit (${formatBytes(limit)})`,'error');return;}
    state.uploading=true;let transfer=null;
    el.uploadProgressContainer.classList.remove('hidden');el.uploadFileName.textContent=file.name;
    try{
      const begin=await requestWs(MT.UploadBegin,{root_id:state.currentRoot,relative_path:state.currentPath,filename:file.name,total_size:file.size,
        chunk_size:state.protocolLimits.max_chunk_size,sha256:null});transfer=begin.transfer_id;
      let offset=0,index=0;
      while(offset<file.size){
        const bytes=new Uint8Array(await file.slice(offset,offset+begin.chunk_size).arrayBuffer());
        await requestWs(MT.UploadChunk,encodeChunk(transfer,index++,offset,bytes),true);
        offset+=bytes.length;const pct=Math.round(100*offset/file.size);el.uploadProgressBar.style.width=`${pct}%`;el.uploadPercent.textContent=`${pct}%`;
      }
      await requestWs(MT.UploadEnd,{transfer_id:transfer,total_size:file.size,sha256:null});transfer=null;
      showToast('Upload completed','success');loadDirectory(state.currentPath);
    }catch(error){showToast(error.message,'error');if(transfer&&state.wsConnected){try{await requestWs(MT.UploadAbort,{transfer_id:transfer,reason:'client error'});}catch{}}}
    finally{state.uploading=false;el.uploadProgressContainer.classList.add('hidden');el.fileInput.value='';}
  }

  // --- Session & HTTP Auth ---
  async function checkSession() {
    try {
      const resp = await fetch('/api/session', { credentials: 'same-origin' });
      if (resp.ok) {
        const data = await resp.json();
        if(data.authenticated===false){showLoginModal();return;}
        state.session = data;
        state.csrfToken = data.csrf_token || '';
        state.roots = data.roots || [];
        onSessionActive();
      } else {
        showLoginModal();
      }
    } catch(err) {
      showLoginModal();
    }
  }

  function showLoginModal() {
    el.loginModal.classList.remove('hidden');
    el.userInfo.classList.add('hidden');
  }

  function onSessionActive() {
    el.loginModal.classList.add('hidden');
    el.userInfo.classList.remove('hidden');
    el.settingsBtn.disabled=!state.session.is_admin;
    el.userBadge.textContent = `${state.session.username} (uid: ${state.session.uid})`;

    // Populate roots dropdown
    el.rootSelect.innerHTML = '';
    for (const r of state.roots) {
      const opt = document.createElement('option');
      opt.value = r.root_id;
      let label = r.root_id;
      if (r.quota_bytes) {
        const used = r.used_bytes != null ? formatBytes(r.used_bytes) : 'unknown';
        const total = formatBytes(r.quota_bytes);
        label += ` [Quota: ${used} / ${total}]`;
      }
      if (r.read_only) label += ' (read-only)';
      opt.textContent = label;
      el.rootSelect.appendChild(opt);
    }
    state.currentRoot = state.roots.length > 0 ? state.roots[0].root_id : null;

    connectWebSocket();
  }

  async function performLogin(username, password) {
    el.loginError.classList.add('hidden');
    try {
      const resp = await fetch('/api/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
        credentials: 'same-origin'
      });
      if (resp.ok) {
        await checkSession();
      } else {
        const err = await resp.json().catch(() => ({ error: 'Login failed' }));
        el.loginError.textContent = err.error || 'Invalid credentials';
        el.loginError.classList.remove('hidden');
      }
    } catch(err) {
      el.loginError.textContent = 'Connection error. Please try again.';
      el.loginError.classList.remove('hidden');
    }
  }

  async function performLogout() {
    try {
      await fetch('/api/logout', {
        method: 'POST',
        headers: { 'x-csrf-token': state.csrfToken },
        credentials: 'same-origin'
      });
    } catch(e) {}
    window.location.reload();
  }

  // --- Utility Functions ---
  function showToast(msg, type = 'info') {
    const t = document.createElement('div');
    t.className = `toast toast-${type}`;
    t.textContent = msg;
    el.toastContainer.appendChild(t);
    setTimeout(() => {
      t.style.opacity = '0';
      setTimeout(() => t.remove(), 200);
    }, 3500);
  }

  function formatBytes(b) {
    if (b === 0) return '0 B';
    if (!b) return '-';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(b) / Math.log(k));
    return parseFloat((b / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  function formatDate(epoch) {
    if (!epoch) return '-';
    const d = new Date(epoch * 1000);
    return d.toLocaleString();
  }

  function getFileIcon(name) {
    const ext = name.split('.').pop().toLowerCase();
    if (['txt', 'md', 'rs', 'js', 'json', 'toml', 'html', 'css', 'py'].includes(ext)) return '📄';
    if (['zip', 'tar', 'gz', 'tgz', 'rar', '7z'].includes(ext)) return '🗜️';
    if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'].includes(ext)) return '🖼️';
    if (['mp4', 'webm', 'ogg', 'mov', 'mkv'].includes(ext)) return '🎬';
    if (['mp3', 'wav', 'flac', 'aac'].includes(ext)) return '🎵';
    if (['pdf'].includes(ext)) return '📕';
    return '📄';
  }

  function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // --- Admin Settings & Dynamic Config ---
  let currentAdminConfig = null;

  async function openSettingsModal() {
    try {
      const resp = await fetch('/api/admin/config', { credentials: 'same-origin' });
      if (!resp.ok) {
        showToast('Failed to fetch server configuration', 'error');
        return;
      }
      const data = await resp.json();
      const cfg = data.config || data;
      currentAdminConfig = cfg;
      if (cfg && cfg.features) {
        el.cfgFeatEdit.checked = !!cfg.features.save_text_file;
        el.cfgFeatDelete.checked = !!cfg.features.delete_permanently;
        el.cfgFeatChmod.checked = !!cfg.features.chmod_file;
        el.cfgFeatArchive.checked = !!cfg.features.extract_archive;
      }
      el.cfgTrashRetention.value = cfg.trash_retention_days ?? 30;
      if (cfg.limits) {
        el.cfgMaxUploadBytes.value = Math.round((cfg.limits.max_in_flight_upload || 1024 * 1024) / (1024 * 1024));
        el.cfgMaxEditTextBytes.value = Math.round((cfg.limits.max_text_edit_size || cfg.limits.max_edit_text_bytes || 128 * 1024) / 1024);
        el.cfgMaxPreviewBytes.value = Math.round((cfg.limits.max_preview_size || 50 * 1024 * 1024) / (1024 * 1024));
      }
      if (cfg.rate_limit) {
        el.cfgRateLimitMaxFailures.value = cfg.rate_limit.max_login_failures ?? 5;
        el.cfgRateLimitLockoutSecs.value = cfg.rate_limit.lockout_duration_seconds ?? 300;
      }
      if (cfg.session) {
        el.cfgSessionIdleMins.value = Math.round((cfg.session.idle_timeout_seconds || 1800) / 60);
      }
      if (cfg.helper) {
        el.cfgHelperTimeoutSecs.value = cfg.helper.timeout_seconds ?? 10;
      }
      if (cfg.search) {
        el.cfgSearchMaxDepth.value = cfg.search.max_depth ?? 20;
        el.cfgSearchMaxResults.value = cfg.search.default_max_results ?? 100;
      }
      el.settingsModal.classList.remove('hidden');
    } catch(e) {
      showToast('Error connecting to admin config API', 'error');
    }
  }

  async function saveSettings() {
    const patch = {
      features: {
        ...(currentAdminConfig?.features||{}),
        save_text_file: el.cfgFeatEdit.checked,
        delete_permanently: el.cfgFeatDelete.checked,
        chmod_file: el.cfgFeatChmod.checked,
        extract_archive: el.cfgFeatArchive.checked,
      },
      trash_retention_days: currentAdminConfig?.trash_retention_days ?? null,
      limits: {
        ...(currentAdminConfig && currentAdminConfig.limits ? currentAdminConfig.limits : {}),
        max_in_flight_upload: (parseInt(el.cfgMaxUploadBytes.value,10)||1)*1024*1024,
        max_text_edit_size: (parseInt(el.cfgMaxEditTextBytes.value, 10) || 128) * 1024,
        max_preview_size: (parseInt(el.cfgMaxPreviewBytes.value, 10) || 50) * 1024 * 1024,
      },
      rate_limit: {
        max_login_failures: parseInt(el.cfgRateLimitMaxFailures.value, 10) || 5,
        lockout_duration_seconds: parseInt(el.cfgRateLimitLockoutSecs.value, 10) || 300,
      },
      session: {
        ...(currentAdminConfig && currentAdminConfig.session ? currentAdminConfig.session : {}),
        idle_timeout_seconds: (parseInt(el.cfgSessionIdleMins.value, 10) || 30) * 60,
      },
      helper: {
        timeout_seconds: parseInt(el.cfgHelperTimeoutSecs.value, 10) || 10,
      },
      search: {
        ...(currentAdminConfig && currentAdminConfig.search ? currentAdminConfig.search : {}),
        max_depth: parseInt(el.cfgSearchMaxDepth.value, 10) || 20,
        default_max_results: parseInt(el.cfgSearchMaxResults.value, 10) || 100,
      }
    };
    const persist = el.cfgPersistDisk.checked ? '?save=true' : '';
    try {
      const resp = await fetch('/api/admin/config' + persist, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json', 'X-CSRF-Token': state.csrfToken },
        body: JSON.stringify(patch),
        credentials: 'same-origin'
      });
      if (resp.ok) {
        showToast('Settings saved and hot-reloaded successfully!', 'success');
        el.settingsModal.classList.add('hidden');
      } else {
        const err = await resp.json().catch(() => ({ error: 'Failed to update config' }));
        showToast(err.error || 'Failed to update config', 'error');
      }
    } catch(e) {
      showToast('Network error saving settings', 'error');
    }
  }

  async function reloadSettingsFromDisk() {
    try {
      const resp = await fetch('/api/admin/config/reload', {
        method: 'POST',
        headers: {'X-CSRF-Token':state.csrfToken},
        credentials: 'same-origin'
      });
      if (resp.ok) {
        showToast('Config reloaded from disk!', 'success');
        await openSettingsModal();
      } else {
        const err = await resp.json().catch(() => ({ error: 'Reload failed' }));
        showToast(err.error || 'Failed to reload config', 'error');
      }
    } catch(e) {
      showToast('Network error reloading config', 'error');
    }
  }

  // --- Event Listeners Setup ---
  function setupEvents() {
    el.settingsBtn.onclick = openSettingsModal;
    el.settingsCloseBtn.onclick = () => el.settingsModal.classList.add('hidden');
    el.settingsCancelBtn.onclick = () => el.settingsModal.classList.add('hidden');
    el.cfgSaveBtn.onclick = saveSettings;
    el.cfgReloadBtn.onclick = reloadSettingsFromDisk;
    el.loginForm.onsubmit = (e) => {
      e.preventDefault();
      performLogin(el.usernameInput.value.trim(), el.passwordInput.value);
    };

    el.logoutBtn.onclick = performLogout;

    el.rootSelect.onchange = () => {
      state.currentRoot = el.rootSelect.value;
      loadDirectory('');
    };

    el.refreshBtn.onclick = () => loadDirectory(state.currentPath);

    el.newFolderBtn.onclick = () => {
      openPrompt('Create New Folder', 'Folder Name', '', (name) => {
        if (!name) return;
        sendWsRequest(MT.CreateFolder, {
          root_id: state.currentRoot,
          parent_relative_path: state.currentPath,
          folder_name: name,
        });
      });
    };

    el.newFileBtn.onclick = () => {
      openPrompt('Create New File', 'File Name (e.g. notes.txt)', '', (name) => {
        if (!name) return;
        const targetPath = state.currentPath ? `${state.currentPath}/${name}` : name;
        sendWsRequest(MT.SaveTextFile, {
          root_id: state.currentRoot,
          relative_path: targetPath,
          content: '',
        });
      });
    };

    el.uploadBtn.onclick = () => el.fileInput.click();
    el.fileInput.onchange = (e) => {
      if (e.target.files && e.target.files[0]) {
        handleFileUpload(e.target.files[0]);
      }
    };

    // Editor modal
    el.editorCloseBtn.onclick = () => el.editorModal.classList.add('hidden');
    el.editorCancelBtn.onclick = () => el.editorModal.classList.add('hidden');

    // Prompt modal
    el.promptCloseBtn.onclick = () => el.promptModal.classList.add('hidden');
    el.promptCancelBtn.onclick = () => el.promptModal.classList.add('hidden');
    el.promptForm.onsubmit = (e) => {
      e.preventDefault();
      el.promptModal.classList.add('hidden');
      if (promptCallback) promptCallback(el.promptInput.value.trim());
    };

    // Chmod modal
    el.chmodCloseBtn.onclick = () => el.chmodModal.classList.add('hidden');
    el.chmodCancelBtn.onclick = () => el.chmodModal.classList.add('hidden');
    el.chmodOctalInput.addEventListener('input',syncChmodFromCode);
    for(const checkbox of el.chmodBits)checkbox.addEventListener('change',syncChmodFromCheckboxes);
    el.chmodForm.onsubmit = async (e) => {
      e.preventDefault();
      const mode=syncChmodFromCode();
      if(mode===null)return;
      el.chmodApplyBtn.disabled=true;
      try {
        await requestWs(MT.ChmodFile,{root_id:state.currentRoot,relative_path:state.chmodTarget,mode});
        el.chmodModal.classList.add('hidden');
        showToast('Permissions updated','success');loadDirectory(state.currentPath);
      } catch(error) {el.chmodError.textContent=error.message;}
      finally {el.chmodApplyBtn.disabled=false;}
    };

    // Trash modal
    el.trashBtn.onclick = () => {
      openTrashModal();
    };
    el.trashCloseBtn.onclick = () => {
      el.trashModal.classList.add('hidden');
    };
    el.trashDoneBtn.onclick = () => {
      el.trashModal.classList.add('hidden');
    };

    // Lightbox modal
    el.lightboxCloseBtn.onclick = () => {
      closeLightbox();
    };
    el.lightboxDoneBtn.onclick = () => {
      closeLightbox();
    };

    // Global keyboard shortcuts (Escape to close modals)
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        if (!el.lightboxModal.classList.contains('hidden')) {
          closeLightbox();
        }
        if (!el.trashModal.classList.contains('hidden')) {
          el.trashModal.classList.add('hidden');
        }
        if (!el.editorModal.classList.contains('hidden')) {
          el.editorModal.classList.add('hidden');
        }
        if (!el.settingsModal.classList.contains('hidden')) {
          el.settingsModal.classList.add('hidden');
        }
      }
    });

    // Search
    let searchDebounceTimer = null;
    el.searchInput.oninput = (e) => {
      const q = e.target.value.trim();
      if (q) {
        el.searchClearBtn.classList.remove('hidden');
      } else {
        el.searchClearBtn.classList.add('hidden');
      }
      clearTimeout(searchDebounceTimer);
      if (!q) {
        loadDirectory(state.currentPath);
        return;
      }
      searchDebounceTimer = setTimeout(() => {
        el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Searching...</td></tr>';
        sendWsRequest(MT.SearchFiles, {
          root_id: state.currentRoot,
          base_path: state.currentPath || '',
          query: q,
          extension: null,
          max_results: 100,
          include_hidden: false,
        });
      }, 300);
    };

    el.searchClearBtn.onclick = () => {
      el.searchInput.value = '';
      el.searchClearBtn.classList.add('hidden');
      loadDirectory(state.currentPath);
    };
  }

  // Initialize
  setupEvents();
  checkSession();
})();
"##;
