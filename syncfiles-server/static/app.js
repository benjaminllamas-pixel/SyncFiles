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
    tr.innerHTML = `
      <td>${escapeHtml(d.device_name || d.device_id)}</td>
      <td>${escapeHtml(d.platform || '—')}</td>
      <td>${active ? '<span class="status-pill synced">Sí</span>' : '<span class="status-pill deleted">No</span>'}</td>
      <td>${fmtDate(d.last_seen_at)}</td>`;
    tbody.appendChild(tr);
  }
}

/* ============ Archivos ============ */
async function loadFiles() {
  clearError('files-error');
  try {
    const showDeleted = $('files-show-deleted').checked;
    const data = await apiJson(`/files/list${showDeleted ? '?include_deleted=true' : ''}`);
    setConnBadge(true);
    renderFiles(data.files || []);
  } catch (err) {
    setConnBadge(false);
    showError('files-error', err);
  }
}

const STATUS_LABELS = { synced: 'Sincronizado', deleted: 'Borrado', pending: 'Pendiente', queued: 'En cola', retry: 'Reintentando' };

function statusPill(status) {
  return `<span class="status-pill ${escapeHtml(status)}">${escapeHtml(STATUS_LABELS[status] || status)}</span>`;
}

function renderFiles(files) {
  const tbody = $('files-table').querySelector('tbody');
  tbody.innerHTML = '';
  $('files-empty').classList.toggle('hidden', files.length > 0);
  for (const f of files) {
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

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'btn tiny danger';
    deleteBtn.innerHTML = `<svg class="b-ico"><use href="#i-trash"/></svg>Borrar`;
    deleteBtn.disabled = f.status === 'deleted';
    deleteBtn.addEventListener('click', () => deleteFile(f));

    const downloadBtn2 = downloadBtn.cloneNode(true);
    downloadBtn2.addEventListener('click', () => downloadFile(f));
    const deleteBtn2 = deleteBtn.cloneNode(true);
    deleteBtn2.disabled = f.status === 'deleted';
    deleteBtn2.addEventListener('click', () => deleteFile(f));

    actions.append(downloadBtn, deleteBtn);
    secondary.append(downloadBtn2, deleteBtn2);

    tr.innerHTML = `
      <td class="path-cell">${escapeHtml(f.relative_path)}</td>
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

function base64ToBytes(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
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
      if (v === 'conflicts') loadConflicts();
      if (v === 'activity') loadActivity();
    });
  }

  $('files-refresh').addEventListener('click', loadFiles);
  $('files-show-deleted').addEventListener('change', loadFiles);
  $('conflicts-refresh').addEventListener('click', loadConflicts);
  $('activity-refresh').addEventListener('click', loadActivity);

  const restored = await tryRestoreSession();
  if (restored) {
    enterApp();
    await Promise.allSettled([loadDashboard(), loadConflictCount()]);
  } else {
    $('login-view').classList.remove('hidden');
  }
});
