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
        <h3>Change Permissions (chmod)</h3>
        <button class="modal-close" id="chmodCloseBtn">&times;</button>
      </div>
      <form id="chmodForm" class="modal-body">
        <p class="file-target-name" id="chmodTargetName"></p>
        <div class="form-group">
          <label for="chmodOctalInput">Octal Mode (e.g. 0644, 0755):</label>
          <input type="text" id="chmodOctalInput" class="form-input" pattern="0?[0-7]{3}" maxlength="4" required>
        </div>
        <div class="modal-actions">
          <button type="button" id="chmodCancelBtn" class="btn btn-secondary">Cancel</button>
          <button type="submit" class="btn btn-primary">Apply</button>
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
  };

  // --- MessagePack Pure JS Minimal Codec ---
  const MsgPack = {
    encode(val) {
      const parts = [];
      function push(u8) { parts.push(u8); }
      function pushBytes(arr) { parts.push(new Uint8Array(arr)); }
      function enc(v) {
        if (v === null || v === undefined) {
          push(new Uint8Array([0xc0]));
        } else if (typeof v === 'boolean') {
          push(new Uint8Array([v ? 0xc3 : 0xc2]));
        } else if (typeof v === 'number') {
          if (Number.isInteger(v)) {
            if (v >= 0 && v <= 0x7f) {
              push(new Uint8Array([v]));
            } else if (v >= -32 && v < 0) {
              push(new Uint8Array([v & 0xff]));
            } else if (v >= 0 && v <= 0xffff) {
              push(new Uint8Array([0xcd, (v >> 8) & 0xff, v & 0xff]));
            } else if (v >= 0 && v <= 0xffffffff) {
              const b = new Uint8Array(5);
              b[0] = 0xce;
              new DataView(b.buffer).setUint32(1, v, false);
              push(b);
            } else {
              const b = new Uint8Array(5);
              b[0] = 0xd2;
              new DataView(b.buffer).setInt32(1, v, false);
              push(b);
            }
          } else {
            const b = new Uint8Array(9);
            b[0] = 0xcb;
            new DataView(b.buffer).setFloat64(1, v, false);
            push(b);
          }
        } else if (typeof v === 'string') {
          const strBytes = new TextEncoder().encode(v);
          const len = strBytes.length;
          if (len <= 31) {
            push(new Uint8Array([0xa0 | len]));
          } else if (len <= 0xff) {
            push(new Uint8Array([0xd9, len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xda, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdb;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          push(strBytes);
        } else if (v instanceof Uint8Array) {
          const len = v.length;
          if (len <= 0xff) {
            push(new Uint8Array([0xc4, len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xc5, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xc6;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          push(v);
        } else if (Array.isArray(v)) {
          const len = v.length;
          if (len <= 15) {
            push(new Uint8Array([0x90 | len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xdc, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdd;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          for (let i = 0; i < len; i++) enc(v[i]);
        } else if (typeof v === 'object') {
          const keys = Object.keys(v);
          const len = keys.length;
          if (len <= 15) {
            push(new Uint8Array([0x80 | len]));
          } else if (len <= 0xffff) {
            push(new Uint8Array([0xde, (len >> 8) & 0xff, len & 0xff]));
          } else {
            const b = new Uint8Array(5);
            b[0] = 0xdf;
            new DataView(b.buffer).setUint32(1, len, false);
            push(b);
          }
          for (const k of keys) {
            enc(k);
            enc(v[k]);
          }
        }
      }
      enc(val);
      const totalLen = parts.reduce((acc, p) => acc + p.length, 0);
      const res = new Uint8Array(totalLen);
      let offset = 0;
      for (const p of parts) {
        res.set(p, offset);
        offset += p.length;
      }
      return res;
    },

    decode(bytes) {
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      let pos = 0;
      function read() {
        if (pos >= bytes.length) return null;
        const b = bytes[pos++];
        if (b <= 0x7f) return b;
        if (b >= 0xe0) return b - 0x100;
        if ((b & 0xe0) === 0xa0) {
          const len = b & 0x1f;
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if ((b & 0xf0) === 0x90) {
          const len = b & 0x0f;
          const arr = [];
          for (let i = 0; i < len; i++) arr.push(read());
          return arr;
        }
        if ((b & 0xf0) === 0x80) {
          const len = b & 0x0f;
          const obj = {};
          for (let i = 0; i < len; i++) {
            const k = read();
            obj[k] = read();
          }
          return obj;
        }
        if (b === 0xc0) return null;
        if (b === 0xc2) return false;
        if (b === 0xc3) return true;
        if (b === 0xc4) {
          const len = bytes[pos++];
          const bin = bytes.subarray(pos, pos + len);
          pos += len;
          return bin;
        }
        if (b === 0xc5) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const bin = bytes.subarray(pos, pos + len);
          pos += len;
          return bin;
        }
        if (b === 0xcc) return bytes[pos++];
        if (b === 0xcd) {
          const v = view.getUint16(pos, false);
          pos += 2;
          return v;
        }
        if (b === 0xce) {
          const v = view.getUint32(pos, false);
          pos += 4;
          return v;
        }
        if (b === 0xd0) return view.getInt8(pos++);
        if (b === 0xd1) {
          const v = view.getInt16(pos, false);
          pos += 2;
          return v;
        }
        if (b === 0xd2) {
          const v = view.getInt32(pos, false);
          pos += 4;
          return v;
        }
        if (b === 0xd9) {
          const len = bytes[pos++];
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if (b === 0xda) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const s = new TextDecoder().decode(bytes.subarray(pos, pos + len));
          pos += len;
          return s;
        }
        if (b === 0xdc) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const arr = [];
          for (let i = 0; i < len; i++) arr.push(read());
          return arr;
        }
        if (b === 0xde) {
          const len = view.getUint16(pos, false);
          pos += 2;
          const obj = {};
          for (let i = 0; i < len; i++) {
            const k = read();
            obj[k] = read();
          }
          return obj;
        }
        return null;
      }
      return read();
    }
  };

  // --- TdrvFrame Binary Protocol ---
  // Header: Magic(4: 'TDRV') + Version(2: 1) + MessageType(2) + Flags(1: 0) + Reserved(1: 0) + HeaderLen(2: 16) + PayloadLen(4)
  const MAGIC = 0x54445256; // "TDRV"

  function encodeFrame(messageType, payloadObj) {
    const payloadBytes = MsgPack.encode(payloadObj);
    const header = new Uint8Array(16);
    const view = new DataView(header.buffer);
    view.setUint32(0, MAGIC, false);
    view.setUint16(4, 1, false); // version 1
    view.setUint16(6, messageType, false);
    view.setUint8(8, 0); // flags
    view.setUint8(9, 0); // reserved
    view.setUint16(10, 16, false); // header length
    view.setUint32(12, payloadBytes.length, false);

    const frame = new Uint8Array(16 + payloadBytes.length);
    frame.set(header, 0);
    frame.set(payloadBytes, 16);
    return frame;
  }

  function decodeFrame(arrayBuffer) {
    const bytes = new Uint8Array(arrayBuffer);
    if (bytes.length < 16) return null;
    const view = new DataView(bytes.buffer, bytes.byteOffset, 16);
    const magic = view.getUint32(0, false);
    if (magic !== MAGIC) return null;
    const version = view.getUint16(4, false);
    const messageType = view.getUint16(6, false);
    const headerLen = view.getUint16(10, false);
    const payloadLen = view.getUint32(12, false);
    const payloadBytes = bytes.subarray(headerLen, headerLen + payloadLen);
    const payload = MsgPack.decode(payloadBytes);
    return { version, messageType, payload };
  }

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

    ws.onopen = function() {
      // Send ClientHello (0x0100)
      const hello = {
        client_version: 1,
        limits: {
          max_directory_page_size: 500,
          max_text_edit_size: 2 * 1024 * 1024,
          max_chunk_size: 1024 * 1024,
          max_download_helper_bytes: 50 * 1024 * 1024,
        }
      };
      ws.send(encodeFrame(0x0100, hello));
    };

    ws.onmessage = function(ev) {
      const frame = decodeFrame(ev.data);
      if (!frame) return;
      handleServerFrame(frame);
    };

    ws.onclose = function() {
      state.wsConnected = false;
      el.wsStatusDot.className = 'status-dot disconnected';
      el.wsStatusText.textContent = 'Disconnected';
      setTimeout(connectWebSocket, 3000);
    };

    ws.onerror = function() {
      state.wsConnected = false;
      el.wsStatusDot.className = 'status-dot disconnected';
      el.wsStatusText.textContent = 'Connection error';
    };
  }

  function handleServerFrame(frame) {
    const { messageType, payload } = frame;
    switch(messageType) {
      case 0x0101: // ServerHello
        state.wsConnected = true;
        el.wsStatusDot.className = 'status-dot connected';
        el.wsStatusText.textContent = 'Connected (Secure IPC)';
        loadDirectory(state.currentPath);
        break;
      case 0x0280: // DirectoryList
        renderDirectoryList(payload);
        break;
      case 0x0108: // SearchResults
        renderSearchResults(payload);
        break;
      case 0x010A: // TrashList
        renderTrashList(payload);
        break;
      case 0x0282: // TextFileContent
        openEditor(payload);
        break;
      case 0x0001: // OperationDone
        showToast(payload.message || 'Operation succeeded', 'success');
        loadDirectory(state.currentPath);
        break;
      case 0x0002: // OperationFailed
        showToast(payload.reason || 'Operation failed', 'error');
        break;
      case 0x0003: // Error
        showToast(payload.safe_message || 'Server error', 'error');
        break;
      default:
        console.log('Frame received:', messageType, payload);
    }
  }

  function sendWsRequest(messageType, payload) {
    if (!state.ws || state.ws.readyState !== WebSocket.OPEN) {
      showToast('WebSocket is not connected', 'error');
      return;
    }
    state.ws.send(encodeFrame(messageType, payload));
  }

  // --- Directory Listing & Navigation ---
  function loadDirectory(path) {
    state.currentPath = path || '';
    renderBreadcrumbs();
    el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Loading files...</td></tr>';
    sendWsRequest(0x0200, {
      root_id: state.currentRoot,
      relative_path: state.currentPath,
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
    const entries = data.entries || [];

    if (entries.length === 0) {
      el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Empty directory</td></tr>';
      return;
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

      const chmodBtn = createActionButton('Mode', () => openChmodModal(itemRelPath, item.mode));
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
      const item = match.entry;
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
    sendWsRequest(0x0202, {
      root_id: state.currentRoot,
      relative_path: relPath,
      max_bytes: 1024 * 1024,
    });
  }

  function openEditor(payload) {
    el.editorTitle.textContent = state.currentEditorFile;
    el.editorTextarea.value = payload.content || '';
    el.editorMetaHint.textContent = `${payload.content ? payload.content.length : 0} bytes | Modified: ${formatDate(payload.modified)}`;
    el.editorSaveBtn.onclick = () => {
      sendWsRequest(0x0203, {
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
    sendWsRequest(0x0304, {
      root_id: state.currentRoot,
      relative_path: relPath,
    });
  }

  function openTrashModal() {
    el.trashTableBody.innerHTML = '<tr class="empty-row"><td colspan="4">Loading trash...</td></tr>';
    el.trashModal.classList.remove('hidden');
    sendWsRequest(0x0109, {
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
    sendWsRequest(0x0305, {
      trash_id: trashId,
      target_root_id: state.currentRoot,
      target_relative_path: null,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function deletePermanentlyFromTrash(trashId) {
    if (!confirm('Are you sure you want to permanently delete this item? This action cannot be undone.')) return;
    sendWsRequest(0x0306, {
      root_id: state.currentRoot,
      relative_path: `.tealdrive-trash/files/${trashId}`,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function openRenameModal(oldName, relPath) {
    openPrompt('Rename Item', 'New Name', oldName, (newName) => {
      if (!newName || newName === oldName) return;
      const targetRel = state.currentPath ? `${state.currentPath}/${newName}` : newName;
      sendWsRequest(0x0301, {
        root_id: state.currentRoot,
        relative_path: relPath,
        target_relative_path: targetRel,
      });
    });
  }

  function openChmodModal(relPath, currentMode) {
    state.chmodTarget = relPath;
    el.chmodTargetName.textContent = relPath;
    el.chmodOctalInput.value = currentMode ? '0' + currentMode.toString(8) : '0644';
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
    if (!file) return;
    const chunkSize = 512 * 1024;
    const totalBytes = file.size;
    const relPath = state.currentPath ? `${state.currentPath}/${file.name}` : file.name;

    el.uploadProgressContainer.classList.remove('hidden');
    el.uploadFileName.textContent = `Uploading ${file.name}...`;
    el.uploadProgressBar.style.width = '0%';
    el.uploadPercent.textContent = '0%';

    // Read full file for SHA-256
    const buffer = await file.arrayBuffer();
    const digest = await crypto.subtle.digest('SHA-256', buffer);
    const sha256 = Array.from(new Uint8Array(digest)).map(b => b.toString(16).padStart(2, '0')).join('');

    // 1. UploadBegin
    sendWsRequest(0x0400, {
      root_id: state.currentRoot,
      relative_path: relPath,
      total_bytes: totalBytes,
      chunk_size: chunkSize,
      sha256: sha256,
      expected_mode: 0o644,
    });

    // 2. UploadChunk stream
    let offset = 0;
    while (offset < totalBytes) {
      const slice = buffer.slice(offset, Math.min(offset + chunkSize, totalBytes));
      sendWsRequest(0x0401, {
        root_id: state.currentRoot,
        relative_path: relPath,
        offset: offset,
        data: new Uint8Array(slice),
      });
      offset += slice.byteLength;
      const pct = Math.min(100, Math.round((offset / totalBytes) * 100));
      el.uploadProgressBar.style.width = `${pct}%`;
      el.uploadPercent.textContent = `${pct}%`;
    }

    // 3. UploadEnd
    sendWsRequest(0x0402, {
      root_id: state.currentRoot,
      relative_path: relPath,
    });

    setTimeout(() => {
      el.uploadProgressContainer.classList.add('hidden');
      loadDirectory(state.currentPath);
    }, 500);
  }

  // --- Session & HTTP Auth ---
  async function checkSession() {
    try {
      const resp = await fetch('/api/session', { credentials: 'same-origin' });
      if (resp.ok) {
        const data = await resp.json();
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
    el.userBadge.textContent = `${state.session.username} (uid: ${state.session.uid})`;

    // Populate roots dropdown
    el.rootSelect.innerHTML = '';
    for (const r of state.roots) {
      const opt = document.createElement('option');
      opt.value = r.root_id;
      let label = r.root_id;
      if (r.quota_bytes) {
        const used = r.used_bytes ? formatBytes(r.used_bytes) : '0 B';
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

  // --- Event Listeners Setup ---
  function setupEvents() {
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
        sendWsRequest(0x0300, {
          root_id: state.currentRoot,
          relative_path: state.currentPath,
          name: name,
        });
      });
    };

    el.newFileBtn.onclick = () => {
      openPrompt('Create New File', 'File Name (e.g. notes.txt)', '', (name) => {
        if (!name) return;
        const targetPath = state.currentPath ? `${state.currentPath}/${name}` : name;
        sendWsRequest(0x0203, {
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
    el.chmodForm.onsubmit = (e) => {
      e.preventDefault();
      const mode = parseInt(el.chmodOctalInput.value.trim(), 8);
      if (!isNaN(mode)) {
        sendWsRequest(0x0307, {
          root_id: state.currentRoot,
          relative_path: state.chmodTarget,
          mode: mode,
        });
      }
      el.chmodModal.classList.add('hidden');
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
        sendWsRequest(0x0107, {
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
