import './js/contracts.js?v=20260414a';
import { createDraft, discoverDraft, loadBootstrap as fetchBootstrap, loadPublicBootstrap, patchDraft, publishDraft, restoreDraftByShareToken, runValidation } from './js/api.js?v=20260414c';
import { loadRememberedSelection, loadSettings, rememberSelection, saveSettings, setToken, getToken } from './js/settings.js?v=20260414b';
import { applyTheme } from './js/theme.js?v=20260414b';
import { fillDraftForm, highlightPage, injectStaticIcons, populateResourceSelects, renderChrome, renderOnboarding, renderOverview, renderSettings, renderStatusNotice } from './js/renderers.js?v=20260414c';

const rawBootstrap = window.__DASHBOARD_BOOTSTRAP__ || {};
const autoPreview = String(window.location?.pathname || '').includes('/dashboard-preview');
const serverBootstrap = {
  ...rawBootstrap,
  readonly: rawBootstrap.readonly ?? autoPreview,
  publicPreview: rawBootstrap.publicPreview ?? autoPreview,
  bootstrapPath: rawBootstrap.bootstrapPath || '/public/dashboard/bootstrap',
};
const state = {
  bootstrap: null,
  currentDraft: null,
  selectedPage: 'overview',
  settings: loadSettings(),
  lastRefreshAt: null,
  refreshTimer: null,
  refreshRequestId: 0,
  refreshAbortController: null,
  lastError: null,
  overviewTaskTab: 'all',
  overviewTaskInsightMode: 'failure',
  overviewStatusFilter: 'all',
  overviewSiteFilter: 'all',
  selectedOverviewTaskId: null,
  selectedEvidenceStage: 'active_success',
  onboardingReadiness: {},
  readonly: Boolean(serverBootstrap.readonly),
  publicPreview: Boolean(serverBootstrap.publicPreview),
  bootstrapPath: serverBootstrap.bootstrapPath,
};
const $ = (id) => document.getElementById(id);
const cloneJson = (value) => JSON.parse(JSON.stringify(value || {}));
const on = (id, eventName, handler) => {
  const element = $(id);
  if (element) element.addEventListener(eventName, handler);
};

function showToast(message, isError = false) {
  const toast = $('toast');
  if (!toast) return;
  toast.textContent = message;
  toast.dataset.tone = isError ? 'danger' : 'info';
  toast.classList.add('show');
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => toast.classList.remove('show'), 2600);
}

function normalizeErrorMessage(error) {
  const raw = String(error?.message || '操作失败').trim();
  if (raw.includes('auth_failed') || raw.includes('invalid admin token')) return '管理令牌无效，请重新保存。';
  if (raw.includes('share_token_expired')) return '当前草稿链接已过期，请重新生成。';
  if (raw.includes('Failed to fetch')) return '网络请求失败，请检查 dashboard 服务是否在线。';
  return raw;
}

function handleError(error) {
  console.error(error);
  state.lastError = { title: '操作失败', message: normalizeErrorMessage(error), isError: true };
  renderStatusNotice(state);
  showToast(normalizeErrorMessage(error), true);
}

function parseArrayInput(value) {
  return String(value || '').split(',').map((item) => item.trim()).filter(Boolean);
}

function buildContractFromInputs() {
  const base = cloneJson(state.currentDraft?.final_contract_json || state.currentDraft?.inferred_contract_json || {});
  const fieldRoles = { ...(base.field_roles || {}) };
  const selectors = {
    username: $('usernameSelector')?.value.trim(),
    password: $('passwordSelector')?.value.trim(),
    remember_me: $('rememberSelector')?.value.trim(),
    submit: $('submitSelector')?.value.trim(),
  };
  Object.entries(selectors).forEach(([key, selector]) => {
    if (selector) fieldRoles[key] = { selector };
    else delete fieldRoles[key];
  });
  const contract = {
    mode: 'auth',
    primary_form_selector: $('primaryFormSelector')?.value.trim() || null,
    field_roles: fieldRoles,
    success: {
      ready_selector: $('readySelector')?.value.trim() || null,
      url_patterns: base.success?.url_patterns || [],
      title_contains: base.success?.title_contains || [],
    },
    error_signals: {
      login_error: parseArrayInput($('loginErrorSignals')?.value),
      field_error: parseArrayInput($('fieldErrorSignals')?.value),
      account_locked: parseArrayInput($('accountLockedSignals')?.value),
    },
  };
  const hasAny = contract.primary_form_selector || Object.keys(fieldRoles).length || contract.success.ready_selector || contract.error_signals.login_error.length || contract.error_signals.field_error.length || contract.error_signals.account_locked.length;
  return hasAny ? contract : null;
}

function rememberSelections() {
  ['behaviorProfile', 'identityProfile', 'sessionProfile', 'fingerprintProfile', 'proxyId'].forEach((key) => rememberSelection(key, $(key)?.value || ''));
}

function extractDraftPayload() {
  rememberSelections();
  return {
    login_url: $('loginUrl')?.value.trim(),
    success_hint: $('successHint')?.value.trim() || null,
    behavior_profile_id: $('behaviorProfile')?.value || null,
    identity_profile_id: $('identityProfile')?.value || null,
    session_profile_id: $('sessionProfile')?.value || null,
    fingerprint_profile_id: $('fingerprintProfile')?.value || null,
    proxy_id: $('proxyId')?.value || null,
    credential_mode: $('credentialMode')?.value || 'alias',
    credential_ref: $('credentialRef')?.value.trim() || null,
    final_contract_json: buildContractFromInputs(),
  };
}

function currentInlineCredentials() {
  if ($('credentialMode')?.value !== 'inline_once') return null;
  const username = $('inlineUsername')?.value.trim();
  const password = $('inlinePassword')?.value.trim();
  if (!username && !password) return null;
  return { username, password };
}

function updateSettings(patch) {
  state.settings = { ...state.settings, ...patch };
  saveSettings(state.settings);
  applyTheme(state.settings);
  resetAutoRefresh();
}

async function hydrateAutoToken() {
  if (!getToken() && serverBootstrap.adminToken) {
    setToken(serverBootstrap.adminToken);
    return;
  }
  try {
    const response = await fetch('/dashboard-session');
    if (!response.ok) return;
    const payload = await response.json();
    if (payload?.public_preview || payload?.readonly) {
      state.publicPreview = true;
      state.readonly = true;
      if (payload?.bootstrap_path) state.bootstrapPath = payload.bootstrap_path;
      return;
    }
    if (payload?.admin_token) setToken(payload.admin_token);
  } catch {
    // 保持静默，允许手动输入。
  }
}

function ensureAdminToken() {
  if (state.publicPreview || state.readonly) {
    state.lastError = { title: '当前是只读预览', message: '这个页面只展示监控数据，不提供发现、发布或验证操作。', isError: false };
    renderStatusNotice(state);
    showToast('只读预览不支持写操作', true);
    return false;
  }
  if (getToken()) return true;
  state.lastError = { title: '请先保存管理令牌', message: '内部环境通常会自动接通；如果没有，请在左侧输入令牌后再继续。', isError: false };
  renderStatusNotice(state);
  showToast('请先保存管理令牌', true);
  return false;
}

function preferredEvidenceStage(evidenceSummary) {
  return ['active_success', 'active_failure', 'retry_observation', 'continuity', 'shadow'].find((key) => evidenceSummary?.[key]) || 'active_success';
}

function selectDraftById(draftId) {
  const draft = (state.bootstrap?.drafts || []).find((item) => item.id === draftId) || null;
  state.currentDraft = draft;
  state.selectedEvidenceStage = preferredEvidenceStage(draft?.evidence_summary_json);
  fillDraftForm(draft);
  state.onboardingReadiness = evaluateOnboardingReadiness();
  renderOnboarding(state);
}

async function resolveDraftSelection(preferredDraftId = null) {
  const drafts = state.bootstrap?.drafts || [];
  if (state.publicPreview || state.readonly) {
    state.currentDraft = null;
    state.selectedEvidenceStage = 'active_success';
    fillDraftForm(null);
    return;
  }
  const shareToken = new URLSearchParams(window.location.search).get('draft');
  let draft = preferredDraftId ? drafts.find((item) => item.id === preferredDraftId) : null;
  if (!draft && shareToken) {
    draft = drafts.find((item) => item.share_token === shareToken) || null;
    if (!draft) {
      try {
        const rows = await restoreDraftByShareToken(shareToken);
        draft = rows?.[0] || null;
      } catch (error) {
        showToast(`草稿链接恢复失败：${normalizeErrorMessage(error)}`, true);
      }
    }
  }
  if (!draft && state.currentDraft) draft = drafts.find((item) => item.id === state.currentDraft.id) || null;
  if (!draft && drafts.length) draft = drafts[0];
  state.currentDraft = draft || null;
  state.selectedEvidenceStage = preferredEvidenceStage(draft?.evidence_summary_json);
  fillDraftForm(state.currentDraft);
}

async function renderApp() {
  applyTheme(state.settings);
  if (state.readonly && state.selectedPage === 'onboarding') state.selectedPage = 'overview';
  if ($('adminToken')) $('adminToken').value = getToken();
  populateResourceSelects(state.bootstrap, state.currentDraft, loadRememberedSelection);
  state.onboardingReadiness = evaluateOnboardingReadiness();
  renderChrome(state);
  renderStatusNotice(state);
  await renderOverview(state);
  renderOnboarding(state);
  renderSettings(state);
  highlightPage(state.selectedPage);
  syncCredentialInputs();
}
async function refreshBootstrap({ preferredDraftId = null, silent = false } = {}) {
  if (!state.publicPreview && !state.readonly && !getToken()) {
    state.lastError = { title: '等待管理会话', message: '当前浏览器还没有管理令牌，页面会自动尝试获取。', isError: false };
    await renderApp();
    return;
  }
  if (state.refreshAbortController) state.refreshAbortController.abort();
  const controller = new AbortController();
  state.refreshAbortController = controller;
  const requestId = ++state.refreshRequestId;
  try {
    const bootstrap = state.publicPreview || state.readonly
      ? await loadPublicBootstrap(controller.signal, state.bootstrapPath)
      : await fetchBootstrap(controller.signal);
    if (requestId !== state.refreshRequestId) return;
    state.bootstrap = bootstrap;
    state.lastRefreshAt = new Date().toISOString();
    state.lastError = bootstrap.status_error ? { title: '控制面状态读取失败', message: bootstrap.status_error, isError: true } : null;
    await resolveDraftSelection(preferredDraftId);
    await renderApp();
    if (!silent) showToast('控制台已刷新');
  } catch (error) {
    if (error.name === 'AbortError') return;
    state.lastError = { title: '数据刷新失败', message: normalizeErrorMessage(error), isError: true };
    await renderApp();
    showToast(normalizeErrorMessage(error), true);
  }
}

function resetAutoRefresh() {
  clearInterval(state.refreshTimer);
  state.refreshTimer = window.setInterval(() => refreshBootstrap({ silent: true }).catch(handleError), Number(state.settings.refreshInterval) * 1000);
}

async function createDraftAction() {
  if (!ensureAdminToken()) return;
  const payload = extractDraftPayload();
  if (!payload.login_url) return showToast('请先填写登录页地址', true);
  const draft = await createDraft(payload);
  showToast('草稿已创建');
  await refreshBootstrap({ preferredDraftId: draft.id, silent: true });
}

async function saveDraftAction({ silent = false } = {}) {
  if (!ensureAdminToken()) return null;
  if (!state.currentDraft?.id) {
    showToast('请先选择或创建草稿', true);
    return null;
  }
  const draft = await patchDraft(state.currentDraft.id, extractDraftPayload());
  if (!silent) showToast('草稿已保存');
  await refreshBootstrap({ preferredDraftId: draft.id, silent: true });
  return draft;
}

async function discoverDraftAction() {
  if (!ensureAdminToken()) return;
  if (!state.currentDraft?.id) return showToast('请先选择草稿', true);
  await saveDraftAction({ silent: true });
  const draft = await discoverDraft(state.currentDraft.id);
  showToast('自动发现完成');
  await refreshBootstrap({ preferredDraftId: draft.id, silent: true });
}

async function publishDraftAction() {
  if (!ensureAdminToken()) return;
  if (!state.currentDraft?.id) return showToast('请先选择草稿', true);
  await saveDraftAction({ silent: true });
  const draft = await publishDraft(state.currentDraft.id);
  showToast('站点策略已发布');
  await refreshBootstrap({ preferredDraftId: draft.id, silent: true });
}

async function validateDraftAction(scenario = 'default') {
  if (!ensureAdminToken()) return;
  if (!state.currentDraft?.id) return showToast('请先选择草稿', true);
  await saveDraftAction({ silent: true });
  if (scenario === 'default') await publishDraft(state.currentDraft.id);
  const draft = await runValidation(state.currentDraft.id, { scenario, inline_credentials: currentInlineCredentials() });
  showToast(`验证完成：${scenario}`);
  await refreshBootstrap({ preferredDraftId: draft.id, silent: true });
}

async function copyShareLinkAction() {
  if (!state.currentDraft?.share_url) return showToast('当前草稿没有可复制的分享链接', true);
  const url = `${window.location.origin}${state.currentDraft.share_url}`;
  await navigator.clipboard.writeText(url);
  showToast('草稿链接已复制');
}

function evaluateOnboardingReadiness() {
  if (state.publicPreview || state.readonly) {
    return {
      createEnabled: false,
      createReason: '只读预览不支持创建草稿',
      saveEnabled: false,
      saveReason: '只读预览不支持保存草稿',
      discoverEnabled: false,
      discoverReason: '只读预览不支持自动发现',
      publishEnabled: false,
      publishReason: '只读预览不支持发布站点策略',
      validateEnabled: false,
      validateReason: '只读预览不支持运行验证',
      sampleEnabled: false,
      sampleReason: '只读预览不支持样本验证',
      hint: '当前页面是只读预览，只用于查看监控数据。',
      reasons: ['只读预览不提供站点接入、发布或验证操作。'],
    };
  }
  const tokenReady = Boolean(getToken());
  const payload = extractDraftPayload();
  const hasDraft = Boolean(state.currentDraft?.id);
  const contract = payload.final_contract_json || {};
  const missing = [
    !contract?.field_roles?.password?.selector && '缺少 password selector',
    !contract?.field_roles?.submit?.selector && '缺少 submit selector',
    !contract?.success?.ready_selector && '缺少 success.ready_selector',
  ].filter(Boolean);
  const reasons = [];
  if (!tokenReady) reasons.push('当前还没有管理令牌。内部环境一般会自动接通，如果没有请手动保存。');
  if (!payload.login_url) reasons.push('还没有填写登录页地址。');
  if (!hasDraft) reasons.push('还没有草稿，请先创建草稿。');
  if (missing.length) reasons.push(...missing);
  return {
    createEnabled: tokenReady && Boolean(payload.login_url),
    createReason: tokenReady ? '请先填写登录页地址' : '请先保存管理令牌',
    saveEnabled: tokenReady && hasDraft,
    saveReason: '请先选择或创建草稿',
    discoverEnabled: tokenReady && hasDraft,
    discoverReason: '请先选择或创建草稿',
    publishEnabled: tokenReady && hasDraft && missing.length === 0,
    publishReason: missing.length ? missing.join('；') : '请先选择草稿',
    validateEnabled: tokenReady && hasDraft && missing.length === 0,
    validateReason: missing.length ? `无法验证：${missing.join('；')}` : '请先选择草稿',
    sampleEnabled: tokenReady && hasDraft,
    sampleReason: '请先选择草稿',
    hint: !tokenReady ? '请先连接管理会话。' : !payload.login_url ? '先填写登录页地址。' : missing.length ? `发布前仍需补齐：${missing.join('、')}` : '当前前置条件齐备，可以继续发布并验证。',
    reasons,
  };
}

function syncCredentialInputs() {
  $('inlineCredentialPanel')?.classList.toggle('hidden', $('credentialMode')?.value !== 'inline_once');
}

function wireEvents() {
  on('saveTokenBtn', 'click', async () => {
    setToken($('adminToken').value);
    state.lastError = null;
    showToast('管理令牌已保存');
    await refreshBootstrap({ silent: true });
  });
  on('manualRefreshBtn', 'click', () => refreshBootstrap().catch(handleError));
  on('createDraftBtn', 'click', () => createDraftAction().catch(handleError));
  on('saveDraftBtn', 'click', () => saveDraftAction().catch(handleError));
  on('discoverBtn', 'click', () => discoverDraftAction().catch(handleError));
  on('publishBtn', 'click', () => publishDraftAction().catch(handleError));
  on('validateBtn', 'click', () => validateDraftAction('default').catch(handleError));
  on('failureBtn', 'click', () => validateDraftAction('business_failure').catch(handleError));
  on('retryBtn', 'click', () => validateDraftAction('retry_observation').catch(handleError));
  on('copyLinkBtn', 'click', () => copyShareLinkAction().catch(handleError));
  document.addEventListener('click', (event) => {
    const navButton = event.target.closest('.nav-item[data-page]');
    if (navButton) { state.selectedPage = navButton.dataset.page; highlightPage(state.selectedPage); return; }
    const draftButton = event.target.closest('[data-draft-id]');
    if (draftButton) { selectDraftById(draftButton.dataset.draftId); return; }
    const taskTab = event.target.closest('[data-task-tab]');
    if (taskTab) { state.overviewTaskTab = taskTab.dataset.taskTab; renderOverview(state).catch(handleError); return; }
    const taskInsight = event.target.closest('[data-task-insight]');
    if (taskInsight) { state.overviewTaskInsightMode = taskInsight.dataset.taskInsight; renderOverview(state).catch(handleError); return; }
    const taskSelect = event.target.closest('[data-task-select]');
    if (taskSelect) { state.selectedOverviewTaskId = taskSelect.dataset.taskSelect; renderOverview(state).catch(handleError); return; }
    const evidenceTab = event.target.closest('[data-evidence-stage]');
    if (evidenceTab) { state.selectedEvidenceStage = evidenceTab.dataset.evidenceStage; renderOnboarding(state); return; }
    const wallpaperButton = event.target.closest('[data-wallpaper-select]');
    if (wallpaperButton) { updateSettings({ wallpaper: wallpaperButton.dataset.wallpaperSelect }); renderSettings(state); return; }
  });
  on('overviewRefreshInterval', 'change', async () => { updateSettings({ refreshInterval: Number($('overviewRefreshInterval').value) }); await renderApp(); });
  on('overviewStatusFilter', 'change', () => { state.overviewStatusFilter = $('overviewStatusFilter').value; renderOverview(state).catch(handleError); });
  on('overviewSiteFilter', 'change', () => { state.overviewSiteFilter = $('overviewSiteFilter').value; renderOverview(state).catch(handleError); });
  ['settingThemeMode', 'settingAccentMode', 'settingWallpaper', 'settingRefreshInterval', 'settingSummaryMode', 'settingDensity', 'settingMotion'].forEach((id) => on(id, 'change', async () => { updateSettings({ themeMode: $('settingThemeMode').value, accentMode: $('settingAccentMode').value, wallpaper: $('settingWallpaper').value, refreshInterval: Number($('settingRefreshInterval').value), summaryMode: $('settingSummaryMode').value, density: $('settingDensity').value, motion: $('settingMotion').value }); await renderApp(); }));
  ['loginUrl', 'successHint', 'behaviorProfile', 'identityProfile', 'sessionProfile', 'fingerprintProfile', 'proxyId', 'credentialMode', 'credentialRef', 'primaryFormSelector', 'usernameSelector', 'passwordSelector', 'rememberSelector', 'submitSelector', 'readySelector', 'loginErrorSignals', 'fieldErrorSignals', 'accountLockedSignals'].forEach((id) => {
    on(id, 'input', () => { state.onboardingReadiness = evaluateOnboardingReadiness(); syncCredentialInputs(); renderOnboarding(state); });
    on(id, 'change', () => { state.onboardingReadiness = evaluateOnboardingReadiness(); syncCredentialInputs(); renderOnboarding(state); });
  });
}

injectStaticIcons();
wireEvents();
applyTheme(state.settings);
resetAutoRefresh();
hydrateAutoToken().finally(() => refreshBootstrap({ silent: true }).catch(handleError));
