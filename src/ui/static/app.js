
// TealDrive Frontend Client SPA
(function() {
  'use strict';

  const t = (value,params={}) => window.TealDriveI18n?.t(value,params) ?? value.replace(/\{(\w+)\}/g,(match,key)=>String(params[key]??match));
  const label = (node,value,params={}) => {
    node.textContent=t(value,params);
    if(node.dataset) {node.dataset.i18n=value;node.dataset.i18nValues=JSON.stringify(params);}
  };
  // State
  let state = {
    session: null,
    csrfToken: '',
    roots: [],
    currentRoot: null,
    currentPath: '',
    wsConnected: false,
    serverShuttingDown: false,
    shutdownPreviousFocus: null,
    currentEditorFile: null,
    editorReadGeneration: 0,
    chmodTarget: null,
    chmodRootId: null,
    listingGeneration: 0,
    chmodOriginalMode: 0,
    chmodIsDirectory: false,
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
    shutdownModal: document.getElementById('shutdownModal'),
    shutdownTitle: document.getElementById('shutdownTitle'),
    shutdownMessage: document.getElementById('shutdownMessage'),
    shutdownDismissBtn: document.getElementById('shutdownDismissBtn'),
    shutdownRetryBtn: document.getElementById('shutdownRetryBtn'),
    loginModal: document.getElementById('loginModal'),
    loginForm: document.getElementById('loginForm'),
    registerWeb: document.getElementById('registerWeb'),
    webPasswordConfirm: document.getElementById('webPasswordConfirm'),
    webPasswordGroup: document.getElementById('webPasswordGroup'),
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
    chmodWarning: document.getElementById('chmodWarning'),
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
    cfgUsersStatus: document.getElementById('cfgUsersStatus'),
    cfgAllowedUsers: document.getElementById('cfgAllowedUsers'),
    cfgTrustedProxies: document.getElementById('cfgTrustedProxies'),
    cfgClientIpHeader: document.getElementById('cfgClientIpHeader'),
    cfgAuditHealth: document.getElementById('cfgAuditHealth'),
    cfgAuditPath: document.getElementById('cfgAuditPath'),
    cfgAuditMaxMB: document.getElementById('cfgAuditMaxMB'),
    cfgAuditArchives: document.getElementById('cfgAuditArchives'),
    cfgAuditDays: document.getElementById('cfgAuditDays'),
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

  let filesModule = null;
  async function connectWebSocket() {
    el.wsStatusDot.className = 'status-dot';
    el.wsStatusText.textContent = t('Connecting...');
    try {
      const client = await getAuthClient();
      const info = client.getInfo();
      state.protocolLimits = info.limits;
      state.serverShuttingDown = false;
      state.wsConnected = true;
      await client.files.refreshState();
      const location = client.files.getState();
      state.currentRoot = location.rootId;
      state.currentPath = (location.currentDirectory ?? '/').replace(/^\//,'');
      el.rootSelect.value = state.currentRoot;
      el.wsStatusDot.className = 'status-dot connected';
      el.wsStatusText.textContent = t('Connected');
      loadDirectory(state.currentPath);
      client.files.quota.get().catch(()=>{});
    } catch (error) { state.wsConnected=false; showToast(error.message,'error'); }
  }
  async function requestFile(action, payload) {
    if (state.serverShuttingDown) throw Error('Server is shutting down. New actions are paused.');
    const client = await getAuthClient();
    filesModule ??= import('/static/files.js');
    return (await filesModule).fileAction(client, action, payload);
  }
  function runFileAction(action,payload) {
    const viewGeneration=['ListDirectory','SearchFiles'].includes(action)?++state.listingGeneration:null;
    return requestFile(action,payload).then(result=>{
      if(viewGeneration!==null && viewGeneration!==state.listingGeneration)return result;
      if(action==='ListDirectory') renderDirectoryList(result);
      else if(action==='ReadTextFile') openEditor(result);
      else if(action==='SearchFiles') renderSearchResults(result);
      else if(action==='ListTrash') renderTrashList(result);
      else { showToast('Operation completed','success');loadDirectory(state.currentPath); }
      return result;
    }).catch(async error=>{
      if(viewGeneration!==null && viewGeneration!==state.listingGeneration)return;
      if(action==='ListDirectory' && ['NOT_FOUND','NOT_A_DIRECTORY'].includes(error.code)) {
        try {
          const client=await getAuthClient();
          await client.files.refreshState();
          const location=client.files.getState();
          state.currentRoot=location.rootId;
          state.currentPath=(location.currentDirectory??'/').replace(/^\//,'');
          el.rootSelect.value=state.currentRoot;
          loadDirectory(state.currentPath);
          return;
        } catch(failure) { showToast(failure.message,'error');return; }
      }
      showToast(error.message,'error');
    });
  }

  function showShutdownNotice(graceSeconds = 30) {
    const firstNotice = !state.serverShuttingDown;
    state.serverShuttingDown = true;
    state.wsConnected = false;
    el.wsStatusDot.className = 'status-dot disconnected';
    el.wsStatusText.textContent = t('Server shutting down');
    el.shutdownTitle.textContent = t('Server is shutting down');
    const grace = Number.isFinite(graceSeconds) && graceSeconds >= 0 ? graceSeconds : 30;
    el.shutdownMessage.textContent = t('New actions are paused. Running operations have up to {seconds} seconds to finish; incomplete uploads will need to be retried. Keep a copy of any unsaved changes. Sign in again after the server restarts.',{seconds:grace});
    if (firstNotice) {
      state.shutdownPreviousFocus = document.activeElement;
      el.shutdownModal.classList.remove('hidden');
      el.shutdownDismissBtn.focus();
    }
  }

  function dismissShutdownNotice() {
    el.shutdownModal.classList.add('hidden');
    state.shutdownPreviousFocus?.focus();
  }

  async function checkServerAvailability() {
    el.shutdownRetryBtn.disabled = true;
    try {
      const response = await fetch('/api/health', {cache: 'no-store'});
      if (!response.ok || (await response.text()).trim() !== 'ok') throw new Error('Server is still unavailable');
      state.serverShuttingDown = false;
      el.shutdownModal.classList.add('hidden');
      await checkSession();
    } catch(error) {
      el.shutdownMessage.textContent = t('The server is still unavailable. Keep this page open to preserve unsaved changes and try again shortly.');
    } finally {
      el.shutdownRetryBtn.disabled = false;
    }
  }

  // --- Directory Listing & Navigation ---
  function loadDirectory(path, cursor = null) {
    if (state.serverShuttingDown) return;
    state.currentPath = path || '';
    renderBreadcrumbs();
    el.fileTableBody.innerHTML = '<tr class="empty-row"><td colspan="5">Loading files...</td></tr>';
    runFileAction('ListDirectory', {
      root_id: state.currentRoot,
      relative_path: state.currentPath,cursor,limit:state.protocolLimits.defaultDirectoryPageSize,include_hidden:state.roots.find(root=>root.root_id===state.currentRoot)?.hidden_files_allowed??false,sort:"NameAsc",filter:{query:null,file_kind:null,filter:"All"},
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
      sep.textContent = t('/');
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
      const previous=document.createElement('button');previous.textContent=t('First page');previous.onclick=()=>loadDirectory(state.currentPath);
      const next=document.createElement('button');next.textContent=t('Next page');next.onclick=()=>loadDirectory(state.currentPath,data.next_cursor);
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
      tdName.setAttribute('data-label', 'Name');
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
      tdSize.setAttribute('data-label', 'Size');
      tdSize.textContent = item.is_dir ? '-' : formatBytes(item.size);

      // Mode
      const tdMode = document.createElement('td');
      tdMode.setAttribute('data-label', 'Mode');
      tdMode.textContent = item.mode != null ? '0' + (item.mode & 0o7777).toString(8).padStart(3, '0') : '-';

      // Modified
      const tdMod = document.createElement('td');
      tdMod.setAttribute('data-label', 'Modified');
      tdMod.textContent = item.modified ? formatDate(item.modified) : '-';

      // Actions
      const tdAct = document.createElement('td');
      tdAct.className = 'col-actions';
      tdAct.setAttribute('data-label', 'Actions');
      const actionGroup = document.createElement('div');
      actionGroup.className = 'row-actions';
      tdAct.appendChild(actionGroup);
      const itemRelPath = state.currentPath ? `${state.currentPath}/${item.name}` : item.name;

      if (!item.is_dir) {
        const dlBtn = createActionButton('Download', async () => {
          const client = await getAuthClient();
          const url = client.files.getDownloadUrl({rootId:state.currentRoot,path:itemRelPath});
          window.location.href = url;
        });
        actionGroup.appendChild(dlBtn);
        if (isMediaFile(item.name)) {
          const prevBtn = createActionButton('Preview', () => openLightbox(itemRelPath));
          actionGroup.appendChild(prevBtn);
        } else {
          const editBtn = createActionButton('Edit', () => viewOrEditFile(itemRelPath));
          actionGroup.appendChild(editBtn);
        }
      }

      const chmodBtn = createActionButton('Permissions', () => openChmodModal(itemRelPath, item.mode, item.is_dir));
      actionGroup.appendChild(chmodBtn);

      const renameBtn = createActionButton('Rename', () => openRenameModal(item.name, itemRelPath));
      actionGroup.appendChild(renameBtn);

      const trashBtn = createActionButton('Trash', () => moveToTrash(itemRelPath), 'btn-danger');
      actionGroup.appendChild(trashBtn);

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
      tdName.setAttribute('data-label', 'Name');
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
      tdSize.setAttribute('data-label', 'Size');
      tdSize.textContent = item.is_dir ? '-' : formatBytes(item.size);

      // Mode
      const tdMode = document.createElement('td');
      tdMode.setAttribute('data-label', 'Mode');
      tdMode.textContent = item.mode != null ? '0' + (item.mode & 0o7777).toString(8).padStart(3, '0') : '-';

      // Modified
      const tdMod = document.createElement('td');
      tdMod.setAttribute('data-label', 'Modified');
      tdMod.textContent = item.modified ? formatDate(item.modified) : '-';

      // Actions
      const tdAct = document.createElement('td');
      tdAct.className = 'col-actions';
      tdAct.setAttribute('data-label', 'Actions');
      const actionGroup = document.createElement('div');
      actionGroup.className = 'row-actions';
      tdAct.appendChild(actionGroup);

      if (!item.is_dir) {
        const dlBtn = createActionButton('Download', async () => {
          const client = await getAuthClient();
          const url = client.files.getDownloadUrl({rootId:state.currentRoot,path:relPath});
          window.location.href = url;
        });
        actionGroup.appendChild(dlBtn);
        if (isMediaFile(item.name)) {
          const prevBtn = createActionButton('Preview', () => openLightbox(relPath));
          actionGroup.appendChild(prevBtn);
        } else {
          const editBtn = createActionButton('Edit', () => viewOrEditFile(relPath));
          actionGroup.appendChild(editBtn);
        }
      }

      const trashBtn = createActionButton('Trash', () => moveToTrash(relPath), 'btn-danger');
      actionGroup.appendChild(trashBtn);

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
    label(btn,text);
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

  async function openLightbox(relPath) {
    const client=await getAuthClient();
    el.lightboxTitle.textContent = relPath;
    const url = client.files.getPreviewUrl({rootId:state.currentRoot,path:relPath});
    const dlUrl = client.files.getDownloadUrl({rootId:state.currentRoot,path:relPath});
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
  async function viewOrEditFile(relPath) {
    if (isMediaFile(relPath)) {
      openLightbox(relPath);
      return;
    }
    const generation = ++state.editorReadGeneration;
    const rootId = state.currentRoot;
    try {
      const payload = await requestFile('ReadTextFile',{root_id:rootId,relative_path:relPath});
      if (generation !== state.editorReadGeneration) return;
      state.currentEditorFile=relPath;
      openEditor(payload);
      el.editorModal.classList.remove('hidden');
    } catch(error) { showToast(error.message,'error'); }
  }

  function openEditor(payload) {
    el.editorTitle.textContent = state.currentEditorFile;
    el.editorTextarea.value = payload.content || '';
    el.editorMetaHint.textContent = `${payload.content ? payload.content.length : 0} bytes | Modified: ${formatDate(payload.modified)}`;
    el.editorSaveBtn.onclick = async () => {
      el.editorSaveBtn.disabled=true;
      try {
        await requestFile('SaveTextFile', {
          root_id: payload.root_id,
          relative_path: payload.relative_path,
          content: el.editorTextarea.value,
          expected_modified: payload.modified,
          expected_revision: payload.revision || null,
        });
        el.editorModal.classList.add('hidden');
        showToast('File saved','success');loadDirectory(state.currentPath);
      } catch(error) {
        el.editorMetaHint.textContent=t(error.message);
        showToast(error.message,'error');
      } finally {el.editorSaveBtn.disabled=false;}
    };
  }

  // --- File Operations ---
  function moveToTrash(relPath) {
    if (!confirm(`Move "${relPath}" to trash?`)) return;
    runFileAction('MoveToTrash', {
      root_id: state.currentRoot,
      relative_path: relPath,
    });
  }

  function openTrashModal() {
    el.trashTableBody.innerHTML = '<tr class="empty-row"><td colspan="4">Loading trash...</td></tr>';
    el.trashModal.classList.remove('hidden');
    runFileAction('ListTrash', {
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
      tdName.setAttribute('data-label', 'Name');
      tdName.innerHTML = `<span>🗑️</span> <span>${escapeHtml(item.display_name)}</span>`;

      const tdPath = document.createElement('td');
      tdPath.setAttribute('data-label', 'Original path');
      tdPath.textContent = item.original_path || '-';

      const tdSize = document.createElement('td');
      tdSize.setAttribute('data-label', 'Size');
      tdSize.textContent = formatBytes(item.size);

      const tdAct = document.createElement('td');
      tdAct.className = 'col-actions';
      tdAct.setAttribute('data-label', 'Actions');
      const actionGroup = document.createElement('div');
      actionGroup.className = 'row-actions';
      tdAct.appendChild(actionGroup);

      const restoreBtn = createActionButton('Restore', () => restoreFromTrash(item.trash_id));
      actionGroup.appendChild(restoreBtn);

      const deleteBtn = createActionButton('Delete Permanently', () => deletePermanentlyFromTrash(item.trash_id), 'btn-danger');
      actionGroup.appendChild(deleteBtn);

      tr.appendChild(tdName);
      tr.appendChild(tdPath);
      tr.appendChild(tdSize);
      tr.appendChild(tdAct);
      el.trashTableBody.appendChild(tr);
    }
  }

  function restoreFromTrash(trashId) {
    runFileAction('RestoreFromTrash', {
      trash_id: trashId,
      target_root_id: state.currentRoot,
      target_relative_path: null,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function deletePermanentlyFromTrash(trashId) {
    if (!confirm(t('Are you sure you want to permanently delete this item? This action cannot be undone.'))) return;
    runFileAction('DeletePermanently', {
      root_id: state.currentRoot,
      relative_path: `.tealdrive-trash/files/${trashId}`,
    });
    setTimeout(() => openTrashModal(), 200);
  }

  function openRenameModal(oldName, relPath) {
    openPrompt('Rename Item', 'New Name', oldName, (newName) => {
      if (!newName || newName === oldName) return;
      const targetRel = state.currentPath ? `${state.currentPath}/${newName}` : newName;
      runFileAction('RenameFile', {
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
    const messages=[];
    if((mode&0o002)&&!(state.chmodOriginalMode&0o002))messages.push('You are enabling write access for other users.');
    const removed=(state.chmodOriginalMode&0o555)&~mode;
    if(removed)messages.push(state.chmodIsDirectory?'Removing read or traversal permissions may prevent services from accessing this directory.':'Removing read or execute permissions may prevent applications from using this file.');
    if(state.chmodOriginalMode&0o7000)messages.push('Special permission bits will be removed; this panel cannot set them.');
    el.chmodWarning.textContent=messages.map(message=>t(message)).join(' ');
    return mode;
  }
  function syncChmodFromCheckboxes() {
    const mode=el.chmodBits.reduce((value,checkbox)=>value|(checkbox.checked?Number(checkbox.dataset.bit):0),0);
    el.chmodOctalInput.value='0'+mode.toString(8).padStart(3,'0');
    syncChmodFromCode();
  }
  function openChmodModal(relPath, currentMode, isDirectory = false) {
    state.chmodOriginalMode=(currentMode??0o644)&0o7777;
    state.chmodIsDirectory=isDirectory;
    state.chmodTarget = relPath;
    state.chmodRootId = state.currentRoot;
    el.chmodTargetName.textContent = relPath;
    const mode=(currentMode??0o644)&0o777;
    el.chmodOctalInput.value='0'+mode.toString(8).padStart(3,'0');
    syncChmodFromCode();
    el.chmodModal.classList.remove('hidden');
  }

  // --- Prompts & Modals ---
  let promptCallback = null;
  function openPrompt(title, promptLabel, defaultValue, callback) {
    label(el.promptTitle,title);
    label(el.promptLabel,promptLabel);
    el.promptInput.value = defaultValue || '';
    promptCallback = callback;
    el.promptModal.classList.remove('hidden');
    el.promptInput.focus();
  }

  // --- Upload Flow ---
  let uploadController = null;
  async function handleFileUpload(file) {
    if (!file || state.uploading) return;
    state.uploading = true;
    uploadController = new AbortController();
    document.getElementById('uploadCancelBtn').disabled = false;
    el.uploadProgressBar.style.width = '0%';
    el.uploadPercent.textContent = '0%';
    el.uploadProgressContainer.classList.remove('hidden');
    el.uploadFileName.textContent = file.name;
    try {
      const client = await getAuthClient();
      await client.files.upload({rootId:state.currentRoot,directory:state.currentPath,file,signal:uploadController.signal,onProgress:progress=>{
        const percentage = progress.totalBytes ? Math.round(100*progress.transferredBytes/progress.totalBytes) : 100;
        el.uploadProgressBar.style.width = `${percentage}%`;
        el.uploadPercent.textContent = `${percentage}%`;
      }});
      showToast('Upload completed','success'); loadDirectory(state.currentPath);
    } catch (error) {
      const messages = {
        FILE_TOO_LARGE: t('File exceeds the server upload limit ({limit}).', {limit:formatBytes(authClient?.getInfo().limits.max_in_flight_upload)}),
        ABORTED: t('Upload cancelled.'),
        DISK_FULL: t('Storage is full or the account quota has been reached.'),
      };
      showToast(messages[error.code] || error.message,'error');
    }
    finally { uploadController=null;state.uploading=false;el.uploadProgressContainer.classList.add('hidden');el.fileInput.value=''; }
  }

  // --- Session & HTTP Auth ---
  async function checkSession() {
    try {
      const client = await getAuthClient();
      const session = await client.auth.getSession();
      if (!session) { showLoginModal(); return; }
      filesModule ??= import('/static/files.js');
      state.session = (await filesModule).toViewModel(session);
      state.csrfToken = state.session.csrf_token || '';
      state.roots = state.session.roots || [];
      onSessionActive();
    } catch(error) { showLoginModal(); }
  }

  function showLoginModal() {
    el.loginModal.classList.remove('hidden');
    el.userInfo.classList.add('hidden');
    getAuthClient().catch(()=>{});
  }

  function onSessionActive() {
    const uiAccount=state.session.account_id??String(state.session.uid);
    if(document.documentElement)document.documentElement.dataset.uiAccount=uiAccount;
    window.TealDriveI18n?.bindAccount(uiAccount).catch(()=>{});
    getAuthClient().catch(()=>{});
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

  let authClient = null;
  let enrolling = false;
  let authImport = null;
  let webPasswords = false;
  async function getAuthClient() {
    if (!authClient) {
      authImport ??= import('/sdk/v1/tealdrive.js');
      const {createClient} = await authImport;
      if (!authClient) {
        authClient = createClient({stateMode:'managed'});
        authClient.on('server.shutdown', notice => showShutdownNotice(notice.deadlineSeconds));
        authClient.on('connection.changed', change => {
          if (change.status !== 'disconnected') return;
          state.wsConnected=false;
          el.wsStatusDot.className='status-dot disconnected';
          el.wsStatusText.textContent=state.serverShuttingDown?'Server stopped':'Disconnected';
          if (!state.serverShuttingDown && state.session) setTimeout(checkSession,3000);
        });
        authClient.on('session.revoked', ()=>{state.session=null;showLoginModal();});
        authClient.files.observeQuotaWarning(warning=>{
          const banner = document.getElementById('quotaWarning');
          banner.classList[warning.active?'remove':'add']('hidden');
          banner.textContent = warning.active ? t('Storage usage is {percent}%. Free space to keep uploads and edits available.',{percent:Math.round(warning.quota?.usagePercent ?? 0)})+(warning.status==='unknown'?' '+t('Current usage could not be refreshed.'):'') : '';
        });
      }
    }
    const info = await authClient.handshake();
    webPasswords = Boolean(info?.authentication?.webPassword);
    document.getElementById('personalPolicyBtn').classList[info?.authentication?.policyManagement?'remove':'add']('hidden');
    document.getElementById('historyBtn').classList[info?.authentication?.auditHistory?'remove':'add']('hidden');
    document.getElementById('manageAccountsBtn').classList[info?.authentication?.accountManagement && state.session?.role==='admin'?'remove':'add']('hidden');
    document.getElementById('accountSecurityBtn').classList[info?.authentication?.passwordChange?'remove':'add']('hidden');
    document.getElementById('registerWebOption').classList[info?.authentication?.enrollment?'remove':'add']('hidden');
    if (!enrolling) label(document.getElementById('passwordLabel'),webPasswords?'Web password':'Password');
    return authClient;
  }
  let accountManagement = null;
  let accountManagementImport = null;
  document.getElementById('manageAccountsBtn').onclick=async()=>{
    try {
      accountManagementImport ??= import('/static/accounts.js');
      const {createAccountManagement}=await accountManagementImport;
      accountManagement ??= createAccountManagement({getClient:getAuthClient,toast:showToast,formatBytes});
      await accountManagement.open();
    } catch(error) { showToast('Unable to open account management. Refresh and try again.','error'); }
  };
  document.getElementById('personalPolicyBtn').onclick=async()=>{
    try{const client=await getAuthClient();const {openPolicyEditor}=await import('/static/policy.js');await openPolicyEditor({client,onSaved:()=>location.reload()});}
    catch(error){showToast(error.message,'error');}
  };
  document.getElementById('historyBtn').onclick=async()=>{
    const button=document.getElementById('historyBtn');
    if(button.disabled) return;
    button.disabled=true;
    try { const client=await getAuthClient();const {openHistory}=await import('/static/history.js');await openHistory({client,restoreFocus:button,onChanged:()=>loadDirectory(state.currentPath)}); }
    catch(error) { showToast(error.message,'error'); }
    finally { button.disabled=false; }
  };
  function closeAccountSecurity() {
    for (const id of ['accountCurrentPassword','accountNewPassword','accountConfirmPassword']) document.getElementById(id).value='';
    document.getElementById('accountSecurityModal').classList.add('hidden');
  }
  document.getElementById('accountSecurityBtn').onclick=async()=>{
    try {
      const client=await getAuthClient(); const session=await client.auth.getSession();
      document.getElementById('accountRevokeBtn').classList[session?.accountKind==='linux'?'remove':'add']('hidden');
      document.getElementById('accountSecurityError').textContent='';
      document.getElementById('accountSecurityModal').classList.remove('hidden');
      document.getElementById('accountCurrentPassword').focus();
    } catch(error) { showToast(error.message,'error'); }
  };
  document.getElementById('accountSecurityCloseBtn').onclick=closeAccountSecurity;
  document.getElementById('accountPasswordForm').onsubmit=async(event)=>{
    event.preventDefault();
    const currentPassword=document.getElementById('accountCurrentPassword').value;
    const newPassword=document.getElementById('accountNewPassword').value;
    const confirmed=document.getElementById('accountConfirmPassword').value;
    for (const id of ['accountCurrentPassword','accountNewPassword','accountConfirmPassword']) document.getElementById(id).value='';
    if(newPassword!==confirmed){document.getElementById('accountSecurityError').textContent=t('Passwords do not match.');return;}
    const button=event.submitter; if(button)button.disabled=true;
    try { const client=await getAuthClient(); await client.auth.getSession(); await client.auth.changePassword({currentPassword,newPassword}); closeAccountSecurity(); location.reload(); }
    catch(error){document.getElementById('accountSecurityError').textContent=error.message;}
    finally{if(button)button.disabled=false;}
  };
  document.getElementById('accountRevokeBtn').onclick=async(event)=>{
    if(!confirm(t('Disable your web access and sign out all web sessions? Your Linux account and files are retained. Re-registration requires your Linux password and remains subject to administrator rules.')))return;
    event.target.disabled=true;
    try { const client=await getAuthClient(); await client.auth.getSession(); await client.auth.revokeSelf(); closeAccountSecurity(); location.reload(); }
    catch(error){document.getElementById('accountSecurityError').textContent=error.message;}
    finally{event.target.disabled=false;}
  };
  let loginInProgress = false;
  async function performLogin(username, password) {
    if (loginInProgress) return;
    loginInProgress = true;
    const submit = document.getElementById('loginSubmitBtn');
    submit.disabled = true;
    el.loginError.classList.add('hidden');
    el.passwordInput.value='';
    try {
      await getAuthClient();
      if (enrolling) {
        const confirm = el.webPasswordConfirm.value;
        el.webPasswordConfirm.value='';
        if (password !== confirm) throw {code:'PASSWORD_MISMATCH'};
        await authClient.auth.completeEnrollment({webPassword:password});
      } else if (el.registerWeb.checked) {
        await authClient.auth.beginEnrollment({username,linuxPassword:password});
        enrolling = true;
        el.usernameInput.readOnly = true;
        el.registerWeb.disabled = true;
        el.webPasswordGroup.classList.remove('hidden');
        el.webPasswordConfirm.required = true;
        el.passwordInput.autocomplete = 'new-password';
        label(document.getElementById('passwordLabel'),'New web password');
        el.loginError.textContent = t('Linux identity verified. Choose a separate web password (at least 12 characters), then confirm it below.');
        el.loginError.classList.remove('hidden');
        el.passwordInput.focus();
        return;
      } else {
        await authClient.auth.login({username,password});
      }
      enrolling = false;
      el.usernameInput.readOnly = false;
      el.registerWeb.checked = false;
      el.registerWeb.disabled = false;
      el.webPasswordGroup.classList.add('hidden');
      el.webPasswordConfirm.required = false;
      el.passwordInput.autocomplete = 'current-password';
      await checkSession();
    } catch(err) {
      const messages = {PASSWORD_MISMATCH:'The web passwords do not match.',INVALID_ARGUMENT:'Enter a web password of at least 12 characters.',FEATURE_DISABLED:'Self-registration is not enabled on this server.',INVALID_CREDENTIALS:'Invalid credentials or account access is disabled.',ENROLLMENT_REQUIRED:'Start registration again.',SESSION_BIND_FAILED:'Session could not be established. Please sign in again.'};
      el.loginError.textContent = t(messages[err.code] || 'Connection or authentication error. Please try again.');
      el.loginError.classList.remove('hidden');
      if (['DISCONNECTED','CONNECTION_CLOSED','CONNECTION_FAILED','HANDSHAKE_REQUIRED','SESSION_BIND_FAILED','ENROLLMENT_REQUIRED'].includes(err.code)) {
        authClient?.disconnect(); authClient=null; enrolling=false;
        el.usernameInput.readOnly=false; el.registerWeb.disabled=false;
        el.webPasswordGroup.classList.add('hidden'); el.webPasswordConfirm.required=false;
        el.passwordInput.autocomplete='current-password';
      }
    } finally {
      password=''; el.passwordInput.value=''; el.webPasswordConfirm.value='';
      loginInProgress=false; submit.disabled=false;
    }
  }

  async function performLogout() {
    try { const client=await getAuthClient(); await client.auth.logout(); window.location.reload(); }
    catch(error) { showToast(error.message || 'Sign-out could not be completed. Please retry.','error'); }
  }

  // --- Utility Functions ---
  function showToast(msg, type = 'info') {
    const t = document.createElement('div');
    t.className = `toast toast-${type}`;
    t.textContent = window.TealDriveI18n?.t(msg) ?? msg;
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

  let selectedLoginUsers = new Set();
  let userChoicesReady = false;
  async function loadUserChoices(configured) {
    selectedLoginUsers = new Set(configured);
    userChoicesReady = false;
    el.cfgAllowedUsers.replaceChildren();
    el.cfgUsersStatus.textContent=t('Loading local users...');
    const client = await getAuthClient();
    const data = await client.admin.systemUsers.list();
    const available=new Set(data.users.map(user=>user.username));
    function addChoice(user,existingOnly=false) {
      const label=document.createElement('label');label.className='user-choice';
      const input=document.createElement('input');input.type='checkbox';input.value=user.username;
      input.checked=selectedLoginUsers.has(user.username);
      const text=document.createElement('span');text.textContent=existingOnly?user.username+' — existing entry outside filtered list (remove only)':user.username+' (UID '+user.uid+')';
      input.addEventListener('change',()=>{
        if(input.checked)selectedLoginUsers.add(user.username);
        else {selectedLoginUsers.delete(user.username);if(existingOnly)input.disabled=true;}
      });
      label.append(input,text);el.cfgAllowedUsers.appendChild(label);
    }
    for(const user of data.users)addChoice(user);
    for(const username of configured)if(!available.has(username))addChoice({username},true);
    el.cfgUsersStatus.textContent=data.truncated?'Showing the first 1024 eligible local users. Existing selections are preserved.':data.users.length?'Select users who may sign in. System and service accounts are filtered.':'No eligible local users found. Existing selections are preserved.';
    userChoicesReady=true;
  }

  let currentAdminConfigMode = "legacy";
  async function openSettingsModal() {
    try {
      const client = await getAuthClient();
      const snapshot = await client.admin.settings.inspect();
      filesModule ??= import('/static/files.js');
      const cfg = (await filesModule).toViewModel(snapshot.config);
      currentAdminConfig = cfg;
      currentAdminConfigMode = snapshot.mode;
      const split = currentAdminConfigMode === "split";
      el.cfgTrustedProxies.disabled = split;
      el.cfgClientIpHeader.disabled = split;
      if (cfg && cfg.features) {
        el.cfgFeatEdit.checked = !!cfg.features.save_text_file;
        el.cfgFeatDelete.checked = !!cfg.features.delete_permanently;
        el.cfgFeatChmod.checked = !!cfg.features.chmod_file;
        el.cfgFeatArchive.checked = !!cfg.features.extract_archive;
      }
      await loadUserChoices(cfg.account_policy?.allowed_users||[]);
      if (split) {
        for (const input of el.cfgAllowedUsers.querySelectorAll("input")) input.disabled = true;
        el.cfgUsersStatus.textContent = t("Root policy controls eligible users. Account grants use account management.");
      }
      el.cfgTrustedProxies.value=(cfg.proxy?.trusted_proxies||[]).join('\n');
      el.cfgClientIpHeader.value=cfg.proxy?.client_ip_header||'XForwardedFor';
      el.cfgAuditPath.value=cfg.audit?.path||'';
      el.cfgAuditMaxMB.value=(cfg.audit?.max_file_bytes??10485760)/1048576;
      el.cfgAuditArchives.value=cfg.audit?.max_archives??5;
      el.cfgAuditDays.value=(cfg.audit?.retention_days?cfg.audit.retention_days*24:cfg.audit?.retention_hours)||'';
      el.cfgAuditHealth.textContent='Audit write failures since server start: '+(resp.headers.get('x-tealdrive-audit-write-failures')||'0');
      el.cfgTrashRetention.disabled = split || !!cfg.web_accounts?.enabled;
      el.cfgTrashRetention.value = cfg.web_accounts?.enabled ? (cfg.lifecycle?.trash_retention_hours ?? 504) / 24 : (cfg.trash_retention_days ?? 30);
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
    if(!userChoicesReady){showToast('Load user choices before saving settings.','error');return;}
    const auditMB=Number(el.cfgAuditMaxMB.value);
    const archives=Number(el.cfgAuditArchives.value);
    const dayText=el.cfgAuditDays.value.trim();const days=dayText===''?null:Number(dayText);
    if(!el.cfgAuditPath.value.trim().startsWith('/')||!Number.isFinite(auditMB)||auditMB<1/1024||auditMB>1024||!Number.isInteger(archives)||archives<0||archives>1000||el.cfgAuditArchives.value.trim()===''|| (days!==null&&(!Number.isInteger(days)||days<1||days>876000))){showToast('Enter a valid absolute log path, file size and archive retention.','error');return;}
    const patch = {
      allowed_users:[...selectedLoginUsers].sort(),
      proxy:{trusted_proxies:el.cfgTrustedProxies.value.split(/[\n,]+/).map(v=>v.trim()).filter(Boolean),client_ip_header:el.cfgClientIpHeader.value},
      audit:{path:el.cfgAuditPath.value.trim(),max_file_bytes:Math.round(auditMB*1048576),max_archives:archives,retention_hours:days},
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
    if (currentAdminConfigMode === "split") {
      delete patch.allowed_users;
      delete patch.proxy;
      delete patch.trash_retention_days;
      patch.session = {idle_timeout_seconds: patch.session.idle_timeout_seconds};
    }
    try {
      const client = await getAuthClient();
      await client.admin.settings.patch({changes:patch,persist:el.cfgPersistDisk.checked});
      showToast('Settings saved and hot-reloaded successfully!','success');
      el.settingsModal.classList.add('hidden');
    } catch(error) { showToast(error.message,'error'); }
  }

  async function reloadSettingsFromDisk() {
    try {
      const client = await getAuthClient();
      await client.admin.settings.reload();
      showToast('Config reloaded from disk!','success');
      await openSettingsModal();
    } catch(error) { showToast(error.message,'error'); }
  }

  // --- Event Listeners Setup ---
  function setupEvents() {
    el.settingsBtn.onclick = openSettingsModal;
    el.settingsCloseBtn.onclick = () => el.settingsModal.classList.add('hidden');
    el.settingsCancelBtn.onclick = () => el.settingsModal.classList.add('hidden');
    el.cfgSaveBtn.onclick = saveSettings;
    el.cfgReloadBtn.onclick = reloadSettingsFromDisk;
    el.registerWeb.onchange = () => {
      label(document.getElementById('passwordLabel'),el.registerWeb.checked?'Linux password':(webPasswords?'Web password':'Password'));
      el.passwordInput.autocomplete=el.registerWeb.checked?'off':'current-password';
      el.passwordInput.value='';
    };
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
        runFileAction('CreateFolder', {
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
        runFileAction('SaveTextFile', {
          root_id: state.currentRoot,
          relative_path: targetPath,
          content: '',
          expected_revision: 'missing',
        });
      });
    };

    document.getElementById('uploadCancelBtn').onclick = (event) => {
      event.target.disabled = true;
      uploadController?.abort();
    };
    el.uploadBtn.onclick = () => el.fileInput.click();
    el.fileInput.onchange = (e) => {
      if (e.target.files && e.target.files[0]) {
        handleFileUpload(e.target.files[0]);
      }
    };

    el.shutdownDismissBtn.onclick = dismissShutdownNotice;
    el.shutdownRetryBtn.onclick = checkServerAvailability;

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
      if((mode&0o002)&&!(state.chmodOriginalMode&0o002)&&!window.confirm(t('Allow other users to write to this item?')))return;
      el.chmodApplyBtn.disabled=true;
      try {
        await requestFile('ChmodFile',{root_id:state.chmodRootId,relative_path:state.chmodTarget,mode});
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
      if (!el.shutdownModal.classList.contains('hidden')) {
        if (e.key === 'Escape') { dismissShutdownNotice(); return; }
        if (e.key === 'Tab') {
          e.preventDefault();
          const next = document.activeElement === el.shutdownDismissBtn && !el.shutdownRetryBtn.disabled
            ? el.shutdownRetryBtn : el.shutdownDismissBtn;
          next.focus();
        }
        return;
      }
      if (e.key === 'Escape') {
        accountManagement?.close();
        closeAccountSecurity();
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
        runFileAction('SearchFiles', {
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
