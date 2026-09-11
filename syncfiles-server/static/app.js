'use strict';

/* ============ Estado ============ */
const store = {
  token: localStorage.getItem('sf_token') || '',
  serverUrl: localStorage.getItem('sf_server') || defaultServerUrl(),
  deviceId: localStorage.getItem('sf_device') || randomId('web'),
  user: null,
  view: 'dashboard',
  refreshTimer: null,
};

function defaultServerUrl() {
  return window.location.origin === 'null' || window.location.origin === 'file://'
    ? 'http://127.0.0.1:8080'
    : window.location.origin;
}

function randomId(prefix) {
  const rnd = crypto.getRandomValues(new Uint8Array(8));
  const hex = Array.from(rnd, b => b.toString(16).padStart(2, '0')).join('');
  return `${prefix}-${hex}`;
}

function api(path, opts = {}) {
  const url = `${store.serverUrl.replace(/\/+$/, '')}/api/v1${path}`;
  const headers = { 'Content-Type': 'application/json', ...(opts.headers || {}) };
  if (store.token) headers['Authorization'] = `Bearer ${store.token}`;
  return fetch(url, { ...opts, headers });
}

async function apiJson(path, opts = {}) {
  const resp = await api(path, opts);
  let body = null;
  try { body = await resp.json(); } catch (_) { /* sin cuerpo */ }
  if (resp.status === 401) {
    logoutLocal();
    throw new Error('Sesión expirada. Vuelve a iniciar sesión.');
  }
  if (!resp.ok) {
    const msg = (body && body.error && body.error.message) || (body && body.message) || `HTTP ${resp.status}`;
    throw new Error(msg);
  }
  return body;
}

/* ============ Utilidades UI ============ */
const $ = (id) => document.getElementById(id);

function toast(msg, kind = 'ok') {
  const el = $('toast');
  el.textContent = msg;
  el.className = `toast ${kind}`;
  clearTimeout(el._t);
  el._t = setTimeout(() => el.classList.add('hidden'), 3500);
}

function showError(id, err) {
  const el = $(id);
  el.textContent = err instanceof Error ? err.message : String(err);
  el.classList.remove('hidden');
}

function clearError(id) { $(id).classList.add('hidden'); }

function fmtBytes(n) {
  if (n === null || n === undefined) return '—';
  if (n < 1024) return `${n} B`;
  const units = ['KiB', 'MiB', 'GiB', 'TiB'];
  let v = n;
  let i = -1;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(1)} ${units[i]}`;
}

function fmtDate(ms) {
  if (!ms) return '—';
  const d = new Date(ms);
  return d.toLocaleString();
}

function setConnBadge(ok) {
  const b = $('conn-badge');
  b.textContent = ok ? 'Conectado' : 'Sin conectar';
  b.className = `badge ${ok ? 'online' : 'offline'}`;
}

/* ============ Sesión ============ */
function logoutLocal() {
  store.token = '';
  store.user = null;
  localStorage.removeItem('sf_token');
  stopAutoRefresh();
  $('app-view').classList.add('hidden');
  $('login-view').classList.remove('hidden');
  $('login-password').value = '';
  setConnBadge(false);
}

async function tryRestoreSession() {
  if (!store.token) return false;
  try {
    const session = await apiJson('/session/status');
    store.user = session;
    return true;
  } catch (_) {
    store.token = '';
    localStorage.removeItem('sf_token');
    return false;
  }
}

/* ============ Login ============ */
async function handleLogin(ev) {
  ev.preventDefault();
  clearError('login-error');
  const btn = $('login-submit');
  btn.disabled = true;
  btn.textContent = 'Conectando…';

  const email = $('login-email').value.trim();
  const password = $('login-password').value;
  const serverInput = $('login-server').value.trim().replace(/\/+$/, '');

  try {
    const resp = await fetch(`${serverInput}/api/v1/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password, device_id: store.deviceId }),
    });
    const body = await resp.json().catch(() => null);
    if (!resp.ok) {
      throw new Error((body && body.error && body.error.message) || (body && body.message) || `Error HTTP ${resp.status}`);
    }
    store.token = body.session_id;
    store.serverUrl = serverInput;
    store.user = { user_id: body.user_id };
    localStorage.setItem('sf_token', store.token);
    localStorage.setItem('sf_server', store.serverUrl);
    localStorage.setItem('sf_device', store.deviceId);

    enterApp();
    await Promise.allSettled([loadDashboard(), loadConflictCount()]);
  } catch (err) {
    showError('login-error', err);
  } finally {
    btn.disabled = false;
    btn.textContent = 'Iniciar sesión';
  }
}

/* ============ Navegación ============ */
function showView(name) {
  store.view = name;
  for (const link of document.querySelectorAll('.nav-link')) {
    link.classList.toggle('active', link.dataset.view === name);
  }
  for (const view of document.querySelectorAll('.view')) {
    view.classList.add('hidden');
  }
  $(`view-${name}`).classList.remove('hidden');
  closeNav();
}

function openNav() {
  $('nav').classList.add('open');
  const overlay = document.createElement('div');
  overlay.id = 'nav-overlay';
  overlay.className = 'nav-overlay';
  overlay.addEventListener('click', closeNav);
  document.body.appendChild(overlay);
}

function closeNav() {
  $('nav').classList.remove('open');
  const overlay = $('nav-overlay');
  if (overlay) overlay.remove();
}

/* ============ Dashboard ============ */
async function loadDashboard() {
  clearError('dash-error');
  try {
    const [stats, devices] = await Promise.all([
      apiJson('/storage/stats'),
      apiJson('/devices'),
    ]);
    setConnBadge(true);

    const stateEl = $('dash-server-state');
    stateEl.textContent = 'En línea';
    stateEl.classList.add('big-ok');
    $('dash-server-url').textContent = store.serverUrl;
    $('dash-storage-used').textContent = fmtBytes(stats.used_bytes);
    $('dash-storage-files').textContent = `${stats.file_count} archivo(s) sincronizado(s)`;
    $('dash-last-mod').textContent = stats.last_modified_at ? fmtDate(stats.last_modified_at) : '—';

    renderDevices(devices.devices || []);
  } catch (err) {
    setConnBadge(false);
    showError('dash-error', err);
  }
}

function renderDevices(devices) {
  const tbody = $('devices-table').querySelector('tbody');
  tbody.innerHTML = '';
  $('devices-empty').classList.toggle('hidden', devices.length > 0);
  for (const d of devices) {
    const tr = document.createElement('tr');
    const active = d.active_sessions >= 1;

    const revokeBtn = document.createElement('button');
    revokeBtn.className = 'btn tiny danger';
    revokeBtn.innerHTML = `<svg class="b-ico"><use href="#i-shield-off"/></svg>Revocar`;
    revokeBtn.disabled = !active || d.device_id === store.deviceId;
    revokeBtn.title = d.device_id === store.deviceId
      ? 'Es tu dispositivo actual: usa "Cerrar sesión"'
      : (active ? 'Revoca todas las sesiones activas de este dispositivo' : 'Sin sesiones activas');
    revokeBtn.addEventListener('click', () => revokeDevice(d));

    const actions = document.createElement('div');
    actions.className = 'row-actions devices-actions';
    actions.appendChild(revokeBtn);
    const revokeBtn2 = revokeBtn.cloneNode(true);
    revokeBtn2.disabled = revokeBtn.disabled;
    revokeBtn2.title = revokeBtn.title;
    revokeBtn2.addEventListener('click', () => revokeDevice(d));
    const secondary = document.createElement('div');
    secondary.className = 'devices-actions-sec';
    secondary.appendChild(revokeBtn2);

    tr.innerHTML = `
      <td>${escapeHtml(d.device_name || d.device_id)}</td>
      <td>${escapeHtml(d.platform || '—')}</td>
      <td>${active ? '<span class="status-pill synced">Sí</span>' : '<span class="status-pill deleted">No</span>'}</td>
      <td>${fmtDate(d.last_seen_at)}</td>`;
    const tdActions = document.createElement('td');
    tdActions.appendChild(actions);
    tr.appendChild(tdActions);
    const tdSecondary = document.createElement('td');
    tdSecondary.appendChild(secondary);
    tr.appendChild(tdSecondary);
    tbody.appendChild(tr);
  }
}

async function revokeDevice(d) {
  if (!window.confirm(`¿Revocar todas las sesiones activas de "${d.device_name || d.device_id}"?\nEse dispositivo tendrá que iniciar sesión de nuevo.`)) return;
  try {
    const data = await apiJson('/devices/revoke', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        target_device_id: d.device_id,
      }),
    });
    const n = (data && data.data && data.data.sessions_revoked) || 0;
    toast(`Dispositivo revocado (${n} sesión(es))`);
    await loadDashboard();
  } catch (err) {
    toast(`Revocación fallida: ${err.message}`, 'err');
  }
}

/* ============ Archivos ============ */
async function loadFiles() {
  clearError('files-error');
  try {
    const showDeleted = $('files-show-deleted').checked;
    const [data, diff] = await Promise.all([
      apiJson(`/files/list${showDeleted ? '?include_deleted=true' : ''}`),
      apiJson('/sync/diff', {
        method: 'POST',
        body: JSON.stringify({
          session_id: store.token,
          device_id: store.deviceId,
          since: 0,
          request_id: randomId('webdiff'),
        }),
      }).catch(() => null),
    ]);
    setConnBadge(true);
    const changes = (diff && diff.changes) || [];
    renderFiles(data.files || [], changes);
    renderDiffCount(changes);
  } catch (err) {
    setConnBadge(false);
    showError('files-error', err);
  }
}

function renderDiffCount(changes) {
  const el = $('files-diff-hint');
  if (!changes.length) {
    el.classList.add('hidden');
    return;
  }
  el.classList.remove('hidden');
  el.textContent = `${changes.length} cambio(s) en el servidor sin aplicar en este navegador`;
}

const STATUS_LABELS = { synced: 'Sincronizado', deleted: 'Borrado', pending: 'Pendiente', queued: 'En cola', retry: 'Reintentando' };

function statusPill(status) {
  return `<span class="status-pill ${escapeHtml(status)}">${escapeHtml(STATUS_LABELS[status] || status)}</span>`;
}

function renderFiles(files, changes = []) {
  const changesByPathHash = new Map(changes.map(c => [c.path_hash, c]));
  const tbody = $('files-table').querySelector('tbody');
  tbody.innerHTML = '';
  $('files-empty').classList.toggle('hidden', files.length > 0);
  for (const f of files) {
    const change = changesByPathHash.get(f.path_hash);
    const outdated = change && change.modified_at > (f.synced_at || 0) && change.operation !== 'delete';
    const tr = document.createElement('tr');
    if (f.status === 'deleted') tr.classList.add('deleted-row');

    const actions = document.createElement('div');
    actions.className = 'row-actions';
    const secondary = document.createElement('div');
    secondary.className = 'file-secondary';

    const downloadBtn = document.createElement('button');
    downloadBtn.className = 'btn tiny ghost';
    downloadBtn.innerHTML = `<svg class="b-ico"><use href="#i-download"/></svg>Descargar`;
    downloadBtn.addEventListener('click', () => downloadFile(f));

    const renameBtn = document.createElement('button');
    renameBtn.className = 'btn tiny ghost';
    renameBtn.innerHTML = `<svg class="b-ico"><use href="#i-edit"/></svg>Renombrar`;
    renameBtn.disabled = f.status === 'deleted';
    renameBtn.addEventListener('click', () => renameFile(f));

    const moveBtn = document.createElement('button');
    moveBtn.className = 'btn tiny ghost';
    moveBtn.innerHTML = `<svg class="b-ico"><use href="#i-move"/></svg>Mover`;
    moveBtn.disabled = f.status === 'deleted';
    moveBtn.addEventListener('click', () => moveFile(f));

    const copyBtn = document.createElement('button');
    copyBtn.className = 'btn tiny ghost';
    copyBtn.innerHTML = `<svg class="b-ico"><use href="#i-copy"/></svg>Copiar`;
    copyBtn.disabled = f.status === 'deleted';
    copyBtn.addEventListener('click', () => copyFile(f));

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'btn tiny danger';
    deleteBtn.innerHTML = `<svg class="b-ico"><use href="#i-trash"/></svg>Borrar`;
    deleteBtn.disabled = f.status === 'deleted';
    deleteBtn.addEventListener('click', () => deleteFile(f));

    const mkBtn2 = (btn) => {
      const clone = btn.cloneNode(true);
      clone.disabled = btn.disabled;
      clone.addEventListener('click', () => btn.click());
      return clone;
    };

    actions.append(downloadBtn, renameBtn, moveBtn, copyBtn, deleteBtn);
    secondary.append(mkBtn2(downloadBtn), mkBtn2(renameBtn), mkBtn2(moveBtn), mkBtn2(copyBtn), mkBtn2(deleteBtn));

    tr.innerHTML = `
      <td class="path-cell">${escapeHtml(f.relative_path)}${outdated ? ' <span class="status-pill pending" title="Cambio en el servidor aún no aplicado en este navegador">Desactualizado</span>' : ''}</td>
      <td class="num">${fmtBytes(f.size_bytes)}</td>
      <td>${fmtDate(f.modified_at)}</td>
      <td>${statusPill(f.status)}</td>`;
    const tdActions = document.createElement('td');
    tdActions.appendChild(actions);
    tr.appendChild(tdActions);

    const tdSecondary = document.createElement('td');
    tdSecondary.appendChild(secondary);
    tr.appendChild(tdSecondary);
    tbody.appendChild(tr);
  }
}

async function downloadFile(f) {
  try {
    const data = await apiJson('/sync/download', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        file_id: f.file_id,
        path_hash: f.path_hash,
        idempotency_key: randomId('webdl'),
      }),
    });
    const bytes = base64ToBytes(data.content);
    const blob = new Blob([bytes]);
    const url = URL.createObjectURL(blob);
    const name = f.relative_path.split('/').pop() || 'archivo';
    const a = document.createElement('a');
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    toast(`Descargado: ${name}`);
  } catch (err) {
    toast(`Descarga fallida: ${err.message}`, 'err');
  }
}

async function deleteFile(f) {
  if (!window.confirm(`¿Borrar "${f.relative_path}" del servidor?\nLos otros dispositivos lo eliminarán en su próxima sincronización.`)) return;
  try {
    await apiJson('/sync/delete', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        file_id: f.file_id,
        path_hash: f.path_hash,
        idempotency_key: randomId('webdel'),
      }),
    });
    toast(`Borrado: ${f.relative_path}`);
    await loadFiles();
    await loadConflictCount();
  } catch (err) {
    toast(`Borrado fallido: ${err.message}`, 'err');
  }
}

/* ============ Subir archivos ============ */
async function uploadFiles(fileList) {
  const files = Array.from(fileList || []);
  if (!files.length) return;

  const progressEl = $('upload-progress');
  progressEl.classList.remove('hidden');

  for (const file of files) {
    progressEl.textContent = `Subiendo: ${file.name} (${fmtBytes(file.size)})…`;
    try {
      await uploadOne(file);
      toast(`Subido: ${file.name}`);
    } catch (err) {
      toast(`Subida fallida (${file.name}): ${err.message}`, 'err');
    }
  }

  progressEl.classList.add('hidden');
  await Promise.allSettled([loadFiles(), loadDashboard()]);
}

async function uploadOne(file) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const checksum = await sha256Hex(bytes);
  await apiJson('/sync/upload', {
    method: 'POST',
    body: JSON.stringify({
      session_id: store.token,
      device_id: store.deviceId,
      file_id: '',
      relative_path: file.name,
      path_hash: await sha256Hex(new TextEncoder().encode(file.name)),
      checksum,
      size_bytes: bytes.length,
      modified_at: file.lastModified || Date.now(),
      idempotency_key: randomId('webup'),
      content: bytesToBase64(bytes),
    }),
  });
}

function base64ToBytes(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function bytesToBase64(bytes) {
  let bin = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    bin += String.fromCharCode.apply(null, bytes.subarray(i, i + chunk));
  }
  return btoa(bin);
}

function toHex(buffer) {
  return Array.from(new Uint8Array(buffer), b => b.toString(16).padStart(2, '0')).join('');
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return toHex(digest);
}

function joinPath(dir, name) {
  if (!dir || dir === '' || dir === '.' || dir === '/') return name;
  return `${dir.replace(/\/+$/, '')}/${name}`;
}

function parentDir(path) {
  const idx = path.lastIndexOf('/');
  return idx === -1 ? '' : path.slice(0, idx);
}

function baseName(path) {
  return path.split('/').pop() || path;
}

/* ============ Modal ============ */
const modalState = { resolve: null };

function openModal({ title, description, label, value, okLabel = 'Aceptar' }) {
  return new Promise((resolve) => {
    modalState.resolve = resolve;
    $('modal-title').textContent = title;
    $('modal-desc').textContent = description || '';
    $('modal-desc').classList.toggle('hidden', !description);
    $('modal-label').textContent = label || 'Valor';
    $('modal-input').value = value || '';
    $('modal-ok').textContent = okLabel;
    clearError('modal-error');
    $('modal-overlay').classList.remove('hidden');
    $('modal-input').focus();
    $('modal-input').select();
  });
}

function closeModal(result) {
  $('modal-overlay').classList.add('hidden');
  const resolve = modalState.resolve;
  modalState.resolve = null;
  if (resolve) resolve(result);
}

async function promptModal(opts) {
  const value = await openModal(opts);
  return value;
}

/* ============ Rename / Move / Copy ============ */
async function renameFile(f) {
  const value = await promptModal({
    title: 'Renombrar archivo',
    description: f.relative_path,
    label: 'Nuevo nombre de archivo',
    value: baseName(f.relative_path),
    okLabel: 'Renombrar',
  });
  if (value === null || value === undefined) return;
  const newName = String(value).trim();
  if (!newName || newName === baseName(f.relative_path)) return;
  const newPath = joinPath(parentDir(f.relative_path), newName);
  try {
    await apiJson('/sync/rename', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        file_id: f.file_id,
        old_path: f.relative_path,
        new_path: newPath,
        idempotency_key: randomId('webren'),
      }),
    });
    toast(`Renombrado a: ${newPath}`);
    await loadFiles();
  } catch (err) {
    toast(`Renombrado fallido: ${err.message}`, 'err');
  }
}

async function moveFile(f) {
  const value = await promptModal({
    title: 'Mover archivo',
    description: f.relative_path,
    label: 'Nueva ruta (carpetas con /)',
    value: f.relative_path,
    okLabel: 'Mover',
  });
  if (value === null || value === undefined) return;
  const newPath = String(value).trim().replace(/^\/+/, '');
  if (!newPath || newPath === f.relative_path) return;
  try {
    await apiJson('/sync/move', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        file_id: f.file_id,
        old_path: f.relative_path,
        new_path: newPath,
        idempotency_key: randomId('webmov'),
      }),
    });
    toast(`Movido a: ${newPath}`);
    await loadFiles();
  } catch (err) {
    toast(`Movimiento fallido: ${err.message}`, 'err');
  }
}

async function copyFile(f) {
  const suggested = joinPath(parentDir(f.relative_path), `${baseName(f.relative_path)}.copia`);
  const value = await promptModal({
    title: 'Copiar archivo',
    description: f.relative_path,
    label: 'Ruta de la copia',
    value: suggested,
    okLabel: 'Copiar',
  });
  if (value === null || value === undefined) return;
  const dst = String(value).trim().replace(/^\/+/, '');
  if (!dst) return;
  try {
    await apiJson('/sync/copy', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        file_id: f.file_id,
        source_path: f.relative_path,
        destination_path: dst,
        idempotency_key: randomId('webcpy'),
      }),
    });
    toast(`Copiado a: ${dst}`);
    await Promise.allSettled([loadFiles(), loadDashboard()]);
  } catch (err) {
    toast(`Copia fallida: ${err.message}`, 'err');
  }
}

/* ============ Cola ============ */
const OP_LABELS = { upload: 'Subir', download: 'Descargar', delete: 'Borrar', rename: 'Renombrar', move: 'Mover', copy: 'Copiar' };

async function loadQueue() {
  clearError('queue-error');
  try {
    const pendingOnly = $('queue-pending-only').checked;
    const q = pendingOnly ? '?status=pending&limit=100' : '?limit=100';
    const data = await apiJson(`/queue${q}`);
    setConnBadge(true);
    renderQueue(data.entries || []);
  } catch (err) {
    setConnBadge(false);
    showError('queue-error', err);
  }
}

function renderQueue(entries) {
  const tbody = $('queue-table').querySelector('tbody');
  tbody.innerHTML = '';
  $('queue-empty').classList.toggle('hidden', entries.length > 0);
  for (const e of entries) {
    const tr = document.createElement('tr');
    const op = escapeHtml(e.operation || '');
    tr.innerHTML = `
      <td class="path-cell">${escapeHtml(e.file_id || '—')}</td>
      <td><span class="op-pill ${op}">${escapeHtml(OP_LABELS[e.operation] || e.operation || '—')}</span></td>
      <td class="path-cell">${escapeHtml(e.device_id || '—')}</td>
      <td>${statusPill(e.status)}</td>
      <td class="num">${e.attempts ?? 0}</td>
      <td>${fmtDate(e.updated_at)}</td>
      <td class="path-cell queue-error-cell">${escapeHtml(e.last_error || '—')}</td>`;
    tbody.appendChild(tr);
  }
}

/* ============ Conflictos ============ */
async function loadConflicts() {
  clearError('conflicts-error');
  try {
    const data = await apiJson('/conflicts');
    setConnBadge(true);
    renderConflicts(data.conflicts || []);
  } catch (err) {
    setConnBadge(false);
    showError('conflicts-error', err);
  }
}

function renderConflicts(conflicts) {
  const list = $('conflicts-list');
  list.innerHTML = '';
  $('conflicts-empty').classList.toggle('hidden', conflicts.length > 0);
  for (const c of conflicts) {
    const card = document.createElement('div');
    card.className = 'conflict-card';
    card.innerHTML = `
      <h4>${escapeHtml(c.file_id)}</h4>
      <div class="conflict-meta">
        <span><span class="k">Tipo:</span> ${escapeHtml(c.conflict_type)}</span>
        <span><span class="k">Estrategia:</span> ${escapeHtml(c.strategy)}</span>
        <span><span class="k">Dispositivo local:</span> ${escapeHtml(c.device_local || '—')}</span>
        <span><span class="k">Dispositivo remoto:</span> ${escapeHtml(c.device_remote || '—')}</span>
        <span><span class="k">Checksum local:</span> <span class="path-cell">${escapeHtml((c.local_checksum || '').slice(0, 12))}</span></span>
        <span><span class="k">Checksum remoto:</span> <span class="path-cell">${escapeHtml((c.remote_checksum || '').slice(0, 12))}</span></span>
        <span><span class="k">Creado:</span> ${fmtDate(c.created_at)}</span>
      </div>`;
    const actions = document.createElement('div');
    actions.className = 'conflict-actions';

    const mkBtn = (label, decision, cls) => {
      const b = document.createElement('button');
      b.className = `btn tiny ${cls}`;
      b.textContent = label;
      b.addEventListener('click', () => resolveConflict(c, decision));
      return b;
    };
    actions.append(
      mkBtn('Mantener local', 'keep_local', 'ghost'),
      mkBtn('Mantener remoto', 'keep_remote', 'ghost'),
      mkBtn('Local + copia alternativa', 'keep_local_preserve', 'primary'),
    );
    card.appendChild(actions);
    list.appendChild(card);
  }
}

async function resolveConflict(c, decision) {
  const preserve = decision.endsWith('_preserve');
  const base = decision.replace('_preserve', '');
  try {
    await apiJson('/conflicts/resolve', {
      method: 'POST',
      body: JSON.stringify({
        session_id: store.token,
        device_id: store.deviceId,
        conflict_id: c.conflict_id,
        decision: base,
        preserve_alternative: preserve,
        new_name: null,
      }),
    });
    toast(`Conflicto resuelto (${base})`);
    await Promise.allSettled([loadConflicts(), loadConflictCount(), loadDashboard()]);
  } catch (err) {
    toast(`Resolución fallida: ${err.message}`, 'err');
  }
}

async function loadConflictCount() {
  try {
    const data = await apiJson('/conflicts');
    const n = data.total || 0;
    const el = $('conflict-count');
    el.textContent = n;
    el.classList.toggle('hidden', n === 0);
  } catch (_) { /* silencioso */ }
}

/* ============ Actividad ============ */
async function loadActivity() {
  clearError('activity-error');
  try {
    const data = await apiJson('/activity?limit=100');
    setConnBadge(true);
    renderActivity(data.events || []);
  } catch (err) {
    setConnBadge(false);
    showError('activity-error', err);
  }
}

function renderActivity(events) {
  const tbody = $('activity-table').querySelector('tbody');
  tbody.innerHTML = '';
  $('activity-empty').classList.toggle('hidden', events.length > 0);
  for (const e of events) {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>${fmtDate(e.created_at)}</td>
      <td><span class="path-cell">${escapeHtml(e.event_name)}</span></td>
      <td>${escapeHtml(e.device_id || '—')}</td>
      <td class="path-cell">${escapeHtml(truncatePayload(e.payload_json))}</td>`;
    tbody.appendChild(tr);
  }
}

function truncatePayload(p) {
  if (!p) return '—';
  return p.length > 80 ? `${p.slice(0, 80)}…` : p;
}

/* ============ Auto-refresh ============ */
function startAutoRefresh() {
  stopAutoRefresh();
  store.refreshTimer = setInterval(async () => {
    if (store.view === 'dashboard') await loadDashboard();
    if (store.view === 'files') await loadFiles();
    if (store.view === 'queue') await loadQueue();
    if (store.view === 'conflicts') await loadConflicts();
    if (store.view === 'activity') await loadActivity();
    await loadConflictCount();
  }, 15000);
}

function stopAutoRefresh() {
  if (store.refreshTimer) clearInterval(store.refreshTimer);
  store.refreshTimer = null;
}

/* ============ Misc ============ */
function escapeHtml(s) {
  if (s === null || s === undefined) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function enterApp() {
  $('login-view').classList.add('hidden');
  $('app-view').classList.remove('hidden');
  showView(store.view || 'dashboard');
  startAutoRefresh();
}

async function handleLogout() {
  try {
    await api('/auth/logout', { method: 'POST' });
  } catch (_) { /* cerrar sesión local de todos modos */ }
  logoutLocal();
}

/* ============ Init ============ */
document.addEventListener('DOMContentLoaded', async () => {
  $('login-server').value = store.serverUrl;
  $('login-server-hint').textContent = `Dispositivo: ${store.deviceId}`;

  $('login-form').addEventListener('submit', handleLogin);
  $('logout-btn').addEventListener('click', handleLogout);

  $('nav-toggle').addEventListener('click', () => {
    if ($('nav').classList.contains('open')) closeNav();
    else openNav();
  });

  for (const link of document.querySelectorAll('.nav-link')) {
    link.addEventListener('click', (ev) => {
      ev.preventDefault();
      showView(link.dataset.view);
      const v = link.dataset.view;
      if (v === 'dashboard') loadDashboard();
      if (v === 'files') loadFiles();
      if (v === 'queue') loadQueue();
      if (v === 'conflicts') loadConflicts();
      if (v === 'activity') loadActivity();
    });
  }

  $('files-refresh').addEventListener('click', loadFiles);
  $('files-show-deleted').addEventListener('change', loadFiles);
  $('queue-refresh').addEventListener('click', loadQueue);
  $('queue-pending-only').addEventListener('change', loadQueue);
  $('conflicts-refresh').addEventListener('click', loadConflicts);
  $('activity-refresh').addEventListener('click', loadActivity);

  // Subida de archivos: click, teclado y drag&drop
  const dz = $('upload-zone');
  const uploadInput = $('upload-input');
  dz.addEventListener('click', () => uploadInput.click());
  dz.addEventListener('keydown', (ev) => {
    if (ev.key === 'Enter' || ev.key === ' ') {
      ev.preventDefault();
      uploadInput.click();
    }
  });
  uploadInput.addEventListener('change', () => {
    uploadFiles(uploadInput.files);
    uploadInput.value = '';
  });
  for (const evt of ['dragenter', 'dragover']) {
    dz.addEventListener(evt, (ev) => {
      ev.preventDefault();
      dz.classList.add('dragover');
    });
  }
  for (const evt of ['dragleave', 'drop']) {
    dz.addEventListener(evt, (ev) => {
      ev.preventDefault();
      dz.classList.remove('dragover');
    });
  }
  dz.addEventListener('drop', (ev) => {
    if (ev.dataTransfer && ev.dataTransfer.files.length) {
      uploadFiles(ev.dataTransfer.files);
    }
  });

  // Modal
  $('modal-close').addEventListener('click', () => closeModal(null));
  $('modal-cancel').addEventListener('click', () => closeModal(null));
  $('modal-overlay').addEventListener('click', (ev) => {
    if (ev.target === $('modal-overlay')) closeModal(null);
  });
  $('modal-ok').addEventListener('click', () => {
    const value = $('modal-input').value;
    clearError('modal-error');
    closeModal(value);
  });
  $('modal-input').addEventListener('keydown', (ev) => {
    if (ev.key === 'Enter') {
      ev.preventDefault();
      const value = $('modal-input').value;
      clearError('modal-error');
      closeModal(value);
    }
  });
  document.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape' && !$('modal-overlay').classList.contains('hidden')) {
      closeModal(null);
    }
  });

  const restored = await tryRestoreSession();
  if (restored) {
    enterApp();
    await Promise.allSettled([loadDashboard(), loadConflictCount()]);
  } else {
    $('login-view').classList.remove('hidden');
  }
});
