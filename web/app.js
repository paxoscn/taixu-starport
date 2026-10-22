/* =========================================================================
   Taixu Starport — vanilla SPA
   State arrives as one JSON document from /api/state. All user-controlled
   strings pass through esc() before entering innerHTML.
   ========================================================================= */

'use strict';

// ---------------------------------------------------------------------------
// Globals
// ---------------------------------------------------------------------------

let S = null;
let token = localStorage.getItem('taixu_token') || '';
let adminToken = localStorage.getItem('taixu_admin') || '';
let activeTab = 'expedition';
let chatChannel = 'global';
let loginRegion = '';
let worldData = null;
let lastResult = null;
let lastError = '';
let refreshing = false;
let adminLoading = false;
let adminData = { overview: null, announcements: null, npcs: null, accounts: null, tasks: null, snapshot: null };
let npcPrompt = null;

const RARITY_COLORS = {
  common: '#c7d2e5',
  uncommon: '#5ee7a3',
  rare: '#5ee7ff',
  epic: '#b794ff',
  legendary: '#fbbf24',
  mythic: '#ff7ad9',
};

const STATUS_LABELS = {
  available: '可接取',
  active: '进行中',
  completed: '可领奖',
  claimed: '已领取',
  locked: '未解锁',
  expired: '已过期',
};

const KIND_LABELS = {
  news: '资讯',
  event: '活动',
  maintenance: '维护',
  promotion: '促销',
  system: '系统',
};

const REGION_STATUS_LABELS = {
  open: '开放',
  maintenance: '维护中',
  closed: '已关闭',
};

const SKIN_CATEGORY_LABELS = {
  character: '角色',
  weapon: '武器',
  avatar: '头像',
  frame: '头像框',
  badge: '徽章',
  background: '背景',
  emote: '表情',
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const $ = (sel, root = document) => root.querySelector(sel);
const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, (ch) => ({
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  }[ch]));
}

function num(value) {
  return Number(value || 0).toLocaleString('zh-CN');
}

function fmtTime(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}

function fmtDateTime(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleString('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit',
  });
}

function fmtDuration(ms) {
  const total = Math.max(0, Math.floor(ms / 1000));
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (d > 0) return `${d}天 ${h}小时`;
  if (h > 0) return `${h}小时 ${m}分`;
  if (m > 0) return `${m}分 ${s}秒`;
  return `${s}秒`;
}

function fmtRelative(ms) {
  if (!ms) return '—';
  const now = S?.server?.now_ms || Date.now();
  return ms <= now ? '现在' : fmtDuration(ms - now);
}

function pct(a, b) {
  if (!b) return 0;
  return Math.max(0, Math.min(100, Math.round((a / b) * 100)));
}

function itemDef(id) {
  return S?.catalogs?.items?.find((item) => item.id === id) || null;
}
function skinDef(id) {
  return S?.catalogs?.skins?.find((skin) => skin.id === id) || null;
}
function resourceDef(id) {
  return S?.catalogs?.resources?.find((resource) => resource.id === id) || null;
}
function currencyDef(id) {
  return S?.catalogs?.currencies?.find((currency) => currency.id === id)
    || { id, name: id, symbol: '•' };
}
function achName(id) {
  return S?.achievements?.find((achievement) => achievement.id === id)?.name || id;
}
function taskName(id) {
  return S?.tasks?.find((task) => task.id === id)?.name || id;
}
function regionName(id) {
  return S?.regions?.find((region) => region.id === id)?.name || id;
}
function metricLabel(metric) {
  return S?.catalogs?.metric_labels?.[metric] || metric;
}
function skinCategoryLabel(category) {
  if (!category) return '其他';
  if (category.startsWith('other:')) return `自定义 · ${category.slice(6)}`;
  return SKIN_CATEGORY_LABELS[category] || category;
}
function rarityClass(rarity) {
  return `r-${rarity || 'common'}`;
}
function skinName(id) {
  return skinDef(id)?.name || id;
}
function itemName(id) {
  return itemDef(id)?.name || id;
}
function person(id) {
  return S?.names?.[id] || { id, display_name: id };
}

function avatarHtml(who, size = '', extra = '') {
  const p = who || {};
  const avatar = p.avatar || {};
  const name = p.display_name || p.name || '?';
  const frame = avatar.frame ? skinDef(avatar.frame) : null;
  const badge = avatar.badge ? skinDef(avatar.badge) : null;
  const color = frame ? RARITY_COLORS[frame.rarity] : '';
  let inner;
  if (avatar.image && /^https?:\/\//i.test(avatar.image)) {
    inner = `<img src="${esc(avatar.image)}" alt="">`;
  } else if (avatar.image) {
    inner = esc(avatar.image);
  } else {
    inner = esc(name.slice(0, 1).toUpperCase());
  }
  const badgeHtml = badge ? '<span class="av-badge">✦</span>' : '';
  const style = color ? `border-color:${color};box-shadow:0 0 16px ${color}33` : '';
  return `<div class="avatar ${size}" style="${style}" ${extra}>${inner}${badgeHtml}</div>`;
}

function rewardRow(reward) {
  if (!reward) return '';
  const out = [];
  for (const grant of reward.currencies || []) {
    const def = currencyDef(grant.currency);
    out.push(`<span class="reward"><span>${esc(def.symbol)}</span>${esc(def.name)} <b>${num(grant.amount)}</b></span>`);
  }
  for (const grant of reward.resources || []) {
    const def = resourceDef(grant.resource);
    out.push(`<span class="reward resource">${esc(def?.name || grant.resource)} <b>${num(grant.amount)}</b></span>`);
  }
  for (const grant of reward.items || []) {
    const def = itemDef(grant.item);
    out.push(`<span class="reward"><span class="${rarityClass(def?.rarity)}">◆</span>${esc(def?.name || grant.item)} <b>×${num(grant.quantity)}</b></span>`);
  }
  for (const skin of reward.skins || []) {
    out.push(`<span class="reward skin">外观 · ${esc(skinName(skin))}</span>`);
  }
  if (reward.xp) out.push(`<span class="reward xp">经验 <b>+${num(reward.xp)}</b></span>`);
  return out.length ? `<div class="reward-row">${out.join('')}</div>` : '<span class="dim tiny">无奖励</span>';
}

function costRow(cost) {
  if (!cost) return '';
  const out = [];
  for (const grant of cost.currencies || []) {
    const def = currencyDef(grant.currency);
    out.push(`<span class="cost"><span>${esc(def.symbol)}</span>${esc(def.name)} <b>${num(grant.amount)}</b></span>`);
  }
  for (const grant of cost.resources || []) {
    const def = resourceDef(grant.resource);
    out.push(`<span class="cost">${esc(def?.name || grant.resource)} <b>${num(grant.amount)}</b></span>`);
  }
  return `<div class="cost-row">${out.join('')}</div>`;
}

function outcomeHtml(outcome) {
  if (!outcome) return '';
  const parts = [];
  for (const id of outcome.achievements || []) {
    parts.push(`<span class="pill completed">成就 · ${esc(achName(id))}</span>`);
  }
  for (const id of outcome.tasks || []) {
    parts.push(`<span class="pill completed">任务 · ${esc(taskName(id))}</span>`);
  }
  return parts.length
    ? `<div class="card-row" style="margin-top:7px">${parts.join('')}</div>`
    : '';
}

function statusPill(status) {
  const cls = ['available', 'active', 'completed', 'claimed', 'locked', 'expired'].includes(status)
    ? status : 'locked';
  return `<span class="pill ${cls}">${esc(STATUS_LABELS[status] || status)}</span>`;
}

function channelLabel(value) {
  if (value === 'global') return '全局';
  if (value === 'region' || value.startsWith('region:')) return '区域';
  if (value.startsWith('room:')) return '房间';
  if (value.startsWith('dm:')) return '私聊';
  return value;
}

function uniqueInventoryItems() {
  const counts = new Map();
  for (const slot of S?.inventory?.slots || []) {
    if (slot.item) counts.set(slot.item, (counts.get(slot.item) || 0) + slot.quantity);
  }
  return Array.from(counts.entries()).map(([id, quantity]) => {
    const def = itemDef(id);
    return { id, quantity, name: def?.name || id, rarity: def?.rarity || 'common' };
  });
}

function feedback(data) {
  if (!data) return;
  if (data.level_up) toast(`✦ 升级到 ${data.level_up} 级`, 'gold');
  if (data.outcome) {
    for (const id of data.outcome.achievements || []) toast(`成就完成：${achName(id)}`, 'gold');
    for (const id of data.outcome.tasks || []) toast(`任务完成：${taskName(id)}`, 'good');
  }
  if (data.result?.outcome) {
    for (const id of data.result.outcome.achievements || []) toast(`成就完成：${achName(id)}`, 'gold');
    for (const id of data.result.outcome.tasks || []) toast(`任务完成：${taskName(id)}`, 'good');
  }
}

// ---------------------------------------------------------------------------
// API + toasts
// ---------------------------------------------------------------------------

async function api(path, { method = 'GET', body } = {}) {
  const headers = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  if (token) headers.authorization = `Bearer ${token}`;
  if (adminToken) headers['x-admin-token'] = adminToken;
  const response = await fetch(path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  let data = null;
  try { data = await response.json(); } catch (_) { /* no body */ }
  if (!response.ok) {
    const error = new Error(data?.error || `请求失败 (${response.status})`);
    error.status = response.status;
    throw error;
  }
  return data;
}

async function post(path, body) {
  return api(path, { method: 'POST', body });
}

function toast(message, kind = '') {
  const host = $('#toasts');
  if (!host) return;
  const node = document.createElement('div');
  node.className = `toast ${kind}`;
  node.textContent = message;
  host.appendChild(node);
  setTimeout(() => node.remove(), 4600);
}

// ---------------------------------------------------------------------------
// Login
// ---------------------------------------------------------------------------

async function showLogin() {
  token = '';
  localStorage.removeItem('taixu_token');
  S = null;
  worldData = null;
  lastResult = null;
  chatChannel = 'global';
  const app = $('#app');
  app.innerHTML = `<div class="boot"><span class="pulse"></span><span>正在连接星港…</span></div>`;
  try {
    worldData = await fetch('/api/world').then((response) => response.json());
  } catch (_) {
    worldData = { regions: [], npcs: [] };
  }
  const openRegions = (worldData.regions || []).filter((region) => region.status === 'open');
  loginRegion = openRegions[0]?.id || '';
  renderLogin();
}

function renderLogin() {
  const regions = worldData?.regions || [];
  const regionCards = regions.map((region) => {
    const disabled = region.status !== 'open';
    const selected = region.id === loginRegion;
    const statusText = { open: '开放', maintenance: '维护中', closed: '已关闭' }[region.status] || region.status;
    return `
      <button type="button" class="login-region ${selected ? 'selected' : ''} ${disabled ? 'disabled' : ''}"
        data-region="${esc(region.id)}" ${disabled ? 'disabled' : ''}>
        <span>
          <span class="lr-name">${esc(region.name)}</span>
          <span class="lr-meta"> · ${region.players || 0}/${region.capacity ?? '∞'} 人 · ${esc(statusText)}</span>
        </span>
        <span class="lr-dot ${esc(region.status)}"></span>
      </button>`;
  }).join('');

  $('#app').innerHTML = `
    <div class="login-wrap">
      <div class="login-hero">
        <div class="login-kicker">TAIXU STARPORT · RUST BACKEND DEMO</div>
        <h1>太虚<br>星港</h1>
        <p class="sub">
          由 <b>Tai Xu</b> 游戏后端库驱动的多人星港：远征、商店、交易、房间、
          社交、外观、图鉴、排行榜、成就、任务、公告与 NPC Agent。
          所有经济操作在库内原子结算，状态以快照持久化。
        </p>
        <div class="login-features">
          <span>⚡ 资源惰性再生</span>
          <span>⇄ 玩家原子交易</span>
          <span>✦ 任务 / 成就 / 图鉴</span>
          <span>🛰 区域 / 房间 / 聊天</span>
          <span>🤖 NPC 结构化动作</span>
        </div>
      </div>
      <div class="login-card">
        <h2>登舰</h2>
        <p class="muted small">演示用无密码登录。账号由宿主创建，Taixu 只接收可信的 AccountId。</p>
        <form data-form="login" class="stack" style="margin-top:16px">
          <div class="field">
            <label>舰员代号</label>
            <input name="name" maxlength="24" placeholder="例如 Nova" autocomplete="nickname" required>
          </div>
          <div class="field">
            <label>初始区域</label>
            <div class="login-regions">${regionCards || '<div class="empty">没有可用区域</div>'}</div>
            <input type="hidden" name="region" id="login-region" value="${esc(loginRegion)}">
          </div>
          <button class="btn primary wide" type="submit">进入星港 →</button>
        </form>
        <p class="dim tiny" style="margin-top:12px">
          已有账号直接输入同名代号即可返回。演示环境不校验密码。
        </p>
      </div>
    </div>`;
}

async function handleLogin(form) {
  const formData = new FormData(form);
  const name = String(formData.get('name') || '').trim();
  const region = String(formData.get('region') || '').trim();
  if (!name) return toast('请输入舰员代号', 'bad');
  if (!region) return toast('请选择一个开放区域', 'bad');
  try {
    const response = await fetch('/api/login', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ name, region }),
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(data.error || '登录失败');
    token = data.token;
    localStorage.setItem('taixu_token', token);
    S = null;
    activeTab = 'expedition';
    chatChannel = 'global';
    toast(`欢迎登舰，${data.account.display_name}`, 'good');
    await refresh(true);
  } catch (error) {
    toast(error.message, 'bad');
  }
}

// ---------------------------------------------------------------------------
// Shell / refresh
// ---------------------------------------------------------------------------

async function refresh(force = false) {
  if (!token) {
    if (!$('.login-wrap')) showLogin();
    return;
  }
  if (refreshing) return;
  refreshing = true;
  try {
    const data = await api(`/api/state?channel=${encodeURIComponent(chatChannel)}`);
    S = data;
    if (S.chat?.channel) chatChannel = S.chat.channel;
    const shell = $('.shell');
    if (!shell || force) {
      renderAll();
    } else {
      renderHud();
      renderNav();
      renderChat();
      const focused = document.activeElement;
      const editing = focused && focused.closest('#main')
        && ['INPUT', 'TEXTAREA', 'SELECT'].includes(focused.tagName);
      if (!editing) renderTab();
      maybeLoadAdmin();
    }
    lastError = '';
  } catch (error) {
    if (error.status === 401) {
      S = null;
      showLogin();
    } else if (error.message !== lastError) {
      lastError = error.message;
      toast(error.message, 'bad');
    }
  } finally {
    refreshing = false;
  }
}

function renderAll() {
  if (!S) return;
  if (!$('.shell')) renderShell();
  renderHud();
  renderNav();
  renderTab();
  renderChat();
  maybeLoadAdmin();
}

function renderShell() {
  $('#app').innerHTML = `
    <div class="shell">
      <aside class="sidebar">
        <div class="brand">
          <div class="brand-mark">✦</div>
          <div>
            <div class="brand-name">太虚星港</div>
            <div class="brand-sub">TAIXU STARPORT</div>
          </div>
        </div>
        <nav class="nav" id="nav"></nav>
        <div class="sidebar-foot">
          <div class="hint">服务器时间</div>
          <div class="mono" id="sidebar-time">—</div>
          <div class="hint">快照</div>
          <div class="mono" title="${esc(S?.server?.store || '')}">${esc(S?.server?.store || '—')}</div>
          <div class="mono">v${esc(S?.server?.version || '0.1.0')} · ${S?.server?.manual_clock ? 'MANUAL CLOCK' : 'SYSTEM CLOCK'}</div>
        </div>
      </aside>
      <main class="main">
        <header class="hud" id="hud"></header>
        <section id="main"></section>
      </main>
      <aside class="chatpane" id="chatpane"></aside>
    </div>`;
}

function renderHud() {
  const hud = $('#hud');
  if (!hud || !S) return;
  const me = S.me;
  const wallet = S.wallet;
  const region = S.regions.find((entry) => entry.id === me.region);
  const defs = wallet.resource_defs || [];
  const currencies = ['gold', 'gems', 'astral'];
  const currencyChips = currencies.map((id) => {
    const def = currencyDef(id);
    const amount = wallet.currencies[id] || 0;
    if (id === 'astral' && amount === 0) return '';
    return `<span class="chip ${id === 'gold' ? 'gold' : id === 'gems' ? 'violet' : 'cyan'}">
      <span>${esc(def.symbol)}</span><b>${num(amount)}</b><span class="dim">${esc(def.name)}</span>
    </span>`;
  }).join('');
  const resourceBars = defs.map((def) => {
    const value = wallet.resources[def.id] || 0;
    const cap = def.cap ?? Math.max(value, 1);
    const width = pct(value, cap);
    return `<div class="resource">
      <div class="rim"><span>${esc(def.name)}</span><span><b>${num(value)}</b> / ${cap === null ? '∞' : num(cap)} <span class="dim">+${num(def.regen_per_hour)}/h</span></span></div>
      <div class="ribar"><span style="width:${width}%"></span></div>
    </div>`;
  }).join('');
  const roomChip = S.room
    ? `<span class="chip cyan">🛰 ${esc(S.room.name)} · ${S.room.player_count}/${S.room.capacity}</span>`
    : '';
  hud.innerHTML = `
    <div class="hud-top">
      ${avatarHtml(me, 'lg')}
      <div class="hud-id">
        <div class="hud-name-row">
          <span class="hud-name">${esc(me.display_name)}</span>
          ${me.avatar?.title ? `<span class="hud-title">「${esc(me.avatar.title)}」</span>` : ''}
          <span class="level-pill">Lv.${me.level}</span>
          ${S.admin ? '<span class="pill hidden">ADMIN</span>' : ''}
          ${S.server.manual_clock ? '<span class="pill locked">MANUAL CLOCK</span>' : ''}
        </div>
        <div class="hud-meta">
          <span>${esc(region?.name || me.region)}</span>
          <span>· ${esc(REGION_STATUS_LABELS[region?.status] || region?.status || '')}</span>
          <span>· 在线 ${S.server.online}/${S.server.accounts}</span>
          <span>· XP ${num(me.xp)} / ${num(me.xp_next)}</span>
        </div>
        <div class="xpbar" style="margin-top:5px"><span style="width:${me.xp_pct}%"></span></div>
      </div>
      <div class="hud-actions">
        <button type="button" class="btn small ghost" data-act="refresh">↻ 刷新</button>
        <button type="button" class="btn small ghost" data-act="logout">断开</button>
      </div>
    </div>
    <div class="hud-stats">
      ${currencyChips}
      ${resourceBars}
      ${roomChip}
      <span class="chip ghost">会话 ${S.server.sessions}</span>
    </div>`;
  const time = $('#sidebar-time');
  if (time) time.textContent = fmtDateTime(S.server.now_ms);
}

function renderNav() {
  const nav = $('#nav');
  if (!nav || !S) return;
  const tasksReady = S.tasks.filter((task) => task.status === 'completed').length;
  const achievementsReady = S.achievements.filter((achievement) => achievement.completed && !achievement.claimed).length;
  const openTrades = S.trades.filter((trade) => !trade.done).length;
  const items = [
    ['expedition', '⚡', '远征控制台', ''],
    ['shops', '🛒', '商店', ''],
    ['tasks', '📋', '任务', tasksReady ? `<span class="nav-badge gold">${tasksReady}</span>` : ''],
    ['achievements', '🏆', '成就', achievementsReady ? `<span class="nav-badge gold">${achievementsReady}</span>` : ''],
    ['collection', '📚', '图鉴', ''],
    ['leaderboards', '📈', '排行榜', ''],
    ['social', '👥', '社交', S.friend_requests.length ? `<span class="nav-badge">${S.friend_requests.length}</span>` : ''],
    ['rooms', '🛰', '房间', ''],
    ['trades', '⇄', '交易', openTrades ? `<span class="nav-badge">${openTrades}</span>` : ''],
    ['profile', '🎨', '外观与档案', ''],
    ['announcements', '📣', '公告', S.unread_announcements ? `<span class="nav-badge red">${S.unread_announcements}</span>` : ''],
    ['admin', '🛠', '运营后台', ''],
  ];
  nav.innerHTML = items.map(([tab, icon, label, badge]) => `
    <button type="button" class="nav-item ${activeTab === tab ? 'active' : ''}" data-tab="${tab}">
      <span class="nav-icon">${icon}</span>
      <span class="nav-label">${label}</span>
      ${badge}
    </button>`).join('');
}

function maybeLoadAdmin() {
  if (activeTab !== 'admin' || !S?.admin || adminData.overview || adminLoading) return;
  adminLoading = true;
  loadAdminData('core').finally(() => { adminLoading = false; });
}

async function loadAdminData(which) {
  if (!S?.admin) return;
  try {
    if (which === 'core') {
      const [overview, announcements, npcs] = await Promise.all([
        api('/api/admin/overview'),
        api('/api/admin/announcements'),
        api('/api/admin/npcs'),
      ]);
      adminData.overview = overview;
      adminData.announcements = announcements;
      adminData.npcs = npcs;
    } else if (which === 'accounts') {
      adminData.accounts = await api('/api/admin/accounts');
    } else if (which === 'tasks') {
      adminData.tasks = await api('/api/admin/tasks');
    }
    const focused = document.activeElement;
    const editing = focused && focused.closest('#main')
      && ['INPUT', 'TEXTAREA', 'SELECT'].includes(focused.tagName);
    if (!editing) renderTab();
  } catch (error) {
    toast(error.message, 'bad');
  }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

function renderTab() {
  const main = $('#main');
  if (!main || !S) return;
  const renderers = {
    expedition: tabExpedition,
    shops: tabShops,
    tasks: tabTasks,
    achievements: tabAchievements,
    collection: tabCollection,
    leaderboards: tabLeaderboards,
    social: tabSocial,
    rooms: tabRooms,
    trades: tabTrades,
    profile: tabProfile,
    announcements: tabAnnouncements,
    admin: tabAdmin,
  };
  const renderer = renderers[activeTab] || tabExpedition;
  main.innerHTML = renderer();
}

function tabExpedition() {
  const me = S.me;
  const wallet = S.wallet;
  const inv = S.inventory;
  const region = S.regions.find((entry) => entry.id === me.region);
  const energy = wallet.resources.energy || 0;
  const focus = wallet.resources.focus || 0;
  const actions = [
    {
      kind: 'expedition', icon: '🚀', label: '深空远征', glow: 'rgba(94,231,255,.22)',
      desc: '消耗体力穿越极光航道，带回金币、矿石与经验。',
      energy: 10, focus: 0,
    },
    {
      kind: 'gather', icon: '🪨', label: '近地采集', glow: 'rgba(52,211,153,.2)',
      desc: '低风险采集矿石与药水，适合稳定积累材料。',
      energy: 5, focus: 0,
    },
    {
      kind: 'boss', icon: '☄', label: '裂隙首领', glow: 'rgba(251,113,133,.22)',
      desc: '高消耗高回报，有机会掉落遗物与奖杯。',
      energy: 25, focus: 0,
    },
    {
      kind: 'secret', icon: '🌌', label: '太虚窥探', glow: 'rgba(167,139,250,.24)',
      desc: '消耗体力与专注，听见低语并触发隐藏任务/成就。',
      energy: 15, focus: 3,
    },
  ];
  const actionCards = actions.map((action) => {
    const disabled = energy < action.energy || focus < action.focus;
    const costs = `<span class="cost">体力 <b>${action.energy}</b></span>`
      + (action.focus ? `<span class="cost">专注 <b>${action.focus}</b></span>` : '');
    return `<div class="action-card" style="--glow:${action.glow}">
      <div class="ac-icon">${action.icon}</div>
      <h3>${action.label}</h3>
      <div class="ac-desc">${action.desc}</div>
      <div class="cost-row" style="margin-bottom:9px">${costs}</div>
      <button type="button" class="btn primary wide" data-act="explore" data-kind="${action.kind}" ${disabled ? 'disabled' : ''}>
        ${disabled ? '资源不足' : '开始'}
      </button>
    </div>`;
  }).join('');

  const slots = (inv.slots || []).map((slot) => {
    if (!slot.item) return '<div class="inv-slot empty"></div>';
    const def = itemDef(slot.item);
    return `<div class="inv-slot" data-r="${esc(def?.rarity || 'common')}" title="${esc(def?.description || '')}">
      <div class="is-name ${rarityClass(def?.rarity)}">${esc(def?.name || slot.item)}</div>
      <div class="is-id">${esc(slot.item)}</div>
      <span class="is-qty">×${num(slot.quantity)}</span>
    </div>`;
  }).join('');

  const inventoryItems = uniqueInventoryItems();
  const itemOptions = inventoryItems.map((entry) =>
    `<option value="${esc(entry.id)}">${esc(entry.name)} ×${num(entry.quantity)}</option>`).join('');
  const craftOptions = [
    ['rift_shard', '裂隙碎片（3 矿石）'],
    ['relic', '太虚遗物（5 碎片 + 2 药水）'],
  ].map(([value, label]) => `<option value="${value}">${label}</option>`).join('');

  const result = lastResult ? `
    <div class="result-card">
      <div class="card-row" style="justify-content:space-between">
        <b>${esc(lastResult.label || '行动结果')}</b>
        <span class="dim tiny">${fmtTime(Date.now())}</span>
      </div>
      <div class="small" style="margin:6px 0 9px">${esc(lastResult.narrative || '')}</div>
      ${rewardRow(lastResult.reward)}
      ${outcomeHtml(lastResult.outcome)}
      ${lastResult.speed_secs ? `<span class="pill" style="margin-top:6px">竞速 ${lastResult.speed_secs} 秒</span>` : ''}
    </div>` : '';

  const regionCards = S.regions.map((entry) => {
    const full = entry.capacity != null && entry.players >= entry.capacity;
    const disabled = entry.is_mine || entry.status !== 'open' || full;
    const reason = entry.is_mine ? '当前区域' : full ? '区域已满' : entry.status !== 'open'
      ? REGION_STATUS_LABELS[entry.status] : '迁入';
    return `<div class="card">
      <div class="card-top">
        <div>
          <h3>${esc(entry.name)} <span class="pill ${entry.status === 'open' ? 'open' : 'locked'}">${esc(REGION_STATUS_LABELS[entry.status] || entry.status)}</span></h3>
          <div class="card-sub">${entry.players}/${entry.capacity ?? '∞'} 人 · 端点 ${esc(entry.endpoint || '未公布')}</div>
        </div>
        ${entry.is_mine ? '<span class="pill open">当前</span>'
          : `<button type="button" class="btn small ${disabled ? 'ghost' : 'primary'}" data-act="travel" data-region="${esc(entry.id)}" ${disabled ? 'disabled' : ''}>${esc(reason)}</button>`}
      </div>
    </div>`;
  }).join('');

  return `
    <div class="panel">
      <div class="panel-head">
        <div>
          <h2>远征控制台</h2>
          <div class="muted small">${esc(region?.name || me.region)} · ${region?.status === 'open' ? '开放' : '受限'} · ${region?.players || 0} 名玩家</div>
        </div>
        <div class="card-row">
          <span class="chip green">体力 <b>${num(energy)}</b></span>
          <span class="chip violet">专注 <b>${num(focus)}</b></span>
        </div>
      </div>
      <div class="panel-body stack">
        <div class="grid four">${actionCards}</div>
        ${result}
      </div>
    </div>
    <div class="panel">
      <div class="panel-head">
        <div>
          <h2>货舱</h2>
          <div class="muted small">${inv.used_slots}/${inv.capacity} 格已用 · ${inv.free_slots} 空位 · 固定槽位自动叠堆</div>
        </div>
        <button type="button" class="btn small gold" data-act="inventory-upgrade">扩容 +10 格</button>
      </div>
      <div class="panel-body stack">
        <div class="inv-grid">${slots}</div>
        <div class="divider"></div>
        <div class="grid two">
          <div class="card">
            <h4>合成</h4>
            <div class="card-sub">消耗材料，获得更高阶物品与经验。</div>
            <form data-form="craft" style="margin-top:9px">
              <div class="row">
                <div class="field"><label>配方</label><select name="recipe">${craftOptions}</select></div>
                <button class="btn primary" type="submit">合成</button>
              </div>
            </form>
          </div>
          <div class="card">
            <h4>出售</h4>
            <div class="card-sub">矿石 8 / 药水 15 / 碎片 60 / 遗物 300 金币。</div>
            <form data-form="sell" style="margin-top:9px">
              <div class="row">
                <div class="field"><label>物品</label>
                  <select name="item">${itemOptions || '<option value="">背包为空</option>'}</select>
                </div>
                <div class="field narrow"><label>数量</label><input type="number" name="quantity" min="1" value="1"></div>
                <button class="btn" type="submit" ${itemOptions ? '' : 'disabled'}>出售</button>
              </div>
            </form>
          </div>
        </div>
        <div class="notice">
          提示：远征、采集、首领战与窥探会推进成就和所有进行中任务；「太虚窥探」会揭示隐藏任务。
          任务奖励与背包扩容由宿主组合流程完成，服务器用互斥锁 + 失败补偿保证一致性。
        </div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-head">
        <div><h2>星图 · 区域迁移</h2><div class="muted small">跨区迁移会自动退出当前房间；目标区域必须开放且容量未满。</div></div>
        <span class="pill">${S.regions.length} 个节点</span>
      </div>
      <div class="panel-body"><div class="grid three">${regionCards}</div></div>
    </div>`;
}

function tabShops() {
  const shops = S.shops || [];
  if (!shops.length) return '<div class="empty">暂无商店</div>';
  const cards = shops.map((shop) => {
    const offers = (shop.offers || []).map((offer) => {
      const disabled = !offer.available;
      const limitReached = offer.per_account_limit != null && offer.purchased >= offer.per_account_limit;
      const out = offer.remaining_stock != null && offer.remaining_stock <= 0;
      const buyDisabled = disabled || limitReached || out;
      const stockBar = offer.stock != null
        ? `<div class="stockbar"><span style="width:${pct(offer.remaining_stock, offer.stock)}%"></span></div>
           <div class="tiny dim">剩余 ${num(offer.remaining_stock)} / ${num(offer.stock)}</div>` : '';
      const limitText = offer.per_account_limit != null
        ? `<span class="pill ${limitReached ? 'locked' : ''}">限购 ${offer.purchased}/${offer.per_account_limit}</span>` : '';
      const reasons = [];
      if (!shop.open) reasons.push('商店未营业');
      if (!offer.region_ok) reasons.push('区域不符');
      if (!offer.window_ok) reasons.push('不在时间窗');
      if (limitReached) reasons.push('已达限购');
      if (out) reasons.push('已售罄');
      return `<div class="offer ${buyDisabled ? 'off' : ''}">
        <div class="offer-top">
          <div>
            <div class="offer-name">${esc(offer.name)}</div>
            <div class="offer-desc">${esc(offer.description || '')}</div>
          </div>
          ${buyDisabled ? `<span class="pill locked">${esc(reasons.join(' · ') || '不可购买')}</span>` : '<span class="pill open">可购买</span>'}
        </div>
        <div>${costRow(offer.cost)}</div>
        <div>${rewardRow(offer.reward)}</div>
        ${stockBar}
        <div class="offer-meta">
          ${limitText}
          ${(offer.tags || []).map((tag) => `<span class="pill">${esc(tag)}</span>`).join('')}
          ${offer.ends_at_ms ? `<span class="pill">至 ${fmtDateTime(offer.ends_at_ms)}</span>` : ''}
        </div>
        <div class="offer-buy">
          <input id="buy-qty-${esc(shop.id)}-${esc(offer.id)}" type="number" min="1" value="1" ${buyDisabled ? 'disabled' : ''}>
          <button type="button" class="btn primary small" data-act="shop-buy" data-shop="${esc(shop.id)}" data-offer="${esc(offer.id)}" ${buyDisabled ? 'disabled' : ''}>购买</button>
        </div>
      </div>`;
    }).join('');
    return `<div class="panel">
      <div class="panel-head">
        <div>
          <h2>${esc(shop.name)}</h2>
          <div class="muted small">${esc(shop.description || '')}</div>
        </div>
        <div class="card-row">
          <span class="pill ${shop.open ? 'open' : 'locked'}">${shop.open ? '营业中' : '未营业'}</span>
          ${shop.closes_at_ms ? `<span class="pill">至 ${fmtDateTime(shop.closes_at_ms)}</span>` : ''}
          ${shop.regions ? `<span class="pill">限定 ${shop.regions.map(regionName).map(esc).join(' / ')}</span>` : ''}
        </div>
      </div>
      <div class="panel-body"><div class="grid two">${offers || '<div class="empty">暂无商品</div>'}</div></div>
    </div>`;
  }).join('');
  return `<div class="stack">${cards}</div>`;
}

function taskHintHtml(task) {
  const hint = S.task_hints?.[task.id];
  const bits = [];
  if (task.min_level) bits.push(`需要 Lv.${task.min_level}`);
  if (hint?.requires?.length) bits.push(`前置：${hint.requires.map(taskName).map(esc).join('、')}`);
  if (hint?.regions?.length) bits.push(`区域：${hint.regions.map(regionName).map(esc).join('、')}`);
  if (task.expires_at_ms) bits.push(`截止 ${fmtDateTime(task.expires_at_ms)}（剩余 ${fmtRelative(task.expires_at_ms)}）`);
  if (task.next_available_at_ms) bits.push(`下次可接 ${fmtDateTime(task.next_available_at_ms)}（${fmtRelative(task.next_available_at_ms)}）`);
  if (task.repeat?.kind === 'cooldown') {
    const ms = task.repeat.duration_ms;
    const label = ms >= 7 * 86400000 ? '每周' : ms >= 86400000 ? '每日' : '冷却';
    bits.push(`${label}重复（${fmtDuration(ms)}）`);
  }
  return bits.length ? `<div class="task-hint">${bits.join(' · ')}</div>` : '';
}

function objectiveHtml(view) {
  const obj = view.objective;
  let label = '';
  let extra = '';
  if (obj.type === 'metric') {
    label = metricLabel(obj.metric);
    extra = `<span class="dim">(${esc(obj.metric)})</span>`;
  } else if (obj.type === 'item') {
    label = `持有 ${itemName(obj.item)}`;
  } else if (obj.type === 'skin') {
    label = `拥有外观 ${skinName(obj.skin)}`;
  } else if (obj.type === 'achievement') {
    label = `完成成就 ${achName(obj.achievement)}`;
  } else {
    label = obj.type;
  }
  return `<div class="objective ${view.complete ? 'complete' : ''}">
    <div class="o-top"><span>${esc(label)} ${extra}</span><b>${num(view.progress)} / ${num(view.target)}</b></div>
    <div class="bar ${view.complete ? 'green' : ''}"><span style="width:${pct(view.progress, view.target)}%"></span></div>
  </div>`;
}

function taskCard(task) {
  const canAccept = task.status === 'available' || task.status === 'expired';
  const canClaim = task.status === 'completed';
  const canAbandon = task.status === 'active' || task.status === 'completed';
  // Library finding (second-round): after a repeatable task's cooldown, the
  // TaskView is Available but still carries the previous cycle's metric
  // counters/timestamps. Present a fresh metric cycle until re-accepting.
  const staleCycle = task.status === 'available' && task.claimed_at_ms != null;
  const objectives = staleCycle
    ? task.objectives.map((view) => view.objective.type === 'metric'
      ? { ...view, progress: 0, complete: false }
      : view)
    : task.objectives;
  const hintTask = staleCycle
    ? { ...task, expires_at_ms: null, next_available_at_ms: null }
    : task;
  return `<div class="card task ${task.status}">
    <div class="task-head">
      <div>
        <div class="task-title">
          <h3>${esc(task.name)}</h3>
          <span class="task-cat">${esc(task.category)}</span>
          ${task.hidden ? '<span class="pill hidden">隐藏</span>' : ''}
        </div>
        <div class="task-desc">${esc(task.description || '')}</div>
      </div>
      ${statusPill(task.status)}
    </div>
    <div class="objectives">${objectives.map(objectiveHtml).join('')}</div>
    ${taskHintHtml(hintTask)}
    ${staleCycle ? '<div class="task-hint">冷却已结束 · 接取新一轮会重置指标进度</div>' : ''}
    <div class="task-foot">
      <div>${rewardRow(task.reward)}</div>
      <div class="card-actions">
        ${canAccept ? `<button type="button" class="btn primary small" data-act="task-accept" data-task="${esc(task.id)}">接取</button>` : ''}
        ${canClaim ? `<button type="button" class="btn gold small" data-act="task-claim" data-task="${esc(task.id)}">领取奖励</button>` : ''}
        ${canAbandon ? `<button type="button" class="btn danger small" data-act="task-abandon" data-task="${esc(task.id)}">放弃</button>` : ''}
      </div>
    </div>
  </div>`;
}

function tabTasks() {
  const groups = [
    ['completed', '可领奖', '目标已完成，奖励等待领取'],
    ['active', '进行中', '失败/放弃会清空本轮进度'],
    ['available', '可接取', '满足全部前置条件'],
    ['locked', '未解锁', '等级、前置、区域或时间窗未满足'],
    ['expired', '已过期', '可以重新接取'],
    ['claimed', '已领取 / 冷却', '重复任务在冷却结束后会重新出现'],
  ];
  const sections = groups.map(([status, title, desc]) => {
    const tasks = S.tasks.filter((task) => task.status === status);
    if (!tasks.length) return '';
    return `<div class="panel">
      <div class="panel-head"><div><h2>${title}</h2><div class="muted small">${desc}</div></div><span class="pill">${tasks.length}</span></div>
      <div class="panel-body stack">${tasks.map(taskCard).join('')}</div>
    </div>`;
  }).filter(Boolean).join('');
  return sections || '<div class="empty">暂无可见任务。完成远征或窥探会揭示更多内容。</div>';
}

function tabAchievements() {
  const achievements = [...S.achievements].sort((a, b) => {
    const score = (achievement) => achievement.completed && !achievement.claimed ? 0
      : achievement.completed ? 2 : 1;
    return score(a) - score(b);
  });
  if (!achievements.length) return '<div class="empty">暂无成就</div>';
  return `<div class="panel">
    <div class="panel-head">
      <div><h2>成就</h2><div class="muted small">指标由游戏事件推进；隐藏成就在完成后才会出现。</div></div>
      <span class="pill">${achievements.filter((achievement) => achievement.completed && !achievement.claimed).length} 可领取</span>
    </div>
    <div class="panel-body"><div class="grid two">${achievements.map((achievement) => {
      const claimable = achievement.completed && !achievement.claimed;
      return `<div class="card ach ${achievement.completed ? 'done' : ''} ${achievement.claimed ? 'claimed' : ''}">
        <div class="ach-icon">${achievement.completed ? '🏆' : '◇'}</div>
        <div class="ach-body">
          <div class="task-title">
            <h3>${esc(achievement.name)}</h3>
            ${achievement.hidden ? '<span class="pill hidden">隐藏</span>' : ''}
            ${achievement.claimed ? '<span class="pill claimed">已领取</span>' : achievement.completed ? '<span class="pill completed">可领取</span>' : ''}
          </div>
          <div class="task-desc">${esc(achievement.description || '')}</div>
          <div class="objective" style="margin-top:8px">
            <div class="o-top"><span>${esc(metricLabel(achievement.metric))}</span><b>${num(achievement.progress)} / ${num(achievement.target)}</b></div>
            <div class="bar ${achievement.completed ? 'green' : ''}"><span style="width:${pct(achievement.progress, achievement.target)}%"></span></div>
          </div>
          <div class="card-actions">
            ${rewardRow(achievement.reward)}
            ${claimable ? `<button type="button" class="btn gold small" data-act="achievement-claim" data-id="${esc(achievement.id)}">领取</button>` : ''}
          </div>
        </div>
      </div>`;
    }).join('')}</div></div>
  </div>`;
}

function tabCollection() {
  if (!S.collections.length) return '<div class="empty">暂无图鉴</div>';
  return `<div class="stack">${S.collections.map((collection) => {
    const def = collection.definition;
    const progress = collection.progress;
    const milestones = progress.milestones.map((milestone) => {
      const reward = def.milestones[milestone.index]?.reward;
      const canClaim = milestone.unlocked && !milestone.claimed;
      return `<div class="milestone ${milestone.unlocked ? 'unlocked' : ''} ${milestone.claimed ? 'claimed' : ''}">
        <div>
          <b>${milestone.required} 件</b>
          <span class="dim small"> · ${milestone.unlocked ? (milestone.claimed ? '已领取' : '可领取') : '未解锁'}</span>
          <div style="margin-top:5px">${rewardRow(reward)}</div>
        </div>
        ${canClaim ? `<button type="button" class="btn gold small" data-act="collection-claim" data-collection="${esc(def.id)}" data-milestone="${milestone.index}">领取</button>` : ''}
      </div>`;
    }).join('');
    const entries = collection.entries.map((entry) => `
      <div class="entry ${entry.owned ? 'owned' : ''}">
        <span>${entry.owned ? '✔' : '○'}</span>
        <div style="min-width:0">
          <div>${esc(entry.name)}</div>
          <div class="e-kind">${esc(entry.kind)} · ${esc(entry.id)}</div>
        </div>
      </div>`).join('');
    return `<div class="panel">
      <div class="panel-head">
        <div><h2>${esc(def.name)}</h2><div class="muted small">${esc(def.description || '')}</div></div>
        <span class="chip gold">${progress.collected} / ${progress.total}</span>
      </div>
      <div class="panel-body stack">
        <div class="bar gold"><span style="width:${pct(progress.collected, progress.total)}%"></span></div>
        <div class="entries">${entries}</div>
        <div class="stack">${milestones}</div>
        <div class="notice">进度从实时状态推导：材料需在背包中、外观需拥有、成就需完成。领取过的里程碑不会重复发放。</div>
      </div>
    </div>`;
  }).join('')}</div>`;
}

function tabLeaderboards() {
  if (!S.leaderboards.length) return '<div class="empty">暂无排行榜</div>';
  return `<div class="stack">${S.leaderboards.map((board) => {
    const rows = board.top.map((entry) => {
      const rankClass = entry.rank === 1 ? 'top1' : entry.rank === 2 ? 'top2' : entry.rank === 3 ? 'top3' : '';
      return `<tr class="${entry.account === S.me.id ? 'me' : ''}">
        <td class="rank ${rankClass}">#${entry.rank}</td>
        <td>${esc(entry.name)}${entry.account === S.me.id ? ' <span class="pill">我</span>' : ''}</td>
        <td class="score">${num(entry.score)}${board.order === 'lower_is_better' ? ' 秒' : ''}</td>
      </tr>`;
    }).join('');
    return `<div class="panel">
      <div class="panel-head">
        <div>
          <h2>${esc(board.name)}</h2>
          <div class="muted small">${esc(board.description || '')}</div>
        </div>
        <div class="card-row">
          <span class="pill">${board.order === 'higher_is_better' ? '越高越好' : '越低越好'}</span>
          <span class="pill">赛季 ${board.season}</span>
          ${board.my_rank ? `<span class="pill open">我的排名 #${board.my_rank}</span>` : ''}
          ${board.my_score != null ? `<span class="pill">我的分数 ${num(board.my_score)}</span>` : ''}
          ${S.admin ? `<button type="button" class="btn tiny danger" data-act="admin-reset-board" data-board="${esc(board.id)}">重置赛季</button>` : ''}
        </div>
      </div>
      <div class="panel-body">
        ${rows ? `<table class="table"><thead><tr><th>排名</th><th>玩家</th><th style="text-align:right">分数</th></tr></thead><tbody>${rows}</tbody></table>` : '<div class="empty">还没有人上榜</div>'}
      </div>
    </div>`;
  }).join('')}</div>`;
}

function tabSocial() {
  const friendIds = new Set(S.friends.map((entry) => entry.id));
  const blocked = new Set(S.blocked.map((entry) => entry.id));
  const outgoing = new Set(S.outgoing_requests.map((entry) => entry.to));
  const known = Object.values(S.names || {}).filter((entry) =>
    entry.id !== S.me.id && !entry.is_system);
  const directory = known.map((entry) => {
    const isFriend = friendIds.has(entry.id);
    const isBlocked = blocked.has(entry.id);
    const pending = outgoing.has(entry.id);
    return `<div class="person" data-search="${esc((entry.display_name + ' ' + entry.id).toLowerCase())}">
      ${avatarHtml(entry, 'sm')}
      <div class="person-main">
        <div class="person-name"><span class="online-dot ${entry.online ? '' : 'off'}"></span>${esc(entry.display_name)}
          ${entry.id === S.me.id ? '<span class="pill">我</span>' : ''}
        </div>
        <div class="person-meta">Lv.${entry.level || 1} · ${esc(regionName(entry.region))} · ${esc(entry.id)}</div>
      </div>
      <div class="card-row">
        ${!isFriend && !isBlocked && !pending ? `<button type="button" class="btn tiny" data-act="friend-request" data-account="${esc(entry.id)}">加好友</button>` : ''}
        ${pending ? '<span class="pill">已申请</span>' : ''}
        ${isFriend ? `<button type="button" class="btn tiny" data-act="friend-dm" data-account="${esc(entry.id)}">私聊</button>` : ''}
        ${isFriend ? `<button type="button" class="btn tiny" data-act="friend-trade" data-account="${esc(entry.id)}">交易邀请</button>` : ''}
        ${!isBlocked && entry.id !== S.me.id ? `<button type="button" class="btn tiny danger" data-act="friend-block" data-account="${esc(entry.id)}">屏蔽</button>` : ''}
        ${isBlocked ? `<button type="button" class="btn tiny" data-act="friend-unblock" data-account="${esc(entry.id)}">解除屏蔽</button>` : ''}
      </div>
    </div>`;
  }).join('');

  const friends = S.friends.map((friend) => `
    <div class="person">
      ${avatarHtml(friend, 'sm')}
      <div class="person-main">
        <div class="person-name"><span class="online-dot ${friend.online ? '' : 'off'}"></span>${esc(friend.display_name)}</div>
        <div class="person-meta">Lv.${friend.level || 1} · ${esc(regionName(friend.region))}</div>
      </div>
      <div class="card-row">
        <button type="button" class="btn tiny" data-act="friend-dm" data-account="${esc(friend.id)}">私聊</button>
        <button type="button" class="btn tiny" data-act="friend-trade" data-account="${esc(friend.id)}">交易邀请</button>
        <button type="button" class="btn tiny danger" data-act="friend-remove" data-account="${esc(friend.id)}">删除</button>
      </div>
    </div>`).join('');

  const requests = S.friend_requests.map((request) => `
    <div class="person">
      ${avatarHtml(person(request.from), 'sm')}
      <div class="person-main">
        <div class="person-name">${esc(request.from_name)}</div>
        <div class="person-meta">${esc(request.message || '请求添加你为好友')} · ${fmtDateTime(request.created_at_ms)}</div>
      </div>
      <div class="card-row">
        <button type="button" class="btn tiny primary" data-act="friend-accept" data-account="${esc(request.from)}">接受</button>
        <button type="button" class="btn tiny danger" data-act="friend-decline" data-account="${esc(request.from)}">拒绝</button>
      </div>
    </div>`).join('');

  const outgoingRows = S.outgoing_requests.map((request) => `
    <div class="person">
      ${avatarHtml(person(request.to), 'sm')}
      <div class="person-main">
        <div class="person-name">${esc(request.to_name)}</div>
        <div class="person-meta">等待对方接受 · ${fmtDateTime(request.created_at_ms)}</div>
      </div>
      <button type="button" class="btn tiny ghost" data-act="friend-cancel" data-account="${esc(request.to)}">撤回</button>
    </div>`).join('');

  const blockedRows = S.blocked.map((entry) => `
    <div class="person">
      ${avatarHtml(person(entry.id), 'sm')}
      <div class="person-main"><div class="person-name">${esc(entry.name)}</div><div class="person-meta">已屏蔽 · 双向阻断私聊与申请</div></div>
      <button type="button" class="btn tiny" data-act="friend-unblock" data-account="${esc(entry.id)}">解除</button>
    </div>`).join('');

  return `<div class="stack">
    <div class="panel">
      <div class="panel-head"><div><h2>社交</h2><div class="muted small">好友申请、私聊、屏蔽与交易邀请</div></div>
        <div class="card-row">
          <span class="pill">${S.friends.length} 好友</span>
          <span class="pill">${S.friend_requests.length} 待处理</span>
          <span class="pill">${S.blocked.length} 已屏蔽</span>
        </div>
      </div>
      <div class="panel-body stack">
        <form data-form="friend-request" class="form-grid">
          <div class="field"><label>玩家</label>
            <select name="account">${known.map((entry) => `<option value="${esc(entry.id)}">${esc(entry.display_name)} (${esc(entry.id)})</option>`).join('') || '<option value="">暂无其他玩家</option>'}</select>
          </div>
          <div class="field"><label>申请留言</label><input name="message" maxlength="120" placeholder="一起远征？"></div>
          <button class="btn primary" type="submit">发送好友申请</button>
        </form>
        ${requests ? `<div class="hr-label">收到的申请</div><div class="stack">${requests}</div>` : ''}
        ${outgoingRows ? `<div class="hr-label">已发送</div><div class="stack">${outgoingRows}</div>` : ''}
      </div>
    </div>
    <div class="panel">
      <div class="panel-head"><div><h2>好友</h2><div class="muted small">私聊会打开右侧聊天栏；交易邀请会创建交易并发送带 metadata 的私聊消息。</div></div></div>
      <div class="panel-body"><div class="stack">${friends || '<div class="empty">还没有好友。先从下面的玩家名录添加吧。</div>'}</div></div>
    </div>
    <div class="panel">
      <div class="panel-head"><div><h2>玩家名录</h2><div class="muted small">选择玩家发送好友申请；屏蔽会同时移除好友关系与待处理申请。</div></div>
        <input id="player-search" placeholder="搜索代号 / ID" style="max-width:220px">
      </div>
      <div class="panel-body"><div class="stack" id="player-directory">${directory || '<div class="empty">暂无其他玩家</div>'}</div></div>
    </div>
    ${blockedRows ? `<div class="panel"><div class="panel-head"><h2>已屏蔽</h2></div><div class="panel-body"><div class="stack">${blockedRows}</div></div></div>` : ''}
  </div>`;
}

function tabRooms() {
  const room = S.room;
  const metaRows = room ? Object.entries(room.metadata || {}).map(([key, value]) => `
    <div class="card-row" style="justify-content:space-between;border-bottom:1px solid var(--line);padding:4px 0">
      <span><span class="dim small">${esc(key)}</span> · ${esc(value)}</span>
      ${room.is_owner ? `<button type="button" class="btn tiny ghost" data-act="room-meta-remove" data-room="${esc(room.id)}" data-key="${esc(key)}">删除</button>` : ''}
    </div>`).join('') : '';
  const players = room ? room.players.map((player) => `
    <span class="mini-person">
      ${avatarHtml(person(player.id), 'sm')}
      <span>${esc(player.name)}</span>
      ${player.is_owner ? '<span class="pill">房主</span>' : ''}
      ${room.is_owner && player.id !== S.me.id ? `<button type="button" class="btn tiny danger" data-act="room-kick" data-room="${esc(room.id)}" data-account="${esc(player.id)}">请出</button>` : ''}
    </span>`).join('') : '';
  const current = room ? `<div class="panel hot">
    <div class="panel-head">
      <div><h2>${esc(room.name)}</h2><div class="muted small">房主 ${esc(room.owner_name)} · ${room.player_count}/${room.capacity} · ${room.is_owner ? '你是房主' : '成员'}</div></div>
      <div class="card-row">
        <button type="button" class="btn small" data-act="room-open-chat" data-room="${esc(room.id)}">打开房间聊天</button>
        <button type="button" class="btn small ghost" data-act="room-leave">离开</button>
        ${room.is_owner ? `<button type="button" class="btn small danger" data-act="room-close" data-room="${esc(room.id)}">关闭房间</button>` : ''}
      </div>
    </div>
    <div class="panel-body stack">
      <div class="room-players">${players}</div>
      <div class="card">
        <h4>房间元数据</h4>
        <div style="margin-top:6px">${metaRows || '<span class="dim small">暂无元数据</span>'}</div>
        ${room.is_owner ? `<form data-form="room-metadata" style="margin-top:9px">
          <input type="hidden" name="room" value="${esc(room.id)}">
          <div class="row">
            <div class="field"><label>键</label><input name="key" placeholder="topic" required></div>
            <div class="field"><label>值</label><input name="value" placeholder="今晚交易" required></div>
            <button class="btn small" type="submit">更新</button>
          </div>
        </form>` : ''}
      </div>
    </div>
  </div>` : '<div class="panel"><div class="panel-body"><div class="empty">你还没有加入房间。可以创建自己的房间，或加入星港酒馆和 NPC 聊天。</div></div></div>';

  const roomList = S.rooms.map((entry) => `
    <div class="card">
      <div class="card-top">
        <div>
          <h3>${esc(entry.name)} ${entry.joined ? '<span class="pill open">当前</span>' : ''}</h3>
          <div class="card-sub">房主 ${esc(entry.owner_name)} · ${entry.player_count}/${entry.capacity} 人 · ${esc(entry.region)}</div>
          <div class="card-sub">${Object.entries(entry.metadata || {}).map(([key, value]) => `${esc(key)}: ${esc(value)}`).join(' · ') || '无主题'}</div>
        </div>
        ${!entry.joined ? `<button type="button" class="btn small primary" data-act="room-join" data-room="${esc(entry.id)}">加入</button>` : '<span class="pill open">已加入</span>'}
      </div>
    </div>`).join('');

  const npcCards = (S.npcs || []).map((npc) => `
    <div class="card">
      <div class="card-top">
        <div>
          <h3>${esc(npc.name)} <span class="pill">${esc(npc.channel)}</span></h3>
          <div class="card-sub">${esc(npc.greeting || npc.persona || '')}</div>
        </div>
        ${npc.enabled ? '<span class="pill open">在线</span>' : '<span class="pill locked">停用</span>'}
      </div>
      <form data-form="npc-say" style="margin-top:9px">
        <input type="hidden" name="npc" value="${esc(npc.id)}">
        <div class="row">
          <div class="field"><label>对 ${esc(npc.name)} 说</label>
            <input name="message" placeholder="试试「补给」「占卜」「任务」" ${npc.enabled ? '' : 'disabled'} required>
          </div>
          <button class="btn primary" type="submit" ${npc.enabled ? '' : 'disabled'}>发送</button>
        </div>
      </form>
      <div class="card-actions">
        <button type="button" class="btn tiny" data-act="npc-quick" data-npc="${esc(npc.id)}" data-message="补给" ${npc.enabled ? '' : 'disabled'}>领取补给</button>
        <button type="button" class="btn tiny" data-act="npc-quick" data-npc="${esc(npc.id)}" data-message="占卜" ${npc.enabled ? '' : 'disabled'}>占卜星运</button>
        <button type="button" class="btn tiny" data-act="npc-quick" data-npc="${esc(npc.id)}" data-message="任务" ${npc.enabled ? '' : 'disabled'}>问任务</button>
        <button type="button" class="btn tiny ghost" data-act="npc-prompt" data-npc="${esc(npc.id)}" ${npc.enabled ? '' : 'disabled'}>查看 Agent 提示词</button>
      </div>
      ${npcPrompt && npcPrompt.npc === npc.id ? `
        <div class="admin-json" style="margin-top:9px">${esc(npcPrompt.text)}</div>
        <div class="card-actions"><button type="button" class="btn tiny ghost" data-act="npc-prompt-close">收起提示词</button></div>` : ''}
    </div>`).join('');

  return `<div class="stack">
    ${current}
    <div class="panel">
      <div class="panel-head"><div><h2>创建房间</h2><div class="muted small">房间属于区域；房主离开时自动转移给最小 AccountId，全部离开则删除。</div></div></div>
      <div class="panel-body">
        <form data-form="room-create" class="form-grid">
          <div class="field"><label>房间名</label><input name="name" maxlength="24" placeholder="今晚的交易厅" required></div>
          <div class="field narrow"><label>容量</label><input type="number" name="capacity" min="1" max="50" value="8"></div>
          <div class="field"><label>主题（元数据）</label><input name="topic" maxlength="80" placeholder="裂隙速刷 / 交易"></div>
          <button class="btn primary" type="submit">创建并加入</button>
        </form>
      </div>
    </div>
    <div class="panel">
      <div class="panel-head">
        <div><h2>附近的 NPC</h2><div class="muted small">NPC Agent 会通过库的原子回合同时提交消息、奖励与排行榜写入。</div></div>
        <span class="pill">${(S.npcs || []).length}</span>
      </div>
      <div class="panel-body"><div class="grid two">${npcCards || '<div class="empty">当前没有可交互的 NPC</div>'}</div></div>
    </div>
    <div class="panel">
      <div class="panel-head"><div><h2>${esc(regionName(S.me.region))} 的房间</h2><div class="muted small">只能加入本区域房间</div></div><span class="pill">${S.rooms.length}</span></div>
      <div class="panel-body"><div class="stack">${roomList || '<div class="empty">本区域还没有房间</div>'}</div></div>
    </div>
  </div>`;
}

function tradeCard(trade) {
  const stateLabel = { open: '进行中', completed: '已完成', cancelled: '已取消', expired: '已过期' }[trade.state] || trade.state;
  const stateClass = trade.state === 'completed' ? 'claimed' : trade.state === 'open' ? 'available' : 'locked';
  const timing = trade.state === 'open'
    ? (trade.expired ? '已过期' : `${fmtRelative(trade.expires_at_ms)} 后过期`)
    : trade.state === 'completed' ? '资产已原子结算' : '已结束';
  const items = uniqueInventoryItems();
  const itemOptions = items.map((entry) => `<option value="${esc(entry.id)}">${esc(entry.name)} ×${num(entry.quantity)}</option>`).join('');
  const currencyOptions = ['gold', 'gems', 'astral'].map((id) => {
    const def = currencyDef(id);
    return `<option value="${id}">${esc(def.name)}</option>`;
  }).join('');
  const sideHtml = (side, title) => `<div class="side ${side.empty ? 'empty-side' : ''}">
    <h4>${title}</h4>
    ${(side.items || []).map((item) => `<div class="side-item"><span class="${rarityClass(itemDef(item.id)?.rarity)}">${esc(item.name)}</span><b>×${num(item.quantity)}</b></div>`).join('') || '<div class="dim tiny">无物品</div>'}
    ${Object.entries(side.currencies || {}).map(([currency, amount]) => `<div class="side-item"><span>${esc(currencyDef(currency).name)}</span><b>${num(amount)}</b></div>`).join('')}
  </div>`;
  const actions = trade.state === 'open' ? `
    <div class="divider"></div>
    <form data-form="trade-side">
      <input type="hidden" name="trade" value="${esc(trade.id)}">
      <div class="form-grid">
        <div class="field"><label>我的物品</label><select name="item"><option value="">不加物品</option>${itemOptions}</select></div>
        <div class="field narrow"><label>数量</label><input type="number" name="item_qty" min="1" value="1"></div>
        <div class="field"><label>我的货币</label><select name="currency"><option value="">不加货币</option>${currencyOptions}</select></div>
        <div class="field narrow"><label>金额</label><input type="number" name="currency_amount" min="1" value="1"></div>
        <button class="btn" type="submit">更新报价</button>
      </div>
    </form>
    <div class="card-actions">
      <button type="button" class="btn primary small" data-act="trade-confirm" data-trade="${esc(trade.id)}" ${trade.my_confirmed ? 'disabled' : ''}>${trade.my_confirmed ? '我已确认，等待对方' : '确认交易'}</button>
      <button type="button" class="btn danger small" data-act="trade-cancel" data-trade="${esc(trade.id)}">取消</button>
      <span class="dim tiny">${fmtRelative(trade.expires_at_ms)} 后过期 · 修改报价会重置双方确认</span>
    </div>` : '';
  return `<div class="card ${trade.state === 'completed' ? 'hot' : ''}" data-trade="${esc(trade.id)}">
    <div class="card-top">
      <div>
        <h3>交易 · ${esc(trade.partner_name)} <span class="pill ${stateClass}">${stateLabel}</span></h3>
        <div class="card-sub">${timing} · 创建于 ${fmtDateTime(trade.created_at_ms)}</div>
      </div>
      <div class="card-row">
        <span class="pill ${trade.my_confirmed ? 'claimed' : ''}">我 ${trade.my_confirmed ? '已确认' : '未确认'}</span>
        <span class="pill ${trade.their_confirmed ? 'claimed' : ''}">对方 ${trade.their_confirmed ? '已确认' : '未确认'}</span>
      </div>
    </div>
    <div style="margin-top:10px" class="trade-sides">
      ${sideHtml(trade.my_side, '我的报价')}
      ${sideHtml(trade.their_side, '对方报价')}
    </div>
    ${actions}
  </div>`;
}

function tabTrades() {
  const friends = S.friends;
  const createOptions = friends.map((friend) =>
    `<option value="${esc(friend.id)}">${esc(friend.display_name)} (${esc(friend.id)})</option>`).join('');
  return `<div class="stack">
    <div class="panel">
      <div class="panel-head"><div><h2>发起交易</h2><div class="muted small">双方挂单 → 双方确认 → 第二人确认时原子结算；任一方资产不足则回滚确认。</div></div></div>
      <div class="panel-body">
        <form data-form="trade-create" class="form-grid">
          <div class="field"><label>交易对象</label>
            <select name="account">${createOptions || '<option value="">先添加好友</option>'}</select>
          </div>
          <button class="btn primary" type="submit" ${friends.length ? '' : 'disabled'}>创建交易</button>
        </form>
        <div class="notice" style="margin-top:10px">社交页的「交易邀请」会创建交易并发送带 <span class="mono">metadata.trade</span> 的私聊消息。</div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-head"><div><h2>我的交易</h2><div class="muted small">交易有效 ${S.server.trade_expiry_secs} 秒；后台每 15 秒标记过期。</div></div><span class="pill">${S.trades.length}</span></div>
      <div class="panel-body"><div class="stack">${S.trades.map(tradeCard).join('') || '<div class="empty">还没有交易记录</div>'}</div></div>
    </div>
  </div>`;
}

function tabProfile() {
  const me = S.me;
  const owned = new Set(S.skins.owned || []);
  const equipped = S.skins.equipped || {};
  const frameOptions = S.catalogs.skins.filter((skin) => skin.category === 'frame' && owned.has(skin.id))
    .map((skin) => `<option value="${esc(skin.id)}" ${me.avatar?.frame === skin.id ? 'selected' : ''}>${esc(skin.name)}</option>`).join('');
  const badgeOptions = S.catalogs.skins.filter((skin) => skin.category === 'badge' && owned.has(skin.id))
    .map((skin) => `<option value="${esc(skin.id)}" ${me.avatar?.badge === skin.id ? 'selected' : ''}>${esc(skin.name)}</option>`).join('');

  const categories = {};
  for (const skin of S.catalogs.skins) {
    (categories[skin.category] ||= []).push(skin);
  }
  const skinSections = Object.entries(categories).map(([category, skins]) => `
    <div class="card">
      <h4>${esc(skinCategoryLabel(category))}</h4>
      <div class="stack" style="margin-top:8px">
        ${skins.map((skin) => {
          const isOwned = owned.has(skin.id);
          const isEquipped = equipped[category] === skin.id;
          return `<div class="card-row" style="justify-content:space-between">
            <div>
              <div class="${rarityClass(skin.rarity)}" style="font-size:12px">${esc(skin.name)} ${isEquipped ? '<span class="pill claimed">已装备</span>' : ''}</div>
              <div class="dim tiny">${esc(skin.description || skin.id)}</div>
            </div>
            <div class="card-row">
              ${!isOwned ? '<span class="pill locked">未拥有</span>' : isEquipped
                ? `<button type="button" class="btn tiny ghost" data-act="skin-unequip" data-category="${esc(category)}">卸下</button>`
                : `<button type="button" class="btn tiny primary" data-act="skin-equip" data-skin="${esc(skin.id)}">装备</button>`}
            </div>
          </div>`;
        }).join('')}
      </div>
    </div>`).join('');

  return `<div class="stack">
    <div class="panel">
      <div class="panel-head"><div><h2>档案与外观</h2><div class="muted small">头像框/徽章必须是已拥有且分类匹配的皮肤；库会强制校验。</div></div></div>
      <div class="panel-body">
        <div class="card-row" style="gap:16px;align-items:center">
          ${avatarHtml(me, 'lg')}
          <div>
            <h3>${esc(me.display_name)} ${me.avatar?.title ? `<span class="hud-title">「${esc(me.avatar.title)}」</span>` : ''}</h3>
            <div class="muted small">Lv.${me.level} · ${esc(regionName(me.region))} · ${esc(me.id)}</div>
            <div class="muted small">${esc(me.metadata?.bio || '还没有签名')}</div>
          </div>
        </div>
        <div class="divider"></div>
        <div class="grid two">
          <form data-form="profile" class="card">
            <h4>档案</h4>
            <div class="stack" style="margin-top:8px">
              <div class="field"><label>显示名</label><input name="display_name" maxlength="24" value="${esc(me.display_name)}" required></div>
              <div class="field"><label>签名（Account metadata.bio）</label><input name="bio" maxlength="80" value="${esc(me.metadata?.bio || '')}"></div>
              <button class="btn primary" type="submit">保存档案</button>
            </div>
          </form>
          <form data-form="avatar" class="card">
            <h4>头像与头衔</h4>
            <div class="stack" style="margin-top:8px">
              <div class="field"><label>头像（URL 或 emoji）</label><input name="image" maxlength="256" value="${esc(me.avatar?.image || '')}" placeholder="https://… 或 🌌"></div>
              <div class="field"><label>头衔</label><input name="title" maxlength="32" value="${esc(me.avatar?.title || '')}" placeholder="星港领航员"></div>
              <div class="row">
                <div class="field"><label>头像框</label><select name="frame"><option value="">无</option>${frameOptions}</select></div>
                <div class="field"><label>徽章</label><select name="badge"><option value="">无</option>${badgeOptions}</select></div>
              </div>
              <button class="btn primary" type="submit">更新头像</button>
            </div>
          </form>
        </div>
      </div>
    </div>
    <div class="panel">
      <div class="panel-head"><div><h2>外观目录</h2><div class="muted small">${S.skins.owned.length} / ${S.catalogs.skins.length} 已拥有 · 每个分类只能装备一件</div></div></div>
      <div class="panel-body"><div class="grid two">${skinSections}</div></div>
    </div>
  </div>`;
}

function tabAnnouncements() {
  const list = S.announcements;
  if (!list.length) return '<div class="empty">当前区域没有有效公告</div>';
  return `<div class="panel">
    <div class="panel-head"><div><h2>公告</h2><div class="muted small">按优先级排序；只显示当前区域与时间窗内的公告。</div></div>
      <span class="pill ${S.unread_announcements ? 'completed' : ''}">${S.unread_announcements} 未读</span>
    </div>
    <div class="panel-body"><div class="stack">${list.map((announcement) => {
      const kind = announcement.kind || 'news';
      return `<div class="ann ${kind} ${announcement.read ? '' : 'unread'}">
        <div class="ann-head">
          <span class="ann-title">${esc(announcement.title)}</span>
          <span class="pill">${esc(KIND_LABELS[kind] || kind)}</span>
          <span class="pill">优先级 ${announcement.priority}</span>
          ${announcement.read ? '<span class="pill claimed">已读</span>' : '<span class="pill completed">未读</span>'}
        </div>
        <div class="ann-body">${esc(announcement.body)}</div>
        <div class="ann-meta">
          <span class="dim tiny">${fmtDateTime(announcement.starts_at_ms)} → ${announcement.ends_at_ms ? fmtDateTime(announcement.ends_at_ms) : '长期'}</span>
          ${!announcement.read ? `<button type="button" class="btn tiny" data-act="announcement-read" data-id="${esc(announcement.id)}">标记已读</button>` : ''}
        </div>
      </div>`;
    }).join('')}</div></div>
  </div>`;
}

function tabAdmin() {
  if (!S.admin) {
    return `<div class="panel"><div class="panel-head"><h2>运营后台</h2></div><div class="panel-body">
      <div class="notice warn">需要管理员令牌。默认 <span class="mono">taixu-admin</span>，可通过环境变量 <span class="mono">TAIXU_ADMIN_TOKEN</span> 修改。</div>
      <form data-form="admin-token" class="form-grid" style="margin-top:12px">
        <div class="field"><label>管理员令牌</label><input name="token" value="${esc(adminToken)}" placeholder="taixu-admin"></div>
        <button class="btn primary" type="submit">保存并验证</button>
      </form>
    </div></div>`;
  }
  const overview = adminData.overview;
  const announcements = adminData.announcements || [];
  const npcs = adminData.npcs || [];
  const shops = S.shops;
  const regionOptions = S.regions.map((region) => `<option value="${esc(region.id)}">${esc(region.name)} (${esc(region.status)})</option>`).join('');
  const accountOptions = Object.values(S.names).filter((entry) => !entry.is_system)
    .map((entry) => `<option value="${esc(entry.id)}">${esc(entry.display_name)} (${esc(entry.id)})</option>`).join('');
  const offerOptions = shops.flatMap((shop) => (shop.offers || []).map((offer) =>
    `<option value="${esc(shop.id)}|${esc(offer.id)}">${esc(shop.name)} · ${esc(offer.name)}</option>`)).join('');
  const announcementRows = announcements.map((announcement) => `
    <div class="card-row" style="justify-content:space-between;border-bottom:1px solid var(--line);padding:5px 0">
      <div><b>${esc(announcement.title)}</b> <span class="pill">${esc(announcement.kind)}</span> ${announcement.enabled ? '<span class="pill open">启用</span>' : '<span class="pill locked">停用</span>'}</div>
      <div class="card-row">
        <button type="button" class="btn tiny" data-act="admin-announcement-toggle" data-id="${esc(announcement.id)}" data-enabled="${announcement.enabled ? '0' : '1'}">${announcement.enabled ? '停用' : '启用'}</button>
        <button type="button" class="btn tiny danger" data-act="admin-announcement-remove" data-id="${esc(announcement.id)}">删除</button>
      </div>
    </div>`).join('');
  const npcRows = npcs.map((npc) => `
    <div class="card-row" style="justify-content:space-between;border-bottom:1px solid var(--line);padding:5px 0">
      <div><b>${esc(npc.name)}</b> <span class="dim tiny">${esc(npc.id)} · ${esc(npc.channel || '')}</span> ${npc.enabled ? '<span class="pill open">启用</span>' : '<span class="pill locked">停用</span>'}</div>
      <div class="card-row">
        <button type="button" class="btn tiny" data-act="admin-npc-toggle" data-npc="${esc(npc.id)}" data-enabled="${npc.enabled ? '0' : '1'}">${npc.enabled ? '停用' : '启用'}</button>
        <button type="button" class="btn tiny danger" data-act="admin-npc-remove" data-npc="${esc(npc.id)}">移除</button>
      </div>
    </div>`).join('');
  const accountsTable = adminData.accounts ? `
    <div class="admin-json">${esc(adminData.accounts.map((account) =>
      `${account.id}  ${account.display_name}  Lv.${account.level}  ${account.region}  ${account.online ? 'online' : 'offline'}`).join('\n'))}</div>` : '';
  const taskJson = adminData.tasks ? `<div class="admin-json">${esc(JSON.stringify(adminData.tasks, null, 2))}</div>` : '';
  const snapshot = adminData.snapshot ? `<div class="admin-json">${esc(adminData.snapshot)}</div>` : '';

  return `<div class="admin-grid">
    <div class="admin-block" style="grid-column:1/-1">
      <h4>运营总览 <button type="button" class="btn tiny ghost" data-act="admin-refresh">刷新</button></h4>
      <div class="kv">
        <div class="stat"><div class="k">账号</div><div class="v">${num(overview?.accounts ?? S.server.accounts)}</div></div>
        <div class="stat"><div class="k">在线</div><div class="v">${num(overview?.online ?? S.server.online)}</div></div>
        <div class="stat"><div class="k">会话</div><div class="v">${num(overview?.sessions ?? S.server.sessions)}</div></div>
        <div class="stat"><div class="k">房间</div><div class="v">${num(overview?.rooms ?? 0)}</div></div>
        <div class="stat"><div class="k">交易</div><div class="v">${num(overview?.trades ?? 0)}</div></div>
        <div class="stat"><div class="k">聊天频道</div><div class="v">${num(overview?.chat_channels ?? 0)}</div></div>
        <div class="stat"><div class="k">快照大小</div><div class="v small">${overview ? `${(overview.snapshot_bytes / 1024).toFixed(1)} KB` : '—'}</div></div>
        <div class="stat"><div class="k">时钟</div><div class="v small">${overview?.manual_clock ? 'MANUAL' : 'SYSTEM'}</div></div>
      </div>
      <div class="card-row" style="margin-top:9px">
        <button type="button" class="btn small" data-act="admin-save">立即保存快照</button>
        <button type="button" class="btn small" data-act="admin-expire">过期交易检查</button>
        <button type="button" class="btn small" data-act="admin-load-accounts">读取账号列表</button>
        <button type="button" class="btn small" data-act="admin-load-tasks">读取任务定义</button>
        <button type="button" class="btn small" data-act="admin-snapshot">下载状态快照</button>
      </div>
      <div class="dim tiny" style="margin-top:6px">快照路径：${esc(overview?.store || S.server.store)}</div>
    </div>

    <div class="admin-block">
      <h4>发布公告</h4>
      <form data-form="admin-announcement" class="stack">
        <div class="field"><label>标题</label><input name="title" required maxlength="60"></div>
        <div class="field"><label>正文</label><textarea name="body" required></textarea></div>
        <div class="row">
          <div class="field"><label>类型</label><select name="kind">
            <option value="news">资讯</option><option value="event">活动</option>
            <option value="maintenance">维护</option><option value="promotion">促销</option><option value="system">系统</option>
          </select></div>
          <div class="field narrow"><label>优先级</label><input type="number" name="priority" value="0"></div>
        </div>
        <div class="field"><label>区域（留空 = 全区域，逗号分隔）</label><input name="regions" placeholder="aurora, nebula"></div>
        <div class="row">
          <div class="field"><label>开始（分钟后）</label><input type="number" name="starts_min" value="0"></div>
          <div class="field"><label>结束（分钟后，留空=长期）</label><input type="number" name="ends_min" placeholder="1440"></div>
        </div>
        <button class="btn primary" type="submit">发布</button>
      </form>
      <div style="margin-top:10px">${announcementRows || '<div class="dim tiny">暂无公告</div>'}</div>
    </div>

    <div class="admin-block">
      <h4>区域状态</h4>
      <form data-form="admin-region" class="stack">
        <div class="field"><label>区域</label><select name="region">${regionOptions}</select></div>
        <div class="field"><label>状态</label><select name="status">
          <option value="open">Open</option><option value="maintenance">Maintenance</option><option value="closed">Closed</option>
        </select></div>
        <button class="btn" type="submit">更新</button>
      </form>
      <div class="divider"></div>
      <h4>排行榜赛季</h4>
      <form data-form="admin-reset-board" class="row">
        <div class="field"><label>排行榜</label><select name="board">${S.leaderboards.map((board) => `<option value="${esc(board.id)}">${esc(board.name)}</option>`).join('')}</select></div>
        <button class="btn danger" type="submit">重置</button>
      </form>
      <div class="divider"></div>
      <h4>手动时钟${S.server.manual_clock ? '' : '（未启用）'}</h4>
      <form data-form="admin-clock" class="row">
        <div class="field"><label>推进毫秒</label><input type="number" name="ms" value="60000" ${S.server.manual_clock ? '' : 'disabled'}></div>
        <button class="btn" type="submit" ${S.server.manual_clock ? '' : 'disabled'}>推进</button>
      </form>
    </div>

    <div class="admin-block">
      <h4>发放资源</h4>
      <form data-form="admin-grant" class="stack">
        <div class="field"><label>账号</label><select name="account">${accountOptions}</select></div>
        <div class="row">
          <div class="field"><label>类型</label><select name="grant_type">
            <option value="currency">货币</option><option value="item">物品</option>
            <option value="resource">资源</option><option value="skin">皮肤</option><option value="xp">经验</option>
          </select></div>
          <div class="field"><label>ID</label><input name="grant_id" placeholder="gold / ore / energy / frame_gold"></div>
          <div class="field narrow"><label>数量</label><input type="number" name="grant_amount" value="1"></div>
        </div>
        <button class="btn primary" type="submit">发放</button>
      </form>
      <div class="divider"></div>
      <h4>背包容量</h4>
      <form data-form="admin-capacity" class="row">
        <div class="field"><label>账号</label><select name="account">${accountOptions}</select></div>
        <div class="field narrow"><label>格数</label><input type="number" name="capacity" value="80" min="1" max="100"></div>
        <button class="btn" type="submit">设置</button>
      </form>
    </div>

    <div class="admin-block">
      <h4>NPC</h4>
      ${npcRows || '<div class="dim tiny">暂无 NPC</div>'}
      <div class="divider"></div>
      <h4>移除商品</h4>
      <form data-form="admin-offer-remove" class="row">
        <div class="field"><label>商品</label><select name="offer">${offerOptions}</select></div>
        <button class="btn danger" type="submit">移除</button>
      </form>
    </div>

    <div class="admin-block">
      <h4>聊天治理</h4>
      <form data-form="admin-clear-chat" class="row">
        <div class="field"><label>频道</label><input name="channel" placeholder="global / region:aurora / room:room_xxx"></div>
        <button class="btn danger" type="submit">清空频道</button>
      </form>
      <div class="notice" style="margin-top:9px">单条消息删除：在右侧聊天栏的管理员模式下，悬停消息点击 🗑。</div>
      <div class="divider"></div>
      <h4>账号 / 任务定义</h4>
      ${accountsTable || '<div class="dim tiny">点击上方「读取账号列表」。</div>'}
      <div style="margin-top:8px">${taskJson || '<div class="dim tiny">点击上方「读取任务定义」。</div>'}</div>
    </div>

    ${snapshot ? `<div class="admin-block" style="grid-column:1/-1"><h4>状态快照</h4>${snapshot}</div>` : ''}
  </div>`;
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

function chatHtml() {
  const chat = S.chat || { messages: [], channel: 'global', label: '全局频道' };
  const options = [['global', '全局频道'], [`region:${S.me.region}`, `区域 · ${regionName(S.me.region)}`]];
  if (S.room) options.push([`room:${S.room.id}`, `房间 · ${S.room.name}`]);
  for (const friend of S.friends) options.push([`dm:${friend.id}`, `私聊 · ${friend.display_name}`]);
  const current = chat.channel || chatChannel;
  if (!options.some(([value]) => value === current)) options.push([current, chat.label || current]);
  const messages = (chat.messages || []).map((message) => {
    const mine = message.author?.kind === 'account' && message.author.id === S.me.id;
    const npc = message.author?.kind === 'npc';
    const system = message.author?.kind === 'system';
    const klass = system ? 'system' : npc ? 'npc' : mine ? 'mine' : '';
    const author = message.author || { name: '未知' };
    const meta = message.metadata || {};
    const metaHtml = meta.kind === 'trade_invite'
      ? `<div class="msg-meta"><span class="pill completed">交易邀请</span>${meta.trade ? `<span class="dim tiny mono">${esc(meta.trade)}</span>` : ''}</div>`
      : Object.keys(meta).length
        ? `<div class="msg-meta"><span class="pill">${esc(meta.kind || 'metadata')}</span></div>`
        : '';
    const deleteButton = S.admin
      ? `<button type="button" class="btn tiny danger msg-del" data-act="chat-delete" data-message="${esc(message.id)}" title="删除消息">🗑</button>`
      : '';
    if (system) {
      return `<div class="msg system"><div class="bubble"><div class="msg-author">星港系统 · ${fmtTime(message.sent_at_ms)}</div><div class="msg-body">${esc(message.body)}</div>${metaHtml}</div></div>`;
    }
    return `<div class="msg ${klass}">
      ${avatarHtml({ display_name: author.name, avatar: null }, 'sm')}
      <div class="bubble">
        <div class="msg-head">
          <span class="msg-author">${esc(author.name || '未知')}${npc ? ' · NPC' : ''}</span>
          <span class="msg-time">${fmtTime(message.sent_at_ms)}</span>
          ${deleteButton}
        </div>
        <div class="msg-body">${esc(message.body)}</div>
        ${metaHtml}
      </div>
    </div>`;
  }).join('');

  return `
    <div class="chat-head">
      <div class="chat-head-top">
        <h3>${esc(chat.label || '聊天')}</h3>
        <button type="button" class="btn tiny ghost" data-act="refresh">↻</button>
      </div>
      <div class="chat-channel">
        <select data-act="chat-channel">
          ${options.map(([value, label]) => `<option value="${esc(value)}" ${value === current ? 'selected' : ''}>${esc(label)}</option>`).join('')}
        </select>
      </div>
      ${chat.error ? `<div class="notice bad tiny">${esc(chat.error)}</div>` : ''}
    </div>
    <div class="chat-log" id="chat-log">
      ${messages || '<div class="empty">还没有消息。说点什么吧。</div>'}
    </div>
    <form class="chat-form" data-form="chat-send">
      <textarea id="chat-input" name="body" maxlength="500" placeholder="发送到 ${esc(chat.label || '当前频道')}… Enter 发送，Shift+Enter 换行"></textarea>
      <div class="cf-row">
        <span class="chat-note">${(chat.messages || []).length} 条最近消息 · 每 3 秒轮询</span>
        <span class="spacer"></span>
        <button class="btn primary small" type="submit">发送</button>
      </div>
    </form>`;
}

function renderChat() {
  const pane = $('#chatpane');
  if (!pane || !S) return;
  const oldInput = $('#chat-input', pane);
  const draft = oldInput ? oldInput.value : '';
  const focused = oldInput && document.activeElement === oldInput;
  const oldLog = $('#chat-log', pane);
  const nearBottom = !oldLog || (oldLog.scrollHeight - oldLog.scrollTop - oldLog.clientHeight < 90);
  pane.innerHTML = chatHtml();
  const input = $('#chat-input', pane);
  if (input) {
    input.value = draft;
    if (focused) {
      input.focus();
      input.selectionStart = input.selectionEnd = draft.length;
    }
  }
  const log = $('#chat-log', pane);
  if (log && nearBottom) log.scrollTop = log.scrollHeight;
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

async function runAction(action, element) {
  const dataset = element.dataset || {};
  try {
    switch (action) {
      case 'refresh':
        await refresh(true);
        break;
      case 'logout': {
        try { await post('/api/logout'); } catch (_) { /* already gone */ }
        token = '';
        localStorage.removeItem('taixu_token');
        S = null;
        showLogin();
        break;
      }
      case 'explore': {
        const data = await post('/api/action/explore', { kind: dataset.kind });
        lastResult = data.result;
        feedback(data);
        toast(data.result?.narrative || '行动完成', 'good');
        await refresh(true);
        break;
      }
      case 'inventory-upgrade': {
        const data = await post('/api/inventory/upgrade');
        feedback(data);
        toast(data.result?.message || '扩容成功', 'good');
        await refresh(true);
        break;
      }
      case 'task-accept': {
        await post('/api/tasks/accept', { task: dataset.task });
        toast('任务已接取', 'good');
        await refresh(true);
        break;
      }
      case 'task-abandon': {
        if (!confirm('放弃后本轮进度会清空，确定吗？')) return;
        await post('/api/tasks/abandon', { task: dataset.task });
        toast('任务已放弃', '');
        await refresh(true);
        break;
      }
      case 'task-claim': {
        const data = await post('/api/tasks/claim', { task: dataset.task });
        feedback(data);
        toast('任务奖励已领取', 'gold');
        await refresh(true);
        break;
      }
      case 'achievement-claim': {
        const data = await post('/api/achievements/claim', { id: dataset.id });
        feedback(data);
        toast('成就奖励已领取', 'gold');
        await refresh(true);
        break;
      }
      case 'collection-claim': {
        const data = await post('/api/collections/claim', {
          collection: dataset.collection, milestone: Number(dataset.milestone),
        });
        feedback(data);
        toast('图鉴里程碑奖励已领取', 'gold');
        await refresh(true);
        break;
      }
      case 'shop-buy': {
        const quantity = Number($(`#buy-qty-${dataset.shop}-${dataset.offer}`)?.value || 1);
        const data = await post('/api/shop/buy', {
          shop: dataset.shop, offer: dataset.offer, quantity: Math.max(1, quantity),
        });
        feedback(data);
        toast(`购买成功：${data.receipt?.offer} ×${data.receipt?.quantity}`, 'gold');
        await refresh(true);
        break;
      }
      case 'friend-request': {
        await post('/api/friends/request', { account: dataset.account });
        toast('好友申请已发送', 'good');
        await refresh(true);
        break;
      }
      case 'friend-accept': {
        await post('/api/friends/accept', { account: dataset.account });
        toast('已成为好友', 'good');
        await refresh(true);
        break;
      }
      case 'friend-decline': {
        await post('/api/friends/decline', { account: dataset.account });
        toast('已拒绝申请', '');
        await refresh(true);
        break;
      }
      case 'friend-cancel': {
        await post('/api/friends/cancel', { account: dataset.account });
        toast('申请已撤回', '');
        await refresh(true);
        break;
      }
      case 'friend-remove': {
        await post('/api/friends/remove', { account: dataset.account });
        toast('好友已删除', '');
        await refresh(true);
        break;
      }
      case 'friend-block': {
        if (!confirm('屏蔽会移除好友关系并双向阻断私聊与申请，确定吗？')) return;
        await post('/api/friends/block', { account: dataset.account });
        toast('已屏蔽', '');
        await refresh(true);
        break;
      }
      case 'friend-unblock': {
        await post('/api/friends/unblock', { account: dataset.account });
        toast('已解除屏蔽', 'good');
        await refresh(true);
        break;
      }
      case 'friend-dm': {
        chatChannel = `dm:${dataset.account}`;
        await refresh(true);
        break;
      }
      case 'friend-trade': {
        const data = await post('/api/chat/invite-trade', { account: dataset.account });
        toast(`交易邀请已发送 (${data.trade?.id || ''})`, 'gold');
        chatChannel = `dm:${dataset.account}`;
        await refresh(true);
        break;
      }
      case 'trade-confirm': {
        const data = await post('/api/trades/confirm', { trade: dataset.trade });
        toast(data.trade?.state === 'completed' ? '交易已完成，资产已结算' : '已确认，等待对方', 'gold');
        await refresh(true);
        break;
      }
      case 'trade-cancel': {
        await post('/api/trades/cancel', { trade: dataset.trade });
        toast('交易已取消', '');
        await refresh(true);
        break;
      }
      case 'room-join': {
        await post('/api/rooms/join', { room: dataset.room });
        chatChannel = `room:${dataset.room}`;
        toast('已加入房间', 'good');
        await refresh(true);
        break;
      }
      case 'room-leave': {
        await post('/api/rooms/leave');
        chatChannel = `region:${S.me.region}`;
        toast('已离开房间', '');
        await refresh(true);
        break;
      }
      case 'room-close': {
        if (!confirm('关闭房间会移除所有成员，确定吗？')) return;
        await post('/api/rooms/close', { room: dataset.room });
        chatChannel = 'region';
        toast('房间已关闭', '');
        await refresh(true);
        break;
      }
      case 'room-kick': {
        if (!confirm('将该玩家请出房间？')) return;
        await post('/api/rooms/kick', { room: dataset.room, account: dataset.account });
        toast('已请出玩家', '');
        await refresh(true);
        break;
      }
      case 'room-meta-remove': {
        await post('/api/rooms/metadata/remove', { room: dataset.room, key: dataset.key });
        toast('元数据已删除', '');
        await refresh(true);
        break;
      }
      case 'room-open-chat': {
        chatChannel = `room:${dataset.room}`;
        await refresh(true);
        break;
      }
      case 'travel': {
        const data = await post('/api/travel', { region: dataset.region });
        toast(`已迁移到 ${regionName(data.account.region)}`, 'good');
        chatChannel = `region:${data.account.region}`;
        await refresh(true);
        break;
      }
      case 'skin-equip': {
        await post('/api/skins/equip', { skin: dataset.skin });
        toast('已装备', 'good');
        await refresh(true);
        break;
      }
      case 'skin-unequip': {
        await post('/api/skins/unequip', { category: dataset.category });
        toast('已卸下', '');
        await refresh(true);
        break;
      }
      case 'announcement-read': {
        await post('/api/announcements/read', { id: dataset.id });
        await refresh(true);
        break;
      }
      case 'npc-quick': {
        const data = await post('/api/npc/say', {
          npc: dataset.npc, message: dataset.message || '你好',
        });
        feedback(data);
        if (data.turn?.channel) chatChannel = data.turn.channel;
        toast(`NPC 回应了 ${data.turn?.npc_messages?.length || 0} 条消息`, 'good');
        await refresh(true);
        break;
      }
      case 'npc-prompt': {
        const data = await post('/api/npc/prompt', {
          npc: dataset.npc, message: '你好',
        });
        npcPrompt = { npc: dataset.npc, text: data.prompt };
        renderTab();
        break;
      }
      case 'npc-prompt-close': {
        npcPrompt = null;
        renderTab();
        break;
      }
      case 'chat-delete': {
        const channel = adminChannelFromCurrent();
        await api('/api/admin/chat/delete', {
          method: 'POST',
          body: { channel, message: dataset.message },
        });
        toast('消息已删除', '');
        await refresh(true);
        break;
      }
      case 'admin-reset-board': {
        if (!confirm('重置赛季会清空该排行榜全部分数，确定吗？')) return;
        const data = await api('/api/admin/leaderboard/reset', {
          method: 'POST', body: { leaderboard: dataset.board },
        });
        toast(`新赛季 ${data.season}`, 'gold');
        await refresh(true);
        break;
      }
      case 'admin-refresh': {
        adminData = { overview: null, announcements: null, npcs: null, accounts: null, tasks: null, snapshot: null };
        await loadAdminData('core');
        break;
      }
      case 'admin-load-accounts':
        await loadAdminData('accounts');
        break;
      case 'admin-load-tasks':
        await loadAdminData('tasks');
        break;
      case 'admin-save': {
        await api('/api/admin/save', { method: 'POST', body: {} });
        toast('快照已保存', 'good');
        break;
      }
      case 'admin-expire': {
        const data = await api('/api/admin/trade/expire', { method: 'POST', body: {} });
        toast(`已过期 ${data.expired.length} 笔交易`, '');
        await refresh(true);
        break;
      }
      case 'admin-snapshot': {
        const data = await api('/api/admin/snapshot');
        adminData.snapshot = JSON.stringify(data, null, 2);
        toast('快照已读取', 'good');
        renderTab();
        break;
      }
      case 'admin-announcement-toggle': {
        await api('/api/admin/announcement/enabled', {
          method: 'POST',
          body: { id: dataset.id, enabled: dataset.enabled === '1' },
        });
        adminData.announcements = null;
        await loadAdminData('core');
        await refresh(true);
        break;
      }
      case 'admin-announcement-remove': {
        if (!confirm('删除公告？')) return;
        await api('/api/admin/announcement/remove', { method: 'POST', body: { id: dataset.id } });
        adminData.announcements = null;
        await loadAdminData('core');
        await refresh(true);
        break;
      }
      case 'admin-npc-toggle': {
        await api('/api/admin/npc/enabled', {
          method: 'POST',
          body: { npc: dataset.npc, enabled: dataset.enabled === '1' },
        });
        adminData.npcs = null;
        await loadAdminData('core');
        await refresh(true);
        break;
      }
      case 'admin-npc-remove': {
        if (!confirm('移除 NPC？')) return;
        await api('/api/admin/npc/remove', { method: 'POST', body: { id: dataset.npc } });
        adminData.npcs = null;
        await loadAdminData('core');
        await refresh(true);
        break;
      }
      case 'open-tab':
        activeTab = dataset.tab;
        renderNav();
        renderTab();
        break;
      default:
        console.warn('unknown action', action);
    }
  } catch (error) {
    if (error.status === 401) {
      showLogin();
    } else {
      toast(error.message, 'bad');
    }
  }
}

function adminChannelFromCurrent() {
  const channel = S?.chat?.channel || chatChannel;
  if (channel === 'global') return 'global';
  if (channel === 'region') return `region:${S.me.region}`;
  if (channel.startsWith('room:')) return channel;
  if (channel.startsWith('dm:')) {
    const other = channel.slice(3);
    return `dm:${S.me.id}|${other}`;
  }
  return channel;
}

async function handleForm(name, form) {
  const fd = new FormData(form);
  const get = (key) => String(fd.get(key) || '').trim();
  try {
    switch (name) {
      case 'login':
        await handleLogin(form);
        return;

      case 'chat-send': {
        const body = get('body');
        if (!body) return;
        const data = await post('/api/chat/send', { channel: chatChannel, body });
        feedback(data);
        form.reset();
        await refresh(true);
        break;
      }
      case 'npc-say': {
        const message = get('message');
        if (!message) return;
        const data = await post('/api/npc/say', { npc: get('npc'), message });
        feedback(data);
        if (data.turn?.channel) chatChannel = data.turn.channel;
        form.reset();
        toast(`NPC 回应了 ${data.turn?.npc_messages?.length || 0} 条消息`, 'good');
        await refresh(true);
        break;
      }
      case 'craft': {
        const data = await post('/api/inventory/craft', { recipe: get('recipe') });
        feedback(data);
        toast(data.result?.message || '合成成功', 'good');
        await refresh(true);
        break;
      }
      case 'sell': {
        const data = await post('/api/inventory/sell', {
          item: get('item'), quantity: Number(get('quantity') || 0),
        });
        feedback(data);
        toast(data.result?.message || '出售成功', 'gold');
        await refresh(true);
        break;
      }
      case 'room-create': {
        await post('/api/rooms/create', {
          name: get('name'),
          capacity: Number(get('capacity') || 8),
          topic: get('topic') || null,
        });
        toast('房间已创建', 'good');
        await refresh(true);
        break;
      }
      case 'room-metadata': {
        await post('/api/rooms/metadata', {
          room: get('room'), key: get('key'), value: get('value'),
        });
        toast('元数据已更新', 'good');
        await refresh(true);
        break;
      }
      case 'friend-request': {
        await post('/api/friends/request', {
          account: get('account'), message: get('message') || null,
        });
        toast('好友申请已发送', 'good');
        await refresh(true);
        break;
      }
      case 'trade-create': {
        const data = await post('/api/trades/create', { account: get('account') });
        toast(`交易已创建 ${data.trade?.id || ''}`, 'good');
        await refresh(true);
        break;
      }
      case 'trade-side': {
        const items = {};
        const item = get('item');
        const itemQty = Number(get('item_qty') || 0);
        if (item && itemQty > 0) items[item] = itemQty;
        const currencies = {};
        const currency = get('currency');
        const amount = Number(get('currency_amount') || 0);
        if (currency && amount > 0) currencies[currency] = amount;
        await post('/api/trades/side', { trade: get('trade'), items, currencies });
        toast('报价已更新，双方确认已重置', 'good');
        await refresh(true);
        break;
      }
      case 'profile': {
        const displayName = get('display_name');
        const bio = get('bio');
        await post('/api/profile', {
          display_name: displayName || null,
          metadata: { bio },
        });
        toast('档案已保存', 'good');
        await refresh(true);
        break;
      }
      case 'avatar': {
        const value = (key) => {
          const raw = get(key);
          return raw ? raw : null;
        };
        await post('/api/profile/avatar', {
          image: value('image'),
          title: value('title'),
          frame: value('frame'),
          badge: value('badge'),
        });
        toast('头像已更新', 'good');
        await refresh(true);
        break;
      }
      case 'admin-token': {
        adminToken = get('token');
        localStorage.setItem('taixu_admin', adminToken);
        toast('管理员令牌已保存', 'good');
        adminData = { overview: null, announcements: null, npcs: null, accounts: null, tasks: null, snapshot: null };
        await refresh(true);
        break;
      }
      case 'admin-announcement': {
        const regions = get('regions')
          ? get('regions').split(/[,，\s]+/).filter(Boolean)
          : null;
        const startsMin = get('starts_min');
        const endsMin = get('ends_min');
        const body = {
          title: get('title'),
          body: get('body'),
          kind: get('kind'),
          priority: Number(get('priority') || 0),
          regions,
          starts_in_ms: startsMin === '' ? null : Number(startsMin) * 60000,
          ends_in_ms: endsMin === '' ? null : Number(endsMin) * 60000,
        };
        await api('/api/admin/announcement', { method: 'POST', body });
        adminData.announcements = null;
        toast('公告已发布', 'good');
        form.reset();
        await loadAdminData('core');
        await refresh(true);
        break;
      }
      case 'admin-region': {
        await api('/api/admin/region/status', {
          method: 'POST',
          body: { region: get('region'), status: get('status') },
        });
        toast('区域状态已更新', 'good');
        await refresh(true);
        break;
      }
      case 'admin-reset-board': {
        const data = await api('/api/admin/leaderboard/reset', {
          method: 'POST', body: { leaderboard: get('board') },
        });
        toast(`新赛季 ${data.season}`, 'gold');
        await refresh(true);
        break;
      }
      case 'admin-clock': {
        const data = await api('/api/admin/clock/advance', {
          method: 'POST', body: { ms: Number(get('ms') || 0) },
        });
        toast(`世界时间推进到 ${fmtDateTime(data.now_ms)}`, 'gold');
        await refresh(true);
        break;
      }
      case 'admin-grant': {
        const account = get('account');
        const type = get('grant_type');
        const id = get('grant_id');
        const amount = Number(get('grant_amount') || 1);
        if (!account) return toast('请选择账号', 'bad');
        const payload = { account };
        if (type === 'xp') {
          payload.xp = amount;
        } else if (type === 'skin') {
          if (!id) return toast('请输入皮肤 ID', 'bad');
          payload.skin = id;
        } else if (type === 'currency') {
          if (!id) return toast('请输入货币 ID', 'bad');
          payload.currency = id;
          payload.currency_amount = amount;
        } else if (type === 'item') {
          if (!id) return toast('请输入物品 ID', 'bad');
          payload.item = id;
          payload.item_quantity = amount;
        } else if (type === 'resource') {
          if (!id) return toast('请输入资源 ID', 'bad');
          payload.resource = id;
          payload.resource_amount = amount;
        }
        const data = await api('/api/admin/grant', { method: 'POST', body: payload });
        feedback(data);
        toast('发放成功', 'gold');
        await refresh(true);
        break;
      }
      case 'admin-capacity': {
        await api('/api/admin/capacity', {
          method: 'POST',
          body: { account: get('account'), capacity: Number(get('capacity') || 40) },
        });
        toast('背包容量已更新', 'good');
        await refresh(true);
        break;
      }
      case 'admin-offer-remove': {
        const [shop, offer] = get('offer').split('|');
        if (!shop || !offer) return toast('请选择商品', 'bad');
        if (!confirm('移除该商品？')) return;
        await api('/api/admin/offer/remove', { method: 'POST', body: { shop, offer } });
        toast('商品已移除', '');
        await refresh(true);
        break;
      }
      case 'admin-clear-chat': {
        if (!confirm('清空该频道全部聊天记录？')) return;
        const data = await api('/api/admin/chat/clear', {
          method: 'POST', body: { channel: get('channel') },
        });
        toast(data.cleared ? '频道已清空' : '频道不存在或为空', '');
        await refresh(true);
        break;
      }
      default:
        console.warn('unknown form', name);
    }
  } catch (error) {
    if (error.status === 401) {
      showLogin();
    } else {
      toast(error.message, 'bad');
    }
  }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

document.addEventListener('click', async (event) => {
  const regionButton = event.target.closest('.login-region[data-region]');
  if (regionButton && !regionButton.disabled) {
    loginRegion = regionButton.dataset.region;
    $$('.login-region').forEach((node) => node.classList.toggle('selected', node.dataset.region === loginRegion));
    const hidden = $('#login-region');
    if (hidden) hidden.value = loginRegion;
    return;
  }

  const tabButton = event.target.closest('[data-tab]');
  if (tabButton) {
    activeTab = tabButton.dataset.tab;
    renderNav();
    renderTab();
    maybeLoadAdmin();
    return;
  }

  const actionButton = event.target.closest('[data-act]');
  if (actionButton) {
    if (actionButton.tagName === 'SELECT') return; // handled by change
    event.preventDefault();
    await runAction(actionButton.dataset.act, actionButton);
  }
});

document.addEventListener('change', async (event) => {
  const target = event.target;
  if (target.dataset?.act === 'chat-channel') {
    chatChannel = target.value;
    await refresh(true);
  }
});

document.addEventListener('input', (event) => {
  if (event.target.id === 'player-search') {
    const query = event.target.value.trim().toLowerCase();
    $$('#player-directory .person').forEach((node) => {
      node.style.display = !query || (node.dataset.search || '').includes(query) ? '' : 'none';
    });
  }
});

document.addEventListener('submit', async (event) => {
  const form = event.target.closest('[data-form]');
  if (!form) return;
  event.preventDefault();
  await handleForm(form.dataset.form, form);
});

document.addEventListener('keydown', (event) => {
  if (event.target.id === 'chat-input' && event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    const form = event.target.closest('form');
    if (form) form.requestSubmit();
  }
});

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

if (token) {
  refresh(true);
} else {
  showLogin();
}

setInterval(() => {
  if (!document.hidden && token) refresh(false);
}, 3000);

setInterval(() => {
  if (!document.hidden && token) {
    api('/api/heartbeat', { method: 'POST' }).catch(() => {});
  }
}, 30000);

document.addEventListener('visibilitychange', () => {
  if (!document.hidden && token) refresh(true);
});
