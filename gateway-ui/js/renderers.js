import { renderTaskDonut } from './charts.js?v=20260414a';
import { DEFAULT_SETTINGS, REFRESH_OPTIONS, WALLPAPER_PRESETS, describeSettings } from './settings.js?v=20260414b';
import { buildWallpaperPreviewCards, statusLightClass } from './theme.js?v=20260414b';
import { detailSummaryText, displayStatus, failureSignalLabel, navIcon, rawSummary, statusMeta, summaryText, taskDisplayLabel, taskIcon, taskMeta } from './summary-formatters.js?v=20260414b';

const $ = (id) => document.getElementById(id);
const STAGE_META = { shadow: 'Shadow', active_success: 'Active 成功', active_failure: 'Active 失败', retry_observation: '重试观察', continuity: '连续性恢复' };
const THEME_OPTIONS = [{ value: 'dark', label: '深色' }, { value: 'light', label: '浅色' }];
const ACCENT_OPTIONS = [{ value: 'dynamic', label: '随壁纸动态' }, { value: 'cool', label: 'Ventura 冷色' }, { value: 'warm', label: 'Sonoma 暖色' }];
const SUMMARY_OPTIONS = [{ value: 'zh', label: '中文简化' }, { value: 'raw', label: '技术原文' }];
const DENSITY_OPTIONS = [{ value: 'standard', label: '标准' }, { value: 'compact', label: '紧凑' }];
const MOTION_OPTIONS = [{ value: 'on', label: '开启动画' }, { value: 'off', label: '减少动画' }];
const PROXY_HEALTH_STALE_MS = 1000 * 60 * 120;

export function injectStaticIcons() {
  document.querySelectorAll('[data-icon]').forEach((node) => {
    node.innerHTML = navIcon(node.dataset.icon || 'overview');
  });
}

export function highlightPage(page) {
  document.querySelectorAll('.page').forEach((node) => node.classList.toggle('active', node.id === `page-${page}`));
  document.querySelectorAll('.nav-item[data-page]').forEach((node) => node.classList.toggle('active', node.dataset.page === page));
}

export function populateResourceSelects(bootstrap, currentDraft, loadRememberedSelection) {
  const mapNamed = (items) => (Array.isArray(items) ? items : []).map((item) => ({ value: item.id, label: `${item.name} · v${item.version}` }));
  populateSelect($('behaviorProfile'), mapNamed(bootstrap?.behavior_profiles), pickSelection(currentDraft?.behavior_profile_id, 'behaviorProfile', bootstrap?.behavior_profiles, loadRememberedSelection), '选择行为配置');
  populateSelect($('identityProfile'), mapNamed(bootstrap?.identity_profiles), pickSelection(currentDraft?.identity_profile_id, 'identityProfile', bootstrap?.identity_profiles, loadRememberedSelection), '选择身份配置');
  populateSelect($('sessionProfile'), mapNamed(bootstrap?.session_profiles), pickSelection(currentDraft?.session_profile_id, 'sessionProfile', bootstrap?.session_profiles, loadRememberedSelection), '选择会话配置');
  populateSelect($('fingerprintProfile'), mapNamed(bootstrap?.fingerprint_profiles), pickSelection(currentDraft?.fingerprint_profile_id, 'fingerprintProfile', bootstrap?.fingerprint_profiles, loadRememberedSelection), '选择指纹配置');
  populateSelect(
    $('proxyId'),
    (Array.isArray(bootstrap?.proxies) ? bootstrap.proxies : []).map((item) => ({
      value: item.id,
      label: buildProxyOptionLabel(item),
    })),
    pickSelection(currentDraft?.proxy_id, 'proxyId', bootstrap?.proxies, loadRememberedSelection),
    '自动选择代理',
  );
}

function buildProxyOptionLabel(item) {
  const provider = item?.provider || '未知提供商';
  const region = item?.region || '未知地区';
  const health = item?.proxy_health_grade
    ? `健康 ${item.proxy_health_grade}${item?.proxy_health_score != null ? ` ${formatNumber(item.proxy_health_score, 0)}` : ''}`
    : (item?.score != null ? `分数 ${formatNumber(item.score, 1)}` : '未巡检');
  const trust = item?.trust_score_total != null ? `Trust ${item.trust_score_total}` : 'Trust --';
  return `${provider} / ${region} / ${health} / ${trust}`;
}

export function fillDraftForm(draft) {
  const contract = draft?.final_contract_json || draft?.inferred_contract_json || {};
  const roles = contract.field_roles || {};
  setValue('loginUrl', draft?.login_url || '');
  setValue('successHint', draft?.success_hint || '');
  setValue('behaviorProfile', draft?.behavior_profile_id || $('behaviorProfile')?.value || '');
  setValue('identityProfile', draft?.identity_profile_id || $('identityProfile')?.value || '');
  setValue('sessionProfile', draft?.session_profile_id || $('sessionProfile')?.value || '');
  setValue('fingerprintProfile', draft?.fingerprint_profile_id || $('fingerprintProfile')?.value || '');
  setValue('proxyId', draft?.proxy_id || $('proxyId')?.value || '');
  setValue('credentialMode', draft?.credential_mode || 'alias');
  setValue('credentialRef', draft?.credential_ref || '');
  setValue('primaryFormSelector', contract.primary_form_selector || '');
  setValue('usernameSelector', roles.username?.selector || '');
  setValue('passwordSelector', roles.password?.selector || '');
  setValue('rememberSelector', roles.remember_me?.selector || '');
  setValue('submitSelector', roles.submit?.selector || '');
  setValue('readySelector', contract.success?.ready_selector || '');
  setValue('loginErrorSignals', arrayToInput(contract.error_signals?.login_error));
  setValue('fieldErrorSignals', arrayToInput(contract.error_signals?.field_error));
  setValue('accountLockedSignals', arrayToInput(contract.error_signals?.account_locked));
}

export function renderChrome(state) {
  const bootstrap = state.bootstrap || {};
  const shell = bootstrap.ui_model?.shell || {};
  const stats = bootstrap.gateway_stats_snapshot || {};
  const readonly = Boolean(state.readonly || bootstrap.readonly || shell.readonly);
  const latestEvent = Array.isArray(stats.recent) && stats.recent.length ? stats.recent[0] : null;
  const dataSources = Array.isArray(bootstrap.ui_model?.display_meta?.data_sources) ? bootstrap.ui_model.display_meta.data_sources : [];
  const connected = shell.connection_status ? shell.connection_status === 'online' : (Boolean(bootstrap.status) && !bootstrap.status_error);
  document.body.dataset.dashboardMode = readonly ? 'readonly' : 'admin';
  document.querySelectorAll('.nav-item[data-page="onboarding"]').forEach((node) => node.classList.toggle('hidden', readonly));
  document.querySelectorAll('.side-accordion').forEach((node) => node.classList.toggle('hidden', readonly));
  $('page-onboarding')?.classList.toggle('hidden', readonly);
  $('statusLight').className = statusLightClass(connected || !state.lastError?.isError);
  $('systemTitle').textContent = readonly ? '行为拟真公开预览' : readableTextOr(shell.status_title, '行为拟真控制台');
  $('systemSubtitle').textContent = readonly ? 'Readonly Preview · Monitoring-only' : readableTextOr(shell.status_subtitle, 'Monitoring-first operator console');
  $('runtimeModeBadge').textContent = `模式 ${readableTextOr(shell.runtime_mode_label, runtimeLabel(bootstrap.runtime_mode))}`;
  $('dataSourceBadge').textContent = readonly ? `只读预览 · 数据源 ${dataSources.length || 1} 个` : `数据源 ${dataSources.length || 1} 个`;
  $('refreshBadge').textContent = `刷新 ${Number(state.settings?.refreshInterval || DEFAULT_SETTINGS.refreshInterval)} 秒`;
  $('summaryBadge').textContent = `摘要 ${state.settings?.summaryMode === 'raw' ? '技术原文' : '中文简化'}`;
  $('lastRefreshText').textContent = state.lastRefreshAt ? formatDateTime(state.lastRefreshAt) : '--';
  $('freshnessText').textContent = state.lastRefreshAt ? `${formatAge(state.lastRefreshAt)}前更新` : '等待首次加载';
  $('gatewayStatsHeadline').textContent = stats.total_events ? String(stats.total_events) : '--';
  $('gatewayStatsSubline').textContent = latestEvent ? `${latestEvent.token_label || '未知令牌'} · ${latestEvent.path || '/'}` : '暂无网关统计';
  $('footerSummary').textContent = `刷新间隔：${Number(state.settings?.refreshInterval || DEFAULT_SETTINGS.refreshInterval)} 秒 · 摘要：${state.settings?.summaryMode === 'raw' ? '技术原文' : '中文简化'}`;
  $('footerContinuity').textContent = `连续性事件：${Array.isArray(bootstrap.continuity_events) ? bootstrap.continuity_events.length : 0}`;
}

export function renderStatusNotice(state) {
  const notices = [];
  const shellNotices = Array.isArray(state.bootstrap?.ui_model?.shell?.notices) ? state.bootstrap.ui_model.shell.notices : [];
  const hasReadonlyShellNotice = shellNotices.some((item) => String(item?.title || '').includes('只读预览'));
  if ((state.readonly || state.publicPreview) && !hasReadonlyShellNotice) {
    notices.push({ kind: 'info', title: '只读预览', detail: '当前页面只展示监控数据，不提供站点接入、发布或验证操作。' });
  } else if (!state.bootstrap && !$('adminToken')?.value) {
    notices.push({ kind: 'warning', title: '等待连接', detail: '页面会自动尝试获取管理会话。' });
  }
  if (state.lastError) notices.push({ kind: state.lastError.isError ? 'danger' : 'warning', title: state.lastError.title || '提醒', detail: state.lastError.message || '发生了未知问题。' });
  if (state.bootstrap?.status_error) notices.push({ kind: 'warning', title: '部分数据加载失败', detail: state.bootstrap.status_error });
  shellNotices.forEach((item) => {
    notices.push({
      kind: item?.kind || 'warning',
      title: item?.title || '系统提示',
      detail: item?.detail || '控制面返回了一条未命名提示。',
    });
  });
  const container = $('statusNotice');
  if (!notices.length) {
    container.classList.add('hidden');
    container.innerHTML = '';
    return;
  }
  container.classList.remove('hidden');
  container.innerHTML = notices.map((item) => `<article class="notice-block ${escapeHtml(item.kind)}"><strong>${escapeHtml(item.title)}</strong><p>${escapeHtml(item.detail)}</p></article>`).join('');
}

function populateSelect(select, items, value, placeholder) {
  if (!select) return;
  const normalized = Array.isArray(items) ? items : [];
  const known = new Set(normalized.map((item) => String(item.value)));
  const current = value != null ? String(value) : '';
  const extra = current && !known.has(current) ? [{ value: current, label: `${current}（当前值）` }] : [];
  select.innerHTML = [{ value: '', label: placeholder }, ...extra, ...normalized].map((item) => `<option value="${escapeHtml(String(item.value))}">${escapeHtml(item.label)}</option>`).join('');
  select.value = current;
}

function pickSelection(currentValue, key, items, loadRememberedSelection) {
  if (currentValue) return currentValue;
  const remembered = loadRememberedSelection?.(key);
  if (remembered && Array.isArray(items) && items.some((item) => item.id === remembered)) return remembered;
  return Array.isArray(items) && items.length === 1 ? items[0].id : '';
}

function ensureRefreshSelect(selected) {
  populateSelect($('overviewRefreshInterval'), REFRESH_OPTIONS.map((item) => ({ value: String(item), label: `${item} 秒` })), String(selected || DEFAULT_SETTINGS.refreshInterval), '刷新频率');
}

function ensureOverviewFilters(taskRows, siteRollups, state) {
  populateSelect($('overviewStatusFilter'), [
    { value: 'all', label: '全部状态' },
    { value: 'failed', label: '失败' },
    { value: 'timed_out', label: '超时' },
    { value: 'blocked', label: '阻止' },
    { value: 'running', label: '运行中' },
    { value: 'succeeded', label: '成功' },
    { value: 'shadow_only', label: '仅 Shadow' },
  ], state.overviewStatusFilter || 'all', '状态筛选');
  const sites = ['all', ...new Set([...taskRows.map((item) => item.site_key).filter(Boolean), ...siteRollups.map((item) => item.site_key).filter(Boolean)])];
  populateSelect($('overviewSiteFilter'), sites.map((item) => ({ value: item, label: item === 'all' ? '全部站点' : item })), state.overviewSiteFilter || 'all', '站点筛选');
  document.querySelectorAll('[data-task-tab]').forEach((node) => node.classList.toggle('active', node.dataset.taskTab === (state.overviewTaskTab || 'all')));
  document.querySelectorAll('[data-task-insight]').forEach((node) => node.classList.toggle('active', node.dataset.taskInsight === (state.overviewTaskInsightMode || 'failure')));
}

function filterTaskRows(taskRows, state) {
  return taskRows.filter((item) => {
    const status = displayStatus(item);
    if (state.overviewTaskTab && state.overviewTaskTab !== 'all' && status !== state.overviewTaskTab) return false;
    if (state.overviewStatusFilter && state.overviewStatusFilter !== 'all' && status !== state.overviewStatusFilter) return false;
    if (state.overviewSiteFilter && state.overviewSiteFilter !== 'all' && (item.site_key || '') !== state.overviewSiteFilter) return false;
    return true;
  });
}

function siteDisplay(item) {
  if (item?.site_key) return item.site_key;
  if (item?.final_url) {
    try {
      return new URL(item.final_url).host || '未标记站点';
    } catch {
      return item.final_url;
    }
  }
  return '未标记站点';
}

function sourceKindMeta(kind) {
  const map = {
    text_url: { label: '文本源', icon: 'file' },
    json_file: { label: 'JSON 文件', icon: 'code' },
    text_file: { label: '文本文件', icon: 'file' },
    api: { label: '接口源', icon: 'link' },
    feed: { label: 'Feed', icon: 'globe' },
    unknown: { label: '未知来源', icon: 'database' },
  };
  return map[kind] || { label: kind || '未知来源', icon: 'database' };
}

function taskReasonBadges(item) {
  const badges = [];
  if (item?.failure_signal) badges.push(`<span class="status-chip warning">${escapeHtml(failureSignalLabel(item.failure_signal))}</span>`);
  if (item?.browser_failure_signal) badges.push(`<span class="status-chip warning">${escapeHtml(failureSignalLabel(item.browser_failure_signal))}</span>`);
  if (item?.proxy_health_grade) {
    const tone = proxyHealthTone(item.proxy_health_grade);
    const scoreText = item?.proxy_health_score != null ? ` ${formatNumber(item.proxy_health_score, 0)}` : '';
    badges.push(`<span class="status-chip ${tone}">代理 ${escapeHtml(item.proxy_health_grade)}${escapeHtml(scoreText)}</span>`);
  }
  if (item?.trust_score_total != null) badges.push(`<span class="status-chip neutral">Trust ${escapeHtml(String(item.trust_score_total))}</span>`);
  if (item?.retry_count) badges.push(`<span class="status-chip info">重试 ${escapeHtml(String(item.retry_count))}</span>`);
  return badges.join('');
}

function proxyIdentityText(item) {
  const parts = [item?.proxy_provider, item?.proxy_region, item?.proxy_id, extractSummaryToken(item, 'proxy_id')].filter(Boolean);
  return parts.length ? parts.join(' / ') : '\u672a\u7ed1\u5b9a\u4ee3\u7406';
}

function proxyQualityText(item) {
  if (item?.proxy_health_grade) {
    const score = item?.proxy_health_score != null ? ` / ${formatNumber(item.proxy_health_score, 0)} \u5206` : '';
    return `${item.proxy_health_grade}${score}`;
  }
  if (item?.trust_score_total != null) return `Trust ${item.trust_score_total}`;
  return '\u6682\u65e0\u5065\u5eb7\u5206';
}

function overviewCardIcon(cardId) {
  const map = {
    task_health: 'activity',
    recent_alerts: 'warning',
    proxy_health: 'shield',
    proxy_pool: 'shield',
    session_health: 'usercheck',
  };
  return map[cardId] || 'spark';
}

function renderProxyHealthOverview(model, proxySources = []) {
  const distribution = Array.isArray(model?.grade_distribution) ? model.grade_distribution.filter((item) => Number(item.count || 0) > 0) : [];
  const scoreBands = Array.isArray(model?.score_band_distribution) && model.score_band_distribution.length
    ? model.score_band_distribution.filter((item) => Number(item.count || 0) > 0)
    : buildFallbackScoreBands(distribution);
  const sourceRows = Array.isArray(model?.source_comparison_rows) && model.source_comparison_rows.length
    ? model.source_comparison_rows
    : buildFallbackSourceComparison(proxySources);
  const reasonBuckets = Array.isArray(model?.low_quality_reason_buckets) && model.low_quality_reason_buckets.length
    ? model.low_quality_reason_buckets.filter((item) => Number(item.count || 0) > 0)
    : buildFallbackReasonBuckets(Array.isArray(model?.low_quality_rows) ? model.low_quality_rows : []);
  const lowRows = Array.isArray(model?.low_quality_rows) ? model.low_quality_rows : [];
  const total = Number(model?.total_active || distribution.reduce((sum, item) => sum + Number(item.count || 0), 0));
  $('proxyHealthHeadline').innerHTML = total
    ? [`<span class="status-chip info">\u6d3b\u8dc3 ${formatNumber(total)}</span>`, `<span class="status-chip ${Number(model?.unchecked_count || 0) > 0 ? 'warning' : 'success'}">\u672a\u5de1\u68c0 ${formatNumber(model?.unchecked_count || 0)}</span>`, `<span class="status-chip ${Number(model?.stale_count || 0) > 0 ? 'warning' : 'success'}">\u8fc7\u671f ${formatNumber(model?.stale_count || 0)}</span>`].join('')
    : '<span class="muted tiny">\u7b49\u5f85\u4ee3\u7406\u5065\u5eb7\u6570\u636e</span>';

  if (!total) {
    $('proxyHealthGradeChart').innerHTML = renderEmptyState('\u6682\u65e0\u4ee3\u7406\u5065\u5eb7\u5206\u5e03\u3002');
    $('proxyHealthGradeLegend').innerHTML = '';
    $('proxyHealthScoreBandChart').innerHTML = renderEmptyState('等待低频巡检写入健康快照。');
    $('proxyHealthSourceComparison').innerHTML = renderEmptyState('当前还没有采集源健康对比。');
    $('proxyHealthReasonBuckets').innerHTML = '';
    $('proxyHealthLowList').innerHTML = renderEmptyState('\u6682\u65e0\u4f4e\u5206\u6216\u8fc7\u671f\u4ee3\u7406\u3002');
    return;
  }

  const gradient = buildProxyHealthGradient(distribution, total);
  $('proxyHealthGradeChart').innerHTML = `<div class="big-ring" style="background:${gradient}"><div class="big-ring-inner"><strong>${model?.avg_score != null ? formatNumber(model.avg_score, 0) : '--'}</strong><span>\u5e73\u5747\u5206</span></div></div>`;
  $('proxyHealthGradeLegend').innerHTML = distribution.map((item) => `<div class="proxy-grade-chip"><span class="proxy-grade-dot ${escapeHtml(item.tone || 'neutral')}"></span><strong>${escapeHtml(item.grade)}</strong><small>${escapeHtml(item.label || item.grade)} · ${formatNumber(item.count || 0)}</small></div>`).join('');
  $('proxyHealthScoreBandChart').innerHTML = [
    `<div class="summary-kpi-strip">`,
    renderProxyHealthStat('总活跃代理', formatNumber(model?.total_active || 0)),
    renderProxyHealthStat('已巡检', formatNumber(model?.checked_count || 0)),
    renderProxyHealthStat('未巡检', formatNumber(model?.unchecked_count || 0)),
    renderProxyHealthStat('过期', formatNumber(model?.stale_count || 0)),
    renderProxyHealthStat('健康等级 A+/A/B+', formatNumber(model?.healthy_count || 0)),
    renderProxyHealthStat('需要关注', formatNumber(model?.warning_count || 0)),
    `</div>`,
    renderDistributionPanel(scoreBands, { title: '分数段分布', empty: '暂无分数段统计。', compact: true }),
  ].join('');
  $('proxyHealthSourceComparison').innerHTML = renderSourceComparisonPanel(sourceRows);
  $('proxyHealthReasonBuckets').innerHTML = reasonBuckets.map((item) => `<span class="status-chip ${escapeHtml(item.tone || 'neutral')}">${escapeHtml(item.label)} ${formatNumber(item.count || 0)}</span>`).join('');
  $('proxyHealthLowList').innerHTML = lowRows.length ? lowRows.map(renderProxyHealthLowRow).join('') : renderEmptyState('\u6682\u65e0\u4f4e\u5206\u6216\u8fc7\u671f\u4ee3\u7406\u3002');
}

function renderProxyHealthStat(label, value) {
  return `<div class="proxy-health-stat"><span>${escapeHtml(label)}</span><strong>${escapeHtml(String(value))}</strong></div>`;
}

function renderProxyHealthLowRow(row) {
  const tone = proxyHealthTone(row?.proxy_health_grade);
  const score = row?.proxy_health_score != null ? `${formatNumber(row.proxy_health_score, 0)} \u5206` : '\u672a\u8bc4\u5206';
  const checkedAt = row?.proxy_health_checked_at ? formatDateTime(row.proxy_health_checked_at) : '\u5c1a\u672a\u5de1\u68c0';
  return `<article class="proxy-health-low-row"><div class="proxy-health-low-head"><div><strong>${escapeHtml(row?.provider || row?.proxy_id || '\u672a\u547d\u540d\u4ee3\u7406')}</strong><small>${escapeHtml([row?.region, row?.source_label].filter(Boolean).join(' / ') || row?.proxy_id || '\u65e0\u6765\u6e90\u4fe1\u606f')}</small></div><span class="status-chip ${tone}">${escapeHtml(row?.proxy_health_grade || '\u672a\u5de1\u68c0')}</span></div><p>${escapeHtml(row?.reason || '\u9700\u8981\u5173\u6ce8')}</p><div class="task-inline-meta"><small>${escapeHtml(score)}</small><small>${row?.trust_score_total != null ? `Trust ${escapeHtml(String(row.trust_score_total))}` : 'Trust --'}</small><small>${escapeHtml(checkedAt)}</small></div></article>`;
}

function buildProxyHealthGradient(distribution, total) {
  let offset = 0;
  const segments = distribution.map((item) => {
    const percentage = total > 0 ? (Number(item.count || 0) / total) * 100 : 0;
    const start = offset;
    const end = offset + percentage;
    offset = end;
    return `${proxyHealthColor(item.tone)} ${start}% ${end}%`;
  });
  return `conic-gradient(${segments.join(', ')})`;
}

function proxyHealthTone(grade) {
  if (['A+', 'A', 'B+'].includes(grade)) return 'success';
  if (['B', 'C+', 'unchecked', '\u672a\u5de1\u68c0'].includes(grade)) return 'warning';
  if (['C', 'D', 'F'].includes(grade)) return 'failed';
  return 'neutral';
}

function proxyHealthColor(tone) {
  if (tone === 'ok' || tone === 'success') return 'var(--ok)';
  if (tone === 'warn' || tone === 'warning') return 'var(--warn)';
  if (tone === 'danger' || tone === 'failed') return 'var(--danger)';
  if (tone === 'info') return 'var(--info)';
  return 'rgba(255,255,255,.18)';
}

export async function renderOverview(state) {
  const bootstrap = state.bootstrap || {};
  const overviewModel = bootstrap.ui_model?.overview || {};
  const status = bootstrap.status || {};
  const counts = status.counts || overviewModel.task_status_counts || {};
  const verify = status.verify_metrics || {};
  const proxyPool = status.proxy_pool_status || {};
  const sessionMetrics = status.identity_session_metrics || {};
  const worker = status.worker || {};
  const proxies = Array.isArray(bootstrap.proxies) ? bootstrap.proxies : [];
  const taskRows = Array.isArray(overviewModel.task_rows) ? overviewModel.task_rows : (Array.isArray(bootstrap.overview_tasks) ? bootstrap.overview_tasks : []);
  const siteRollups = Array.isArray(overviewModel.site_rollup_rows) ? overviewModel.site_rollup_rows : (Array.isArray(bootstrap.site_validation_rollups) ? bootstrap.site_validation_rollups : []);
  const continuityRows = Array.isArray(overviewModel.continuity_rows) ? overviewModel.continuity_rows : (Array.isArray(bootstrap.continuity_events) ? bootstrap.continuity_events : []);
  const proxySources = Array.isArray(overviewModel.proxy_source_rows) ? overviewModel.proxy_source_rows : (status.proxy_harvest_metrics?.source_summaries || []);
  const proxyHealthModel = hasProxyHealthData(overviewModel.proxy_health_charts)
    ? overviewModel.proxy_health_charts
    : buildFallbackProxyHealthModel(proxies, proxySources);
  const taskStatusDistribution = Array.isArray(overviewModel.task_status_distribution) && overviewModel.task_status_distribution.length
    ? overviewModel.task_status_distribution
    : buildClientTaskStatusDistribution(taskRows);
  const failureReasonDistribution = Array.isArray(overviewModel.failure_reason_distribution) && overviewModel.failure_reason_distribution.length
    ? overviewModel.failure_reason_distribution
    : buildClientFailureReasonDistribution(taskRows);
  const taskKindDistribution = Array.isArray(overviewModel.task_kind_distribution) && overviewModel.task_kind_distribution.length
    ? overviewModel.task_kind_distribution
    : buildClientTaskKindDistribution(taskRows);
  const primaryCards = Array.isArray(overviewModel.primary_cards) && overviewModel.primary_cards.length
    ? overviewModel.primary_cards
    : [
        { id: 'task_health', title: '任务健康', subtitle: 'Task Health', tone: 'info', value_display: formatNumber(counts.total || 0), lines: [{ label: '成功率', value: `${formatNumber(percent(counts.succeeded || 0, counts.total || 0), 1)}%` }, { label: '运行中', value: formatNumber(counts.running || 0) }, { label: '队列', value: formatNumber(counts.queued || 0) }] },
        { id: 'recent_alerts', title: '最近异常', subtitle: 'Recent Alerts', tone: Number(counts.failed || 0) + Number(counts.timed_out || 0) > 0 ? 'danger' : 'ok', value_display: formatNumber(Number(counts.failed || 0) + Number(counts.timed_out || 0)), lines: [{ label: '失败', value: formatNumber(counts.failed || 0) }, { label: '超时', value: formatNumber(counts.timed_out || 0) }, { label: '阻止', value: formatNumber(siteRollups.filter((item) => item.display_status === 'blocked').length) }] },
        { id: 'proxy_health', title: '代理质量', subtitle: 'Proxy Health', tone: 'accent', value_display: formatNumber(proxyHealthModel.avg_score || 0), lines: [{ label: '活跃 / 总量', value: `${formatNumber(proxyPool.active || 0)}/${formatNumber(proxyPool.total || 0)}` }, { label: '已巡检', value: formatNumber(proxyHealthModel.checked_count || 0) }, { label: '过期', value: formatNumber(proxyHealthModel.stale_count || 0) }] },
        { id: 'session_health', title: '会话健康', subtitle: 'Session Health', tone: continuityRows.some((item) => item.severity === 'warning') ? 'warn' : 'ok', value_display: formatNumber(sessionMetrics.active_sessions || 0), lines: [{ label: '复用率', value: `${formatNumber(percent(sessionMetrics.reused_sessions || 0, (sessionMetrics.reused_sessions || 0) + (sessionMetrics.created_sessions || 0)), 1)}%` }, { label: '复用 / 新建', value: `${formatNumber(sessionMetrics.reused_sessions || 0)} / ${formatNumber(sessionMetrics.created_sessions || 0)}` }, { label: '连续性异常', value: formatNumber(continuityRows.filter((item) => item.severity === 'warning').length) }] },
      ];
  const secondaryMetrics = Array.isArray(overviewModel.secondary_metrics) && overviewModel.secondary_metrics.length
    ? overviewModel.secondary_metrics
    : [
        { label: 'Worker 数', value_display: formatNumber(worker.worker_count || 0) },
        { label: 'Gateway 请求', value_display: formatNumber((bootstrap.gateway_stats_snapshot || {}).total_events || 0) },
        { label: '验证通过 / 失败', value_display: `${formatNumber(verify.verified_ok || 0)} / ${formatNumber(verify.verified_failed || 0)}` },
        { label: '已接入站点', value_display: formatNumber(siteRollups.length) },
      ];

  ensureRefreshSelect(state.settings?.refreshInterval);
  ensureOverviewFilters(taskRows, siteRollups, state);
  const filteredTasks = filterTaskRows(taskRows, state);
  const alertRows = Array.isArray(overviewModel.alert_rows) && overviewModel.alert_rows.length
    ? overviewModel.alert_rows
    : taskRows.filter((item) => ['failed', 'timed_out', 'blocked'].includes(displayStatus(item))).slice(0, 6);
  const selectedTaskId = state.selectedOverviewTaskId
    || overviewModel.initial_selected_task_id
    || alertRows[0]?.id
    || filteredTasks[0]?.id
    || taskRows[0]?.id
    || null;
  const selectedTask = filteredTasks.find((item) => item.id === selectedTaskId)
    || filteredTasks[0]
    || taskRows.find((item) => item.id === selectedTaskId)
    || taskRows[0]
    || null;
  state.selectedOverviewTaskId = selectedTask?.id || null;

  $('overviewPrimaryCards').innerHTML = primaryCards.map((card) => renderPrimaryCard(card.title, card.value_display ?? card.value ?? '--', card.tone || 'neutral', overviewCardIcon(card.id), card.id === 'task_health', Array.isArray(card.lines) ? card.lines.map((line) => [line.label, line.value]) : [], card.subtitle || '监控指标')).join('');
  await renderTaskDonut($('taskDonut'), counts);
  $('overviewSecondaryMetrics').innerHTML = secondaryMetrics.map((item) => renderMiniMetric(item.label, item.value_display ?? item.value ?? '--')).join('');
  renderProxyHealthOverview(proxyHealthModel, proxySources);
  $('overviewTaskInsight').innerHTML = renderDistributionPanel(
    state.overviewTaskInsightMode === 'kind' ? taskKindDistribution : failureReasonDistribution,
    state.overviewTaskInsightMode === 'kind'
      ? { title: '最近任务类型分布', empty: '最近任务里还没有可统计的任务类型。' }
      : { title: '最近失败原因分布', empty: '最近没有失败、超时或阻止类原因。' },
  );

  $('overviewAlertList').innerHTML = alertRows.length ? alertRows.map((item) => renderAlertRow(item)).join('') : renderEmptyState('最近没有失败、超时或阻止类异常。');
  $('overviewTaskList').innerHTML = filteredTasks.length ? filteredTasks.map((item) => renderTaskRow(item, item.id === state.selectedOverviewTaskId)).join('') : renderEmptyState('当前筛选条件下没有任务。');
  $('overviewTaskDetail').innerHTML = selectedTask ? renderTaskDetailPanel(selectedTask) : renderTaskDetailEmpty();
  $('proxySourceGrid').innerHTML = Array.isArray(proxySources) && proxySources.length ? proxySources.map(renderProxySourceCard).join('') : renderEmptyState('当前没有代理采集源统计。');
  const continuityMarkup = continuityRows.length ? continuityRows.map(renderContinuityCard).join('') : renderEmptyState('最近没有连续性事件。');
  $('continuityList').innerHTML = continuityMarkup;
  $('foldContinuityList').innerHTML = continuityMarkup;
  const siteMarkup = siteRollups.length ? siteRollups.slice(0, 5).map((item) => renderSiteRollupCard(item, 'zh')).join('') : renderEmptyState('还没有授权站点验证记录。');
  $('siteRollupList').innerHTML = siteMarkup;
  $('foldSiteList').innerHTML = siteMarkup;
  $('sessionStats').innerHTML = renderSessionStats(sessionMetrics);
  $('phaseChecklist').innerHTML = renderPhaseChecklist(siteRollups, taskRows, continuityRows);
}

function renderPrimaryCard(title, value, tone, icon, chart, lines, subtitle = '监控指标') {
  return `<article class="primary-card" data-tone="${escapeHtml(tone)}"><div class="primary-card-head"><div><p class="eyebrow">${escapeHtml(subtitle)}</p><h3>${escapeHtml(title)}</h3><div class="primary-card-value">${escapeHtml(String(value))}</div></div><div class="icon-badge">${taskIcon(icon)}</div></div>${chart ? '<div class="card-chart" id="taskDonut"></div>' : ''}<div class="primary-card-lines">${lines.map(([label, lineValue]) => `<div><span>${escapeHtml(label)}</span><strong>${escapeHtml(String(lineValue))}</strong></div>`).join('')}</div></article>`;
}

function renderMiniMetric(label, value) {
  return `<div class="mini-metric"><span>${escapeHtml(label)}</span><strong>${escapeHtml(String(value))}</strong></div>`;
}

function renderAlertRow(item, summaryMode) {
  const meta = statusMeta(displayStatus(item));
  return `<article class="alert-row"><div class="alert-row-head"><div class="task-row-title"><span class="icon-badge">${taskIcon(item.kind)}</span><strong>${escapeHtml(taskDisplayLabel(item))}</strong><span class="status-chip ${meta.className}">${escapeHtml(meta.label)}</span></div><small>${escapeHtml(formatDateTime(item.finished_at || item.started_at))}</small></div><p>${escapeHtml(summaryText(item, 'zh'))}</p><div class="task-meta-badges">${taskReasonBadges(item)}</div></article>`;
}

function renderTaskRow(item, isSelected = false) {
  const meta = statusMeta(displayStatus(item));
  const collapsedSummary = summaryText(item, 'zh');
  return `<button type="button" class="task-row selectable${isSelected ? ' selected' : ''}" data-task-select="${escapeHtml(item.id || '')}"><div class="task-row-main"><span class="icon-badge">${taskIcon(item.kind)}</span><div class="task-row-copy"><div class="task-row-title"><strong>${escapeHtml(taskDisplayLabel(item))}</strong><span class="status-chip ${meta.className}">${escapeHtml(meta.label)}</span></div><p>${escapeHtml(collapsedSummary)}</p><div class="task-meta-badges">${taskReasonBadges(item)}</div><div class="task-inline-meta"><small>${escapeHtml(siteDisplay(item))}</small><small>${escapeHtml(proxyIdentityText(item))}</small><small>${escapeHtml(formatDateTime(item.started_at))}</small><small>${escapeHtml(item.id || '-')}</small></div></div></div><div class="task-row-side"><strong>${escapeHtml(proxyQualityText(item))}</strong><small>${escapeHtml(taskMeta(item.kind).subtitle)}</small><small>${item.retry_count != null ? `重试 ${item.retry_count}` : '无重试'}</small><small>${isSelected ? '当前已选中' : '点击查看详情'}</small></div></button>`;
}

function renderTaskDetailEmpty() {
  return renderEmptyState('点击左侧任务列表中的一条记录，这里会显示详细中文摘要、技术原文、代理质量和时间信息。');
}

function renderTaskDetailPanel(item) {
  const meta = statusMeta(displayStatus(item));
  return [
    `<article class="task-detail-summary-card">`,
    `<div class="task-detail-header">`,
    `<div><p class="eyebrow">当前选中任务</p><h3>${escapeHtml(taskDisplayLabel(item))}</h3><p class="muted tiny">${escapeHtml(taskMeta(item.kind).subtitle)}</p></div>`,
    `<span class="status-chip ${escapeHtml(meta.className)}">${escapeHtml(meta.label)}</span>`,
    `</div>`,
    `<div class="task-meta-badges">${taskReasonBadges(item)}</div>`,
    renderSelectionGrid([
      ['站点', siteDisplay(item)],
      ['代理', proxyIdentityText(item)],
      ['代理健康', proxyQualityText(item)],
      ['Trust Score', item.trust_score_total != null ? item.trust_score_total : '--'],
      ['摘要类型', item.summary_kind || '--'],
      ['任务 ID', item.id || '--'],
      ['开始时间', formatDateTime(item.started_at)],
      ['结束时间', formatDateTime(item.finished_at)],
      ['重试次数', item.retry_count != null ? item.retry_count : '0'],
      ['最终地址', item.final_url || '未返回'],
    ]),
    `<div class="evidence-detail-card"><h4>详细中文</h4><p class="detail-copy">${escapeHtml(detailSummaryText(item, 'zh'))}</p></div>`,
    `<details class="subfold"><summary>技术原文</summary><pre class="json-box mono">${escapeHtml(rawSummary(item) || '暂无技术摘要')}</pre></details>`,
    `</article>`,
  ].join('');
}

function renderDistributionPanel(items, options = {}) {
  const rows = Array.isArray(items) ? items.filter((item) => Number(item?.count || 0) > 0) : [];
  if (!rows.length) return renderEmptyState(options.empty || '暂无可视化数据。');
  const maxCount = Math.max(...rows.map((item) => Number(item.count || 0)), 1);
  return `<div class="distribution-list${options.compact ? ' compact' : ''}">${rows.map((item) => renderDistributionRow(item, maxCount)).join('')}</div>`;
}

function hasProxyHealthData(model) {
  if (!model || typeof model !== 'object') return false;
  if (Number(model.total_active || 0) > 0) return true;
  if (Array.isArray(model.grade_distribution) && model.grade_distribution.length > 0) return true;
  if (Array.isArray(model.low_quality_rows) && model.low_quality_rows.length > 0) return true;
  return false;
}

function buildFallbackProxyHealthModel(proxies, proxySources) {
  const rows = Array.isArray(proxies) ? proxies.filter((item) => item?.status === 'active' || item?.proxy_health_score != null) : [];
  const now = Date.now();
  const gradeBuckets = new Map();
  const scoreBands = [
    { key: '90_plus', label: '90+', tone: 'success', count: 0 },
    { key: '80_89', label: '80-89', tone: 'success', count: 0 },
    { key: '70_79', label: '70-79', tone: 'info', count: 0 },
    { key: '60_69', label: '60-69', tone: 'warning', count: 0 },
    { key: 'below_60', label: '<60', tone: 'failed', count: 0 },
  ];
  const reasonBuckets = new Map();
  const sourceBuckets = new Map();
  let checkedCount = 0;
  let staleCount = 0;
  let totalScore = 0;
  const lowQualityRows = [];

  rows.forEach((item) => {
    const score = Number(item?.proxy_health_score);
    const scoreKnown = Number.isFinite(score);
    const grade = item?.proxy_health_grade || (scoreKnown ? inferProxyHealthGrade(score) : '未巡检');
    const checkedAt = parseDate(item?.proxy_health_checked_at);
    const stale = checkedAt ? (now - checkedAt.getTime()) > PROXY_HEALTH_STALE_MS : true;
    const sourceLabel = item?.source_label || item?.provider || '未标记来源';
    const sourceBucket = sourceBuckets.get(sourceLabel) || {
      source_label: sourceLabel,
      avg_score: 0,
      active_count: 0,
      checked_count: 0,
      stale_count: 0,
      low_quality_count: 0,
      _score_total: 0,
    };
    sourceBucket.active_count += 1;
    if (scoreKnown) {
      checkedCount += 1;
      totalScore += score;
      sourceBucket.checked_count += 1;
      sourceBucket._score_total += score;
      const scoreBand = score >= 90 ? '90_plus' : score >= 80 ? '80_89' : score >= 70 ? '70_79' : score >= 60 ? '60_69' : 'below_60';
      const scoreBandBucket = scoreBands.find((bucket) => bucket.key === scoreBand);
      if (scoreBandBucket) scoreBandBucket.count += 1;
    }
    if (stale) {
      staleCount += 1;
      sourceBucket.stale_count += 1;
    }

    const gradeBucket = gradeBuckets.get(grade) || {
      grade,
      label: `等级 ${grade}`,
      tone: proxyHealthTone(grade),
      count: 0,
    };
    gradeBucket.count += 1;
    gradeBuckets.set(grade, gradeBucket);

    const lowQuality = !scoreKnown || stale || score < 70;
    if (lowQuality) {
      sourceBucket.low_quality_count += 1;
      const reason = !scoreKnown ? '未巡检' : stale ? '健康快照过期' : score < 60 ? '低分代理' : '需要关注';
      reasonBuckets.set(reason, (reasonBuckets.get(reason) || 0) + 1);
      lowQualityRows.push({
        provider: item?.provider || '未命名提供商',
        region: item?.region || '未知地区',
        source_label: sourceLabel,
        proxy_id: item?.id || '',
        proxy_health_score: scoreKnown ? score : null,
        proxy_health_grade: grade,
        proxy_health_checked_at: item?.proxy_health_checked_at || null,
        trust_score_total: item?.trust_score_total ?? null,
        reason,
      });
    }

    sourceBuckets.set(sourceLabel, sourceBucket);
  });

  const totalActive = rows.length;
  const checkedRatio = checkedCount > 0 ? totalScore / checkedCount : 0;
  const sourceComparisonRows = Array.from(sourceBuckets.values())
    .map((bucket) => ({
      source_label: bucket.source_label,
      avg_score: bucket.checked_count ? bucket._score_total / bucket.checked_count : Number(proxySources.find((item) => item?.source_label === bucket.source_label)?.health_score || 0),
      active_count: bucket.active_count,
      checked_count: bucket.checked_count,
      stale_count: bucket.stale_count,
      low_quality_count: bucket.low_quality_count,
    }))
    .sort((left, right) => Number(right.avg_score || 0) - Number(left.avg_score || 0));

  return {
    total_active: totalActive,
    checked_count: checkedCount,
    unchecked_count: Math.max(totalActive - checkedCount, 0),
    stale_count: staleCount,
    avg_score: checkedCount ? Number(checkedRatio.toFixed(1)) : 0,
    healthy_count: rows.filter((item) => Number(item?.proxy_health_score || 0) >= 70).length,
    warning_count: rows.filter((item) => item?.proxy_health_score == null || Number(item?.proxy_health_score || 0) < 70).length,
    grade_distribution: Array.from(gradeBuckets.values()).sort((left, right) => Number(right.count || 0) - Number(left.count || 0)),
    score_band_distribution: scoreBands.filter((item) => item.count > 0),
    source_comparison_rows: sourceComparisonRows,
    low_quality_reason_buckets: Array.from(reasonBuckets.entries()).map(([label, count]) => ({
      key: label,
      label,
      tone: label === '低分代理' ? 'failed' : 'warning',
      count,
    })),
    low_quality_rows: lowQualityRows
      .sort((left, right) => {
        const leftScore = left.proxy_health_score == null ? -1 : Number(left.proxy_health_score);
        const rightScore = right.proxy_health_score == null ? -1 : Number(right.proxy_health_score);
        return leftScore - rightScore;
      })
      .slice(0, 8),
  };
}

function inferProxyHealthGrade(score) {
  if (score >= 90) return 'A+';
  if (score >= 80) return 'A';
  if (score >= 70) return 'B+';
  if (score >= 60) return 'B';
  if (score >= 50) return 'C+';
  if (score >= 40) return 'C';
  if (score >= 30) return 'D';
  return 'F';
}

function renderDistributionRow(item, maxCount) {
  const count = Number(item?.count || 0);
  const percentWidth = clamp(percent(count, maxCount), 6, 100);
  return `<div class="distribution-row"><div class="distribution-row-head"><strong>${escapeHtml(item?.label || item?.key || '未命名')}</strong><small>${formatNumber(count)}</small></div><div class="progress-track distribution-track ${escapeHtml(item?.tone || 'neutral')}"><span style="width:${percentWidth}%"></span></div></div>`;
}

function renderSourceComparisonPanel(rows) {
  const items = Array.isArray(rows) ? rows : [];
  if (!items.length) return renderEmptyState('当前还没有采集源健康对比。');
  const maxScore = Math.max(...items.map((item) => Number(item?.avg_score || 0)), 1);
  return `<div class="source-compare-list">${items.map((item) => {
    const score = Number(item?.avg_score || 0);
    return `<article class="source-compare-row"><div class="source-compare-head"><div><strong>${escapeHtml(item?.source_label || '未标记来源')}</strong><small>活跃 ${formatNumber(item?.active_count || 0)} · 低分 ${formatNumber(item?.low_quality_count || 0)} · 过期 ${formatNumber(item?.stale_count || 0)}</small></div><span class="status-chip ${escapeHtml(proxyHealthTone(score >= 80 ? 'A' : score >= 70 ? 'B+' : score >= 60 ? 'B' : 'C'))}">${formatNumber(score, 0)} 分</span></div><div class="progress-track distribution-track info"><span style="width:${clamp(percent(score, maxScore), 6, 100)}%"></span></div></article>`;
  }).join('')}</div>`;
}

function buildFallbackScoreBands(distribution) {
  const bands = [
    { key: '90_plus', label: '90+', tone: 'success', count: 0 },
    { key: '80_89', label: '80-89', tone: 'success', count: 0 },
    { key: '70_79', label: '70-79', tone: 'info', count: 0 },
    { key: '60_69', label: '60-69', tone: 'warning', count: 0 },
    { key: 'below_60', label: '<60', tone: 'failed', count: 0 },
  ];
  const gradeToBand = {
    'A+': '90_plus',
    A: '80_89',
    'B+': '70_79',
    B: '60_69',
    'C+': 'below_60',
    C: 'below_60',
    D: 'below_60',
    F: 'below_60',
  };
  (Array.isArray(distribution) ? distribution : []).forEach((item) => {
    const target = bands.find((bucket) => bucket.key === gradeToBand[item?.grade]);
    if (target) target.count += Number(item?.count || 0);
  });
  return bands.filter((item) => item.count > 0);
}

function buildFallbackSourceComparison(proxySources) {
  return (Array.isArray(proxySources) ? proxySources : []).map((item) => ({
    source_label: item?.source_label || '未标记来源',
    avg_score: Number(item?.health_score || 0),
    active_count: Number(item?.active_count || 0),
    checked_count: Number(item?.active_count || 0),
    stale_count: 0,
    low_quality_count: Math.max(0, Number(item?.candidate_rejected_count || 0)),
  }));
}

function buildFallbackReasonBuckets(rows) {
  const counts = new Map();
  (Array.isArray(rows) ? rows : []).forEach((item) => {
    const label = item?.reason || '其他原因';
    counts.set(label, (counts.get(label) || 0) + 1);
  });
  return Array.from(counts.entries()).map(([label, count]) => ({
    key: label,
    label,
    tone: /过期|失败/i.test(label) ? 'warning' : 'neutral',
    count,
  }));
}

function buildClientTaskStatusDistribution(taskRows) {
  const order = [
    ['failed', '失败', 'failed'],
    ['timed_out', '超时', 'warning'],
    ['blocked', '阻止', 'warning'],
    ['running', '运行中', 'info'],
    ['queued', '排队中', 'neutral'],
    ['succeeded', '成功', 'success'],
    ['shadow_only', '仅 Shadow', 'neutral'],
    ['not_requested', '未请求', 'neutral'],
  ];
  return order.map(([key, label, tone]) => ({
    key,
    label,
    tone,
    count: taskRows.filter((item) => displayStatus(item) === key).length,
  }));
}

function buildClientFailureReasonDistribution(taskRows) {
  const counts = new Map();
  (Array.isArray(taskRows) ? taskRows : []).forEach((item) => {
    const status = displayStatus(item);
    let label = '';
    let tone = 'failed';
    if (status === 'blocked') {
      label = '策略阻止';
      tone = 'warning';
    } else if (status === 'timed_out') {
      label = '等待超时';
      tone = 'warning';
    } else if (status === 'failed') {
      label = failureSignalLabel(item?.failure_signal || item?.browser_failure_signal) || '未知失败';
    }
    if (!label) return;
    const current = counts.get(label) || { key: label, label, tone, count: 0 };
    current.count += 1;
    counts.set(label, current);
  });
  return Array.from(counts.values()).sort((a, b) => b.count - a.count);
}

function buildClientTaskKindDistribution(taskRows) {
  const counts = new Map();
  (Array.isArray(taskRows) ? taskRows : []).forEach((item) => {
    const key = item?.kind || 'unknown';
    const label = taskMeta(key).label;
    const current = counts.get(key) || { key, label, tone: 'info', count: 0 };
    current.count += 1;
    counts.set(key, current);
  });
  return Array.from(counts.values()).sort((a, b) => b.count - a.count);
}

function renderProxySourceCard(row) {
  const score = clamp(Number(row.health_score || 0), 0, 100);
  const sourceMeta = sourceKindMeta(row.source_kind);
  const looksLikeDemo = /demo/i.test(String(row.source_label || '')) || row.enabled === false;
  const activeRate = percent(row.active_count || 0, Math.max(Number(row.candidate_count || 0), 1));
  return `<article class="proxy-source-card${row.enabled ? '' : ' muted-card'}"><div class="proxy-source-head"><div><strong>${escapeHtml(row.source_label || '未命名采集源')}</strong><small>${escapeHtml(sourceMeta.label)}</small></div><span class="status-chip ${row.enabled ? 'success' : 'neutral'}">${row.enabled ? '启用' : '禁用'}</span></div><div class="source-kind-badge"><span class="icon-badge">${taskIcon(sourceMeta.icon)}</span><span>${escapeHtml(row.source_kind || 'unknown')}</span></div><div class="proxy-source-body"><div class="ring-badge" style="background: conic-gradient(var(--accent-1) 0 ${score}%, rgba(255,255,255,.08) ${score}% 100%);">${formatNumber(score, 0)}%</div><div class="task-row-copy"><p>候选 ${formatNumber(row.candidate_count || 0)} → 活跃 ${formatNumber(row.active_count || 0)} → 拒绝 ${formatNumber(row.candidate_rejected_count || 0)}</p><small>Promotion Rate ${formatNumber((Number(row.promotion_rate || 0) * 100), 1)}%</small><small>${looksLikeDemo ? 'Demo 种子（灰显）' : '生产源'}${row.quarantine_until ? ' · 当前隔离中' : ''}</small></div></div><div class="proxy-source-metrics"><div class="selection-item"><span class="label">平均健康</span><strong>${formatNumber(score, 0)} / 100</strong></div><div class="selection-item"><span class="label">活跃率</span><strong>${formatNumber(activeRate, 1)}%</strong></div><div class="selection-item"><span class="label">可用池</span><strong>${formatNumber(row.active_count || 0)}</strong></div></div><div class="progress-track"><span style="width:${score}%"></span></div></article>`;
}

function renderContinuityCard(item) {
  const meta = severityMeta(item.severity);
  return `<article class="continuity-card"><div class="continuity-head"><strong>${escapeHtml(item.event_type || 'continuity')}</strong><span class="status-chip ${meta.className}">${escapeHtml(meta.label)}</span></div><p>${escapeHtml(item.detail_short || '暂无详情')}</p><div class="task-inline-meta"><small>${escapeHtml(item.site_key || '未标记站点')}</small><small>${escapeHtml(item.task_id || '无任务 ID')}</small><small>${escapeHtml(formatDateTime(item.occurred_at))}</small></div></article>`;
}

function renderSiteRollupCard(item, summaryMode) {
  const meta = statusMeta(item.display_status || item.form_action_status || item.status || 'unknown');
  const summary = summaryMode === 'raw'
    ? (item.summary_raw || '暂无摘要')
    : readableTextOr(item.summary_compact_zh, readableTextOr(item.summary_zh, '暂无摘要'));
  return `<article class="site-rollup-card"><div class="site-rollup-head"><div><strong>${escapeHtml(item.site_key || '未命名站点')}</strong><small>${escapeHtml(item.login_url || '')}</small></div><span class="status-chip ${meta.className}">${escapeHtml(meta.label)}</span></div><p>${escapeHtml(summary)}</p><div class="task-inline-meta"><small>${escapeHtml(item.site_policy_id || '未发布策略')}</small><small>${item.retry_count != null ? `重试 ${item.retry_count}` : '无重试'}</small><small>${item.session_persisted ? '已持久化会话' : '未持久化会话'}</small></div></article>`;
}

function renderSessionStats(metrics) {
  return [
    ['Cookie 恢复 / 持久化', metrics.cookie_restore_count || 0, metrics.cookie_persist_count || 0],
    ['LocalStorage 恢复 / 持久化', metrics.local_storage_restore_count || 0, metrics.local_storage_persist_count || 0],
    ['SessionStorage 恢复 / 持久化', metrics.session_storage_restore_count || 0, metrics.session_storage_persist_count || 0],
  ].map(([label, restoreCount, persistCount]) => `<div class="session-row"><div class="session-row-head"><strong>${escapeHtml(label)}</strong><small>${formatNumber(restoreCount)} / ${formatNumber(persistCount)}</small></div><div class="session-bars"><div><small>恢复</small><div class="progress-track"><span style="width:${clamp(percent(restoreCount, Math.max(restoreCount, persistCount, 1)), 0, 100)}%"></span></div></div><div><small>持久化</small><div class="progress-track"><span style="width:${clamp(percent(persistCount, Math.max(restoreCount, persistCount, 1)), 0, 100)}%"></span></div></div></div></div>`).join('');
}

function renderPhaseChecklist(siteRollups, taskRows, continuityRows) {
  const items = [
    ['终态口径统一', !taskRows.some((item) => !['queued', 'running', 'succeeded', 'failed', 'timed_out', 'cancelled', 'blocked', 'shadow_only', 'not_requested'].includes(displayStatus(item)))],
    ['白名单站点已有验证记录', siteRollups.length > 0],
    ['已有连续性证据', continuityRows.some((item) => item.severity === 'success' || item.event_type === 'persist')],
    ['仍保持默认 Shadow / 白名单 Active 边界', true],
  ];
  return items.map(([label, ok]) => `<li><span class="status-chip ${ok ? 'success' : 'warning'}">${ok ? '已满足' : '待补齐'}</span> ${escapeHtml(label)}</li>`).join('');
}

export function renderOnboarding(state) {
  const bootstrap = state.bootstrap || {};
  const onboardingModel = bootstrap.ui_model?.onboarding || {};
  const description = onboardingModel.description || {};
  const drafts = Array.isArray(bootstrap.drafts) ? bootstrap.drafts : [];
  const draft = state.currentDraft || null;
  const readiness = state.onboardingReadiness || {};
  const evidence = draft?.evidence_summary_json || {};
  const entries = getEvidenceEntries(evidence);
  const selectedStage = entries.find((item) => item.key === state.selectedEvidenceStage)?.key || entries[0]?.key || 'active_success';
  $('onboardingSiteList').innerHTML = drafts.length ? drafts.map((item) => renderDraftItem(item, draft?.id)).join('') : renderOnboardingEmptyState(description);
  $('onboardingActionHint').textContent = readiness.hint || readableTextOr(description.minimum_steps, '推荐路径：填写登录页 → 自动发现 → 补齐 selector → 发布并验证');
  if ($('onboardingStepList')) {
    const steps = Array.isArray(description.steps) ? description.steps : [];
    $('onboardingStepList').innerHTML = steps.length
      ? steps.map((item, index) => `<div class="step-pill ${index === 0 ? 'active' : ''}"><span>${index + 1}</span><strong>${escapeHtml(item?.title || `步骤 ${index + 1}`)}</strong><small>${escapeHtml(item?.detail || '')}</small></div>`).join('')
      : $('onboardingStepList').innerHTML;
  }
  $('onboardingSelectionMeta').innerHTML = draft ? renderSelectionGrid([['站点 Key', draft.site_key || '待提取'], ['登录地址', draft.login_url || '未填写'], ['站点策略', draft.site_policy_id ? `${draft.site_policy_id} / v${draft.site_policy_version || 0}` : '尚未发布'], ['凭证模式', draft.credential_mode === 'inline_once' ? '一次性内联' : '身份别名'], ['代理', draft.proxy_id || '自动选择'], ['最后更新', formatDateTime(draft.updated_at)]]) : renderEmptyState('左侧选中一个草稿后，这里会显示当前站点的配置与验证状态。');
  $('shareLinkPreview').textContent = draft?.share_url ? `${window.location.origin}${draft.share_url}` : '当前草稿还没有可复制的分享链接。';
  $('inferredContract').textContent = prettyJson(draft?.inferred_contract_json || draft?.final_contract_json || {});
  $('onboardingDisabledReasons').innerHTML = (readiness.reasons || []).length ? readiness.reasons.map((reason) => `<div class="hint-item">${escapeHtml(reason)}</div>`).join('') : '<div class="hint-item ok">当前前置条件齐备，可以继续发现、发布和验证。</div>';
  applyActionState('createDraftBtn', readiness.createEnabled, readiness.createReason);
  applyActionState('saveDraftBtn', readiness.saveEnabled, readiness.saveReason);
  applyActionState('discoverBtn', readiness.discoverEnabled, readiness.discoverReason);
  applyActionState('publishBtn', readiness.publishEnabled, readiness.publishReason);
  applyActionState('validateBtn', readiness.validateEnabled, readiness.validateReason);
  applyActionState('failureBtn', readiness.sampleEnabled, readiness.sampleReason);
  applyActionState('retryBtn', readiness.sampleEnabled, readiness.sampleReason);
  applyActionState('copyLinkBtn', Boolean(draft?.share_url), draft?.share_url ? '' : '当前草稿还没有分享链接');
  $('recentValidationList').innerHTML = entries.length ? entries.map((item) => renderValidationCard(item.value, item.key, state.settings.summaryMode)).join('') : renderEmptyState('当前草稿还没有验证证据。发布并验证后会在这里显示。');
  $('onboardingEvidenceTabs').innerHTML = entries.length ? entries.map((item) => renderEvidenceTab(item.key, item.value, selectedStage)).join('') : '';
  const selectedEntry = entries.find((item) => item.key === selectedStage)?.value || null;
  $('onboardingEvidenceBody').innerHTML = selectedEntry ? renderEvidenceBody(draft, selectedStage, selectedEntry, state.settings.summaryMode) : renderEmptyState('当前还没有可查看的证据详情。');
  $('evidenceJson').textContent = prettyJson(evidence || {});
}

function renderOnboardingEmptyState(description) {
  return `<article class="empty-guide-card"><strong>${escapeHtml(readableTextOr(description.title, '授权站点（高级）'))}</strong><p>${escapeHtml(readableTextOr(description.when_not_to_use, '日常看监控和异常时，你通常不需要进入这里。'))}</p><div class="hint-list"><div class="hint-item ok">最少步骤：${escapeHtml(readableTextOr(description.minimum_steps, '登录地址 → 自动发现 → 补齐关键 selector → 发布并验证'))}</div></div></article>`;
}

function renderDraftItem(item, activeDraftId) {
  const meta = statusMeta(item.display_status || item.status || 'not_requested');
  return `<button type="button" class="draft-item${item.id === activeDraftId ? ' active' : ''}" data-draft-id="${escapeHtml(item.id)}"><div class="draft-item-head"><div><strong>${escapeHtml(item.site_key || '未命名站点')}</strong><small>${escapeHtml(item.login_url || '')}</small></div><span class="status-chip ${meta.className}">${escapeHtml(meta.label)}</span></div><div class="draft-item-meta"><small>${item.site_policy_id ? `${item.site_policy_id} / v${item.site_policy_version || 0}` : '未发布策略'}</small><small>${escapeHtml(formatDateTime(item.updated_at))}</small></div></button>`;
}

function renderValidationCard(entry, stage, summaryMode) {
  const status = statusMeta(entry.form_action_status || entry.status || 'not_requested');
  const summary = summaryMode === 'raw'
    ? (entry.summary_raw || '暂无摘要')
    : readableTextOr(entry.summary_compact_zh, readableTextOr(entry.summary_zh, '暂无摘要'));
  return `<article class="validation-card"><div class="validation-head"><strong>${escapeHtml(STAGE_META[stage] || stage)}</strong><span class="status-chip ${status.className}">${escapeHtml(status.label)}</span></div><p class="validation-summary">${escapeHtml(summary)}</p><div class="validation-meta"><small>${entry.failure_signal ? `失败信号：${failureSignalLabel(entry.failure_signal)}` : '无失败信号'}</small><small>${entry.retry_count != null ? `重试 ${entry.retry_count}` : '无重试'}</small><small>${entry.session_persisted ? '已持久化会话' : '未持久化会话'}</small></div></article>`;
}

function renderEvidenceTab(stage, entry, selectedStage) {
  const status = statusMeta(entry.form_action_status || entry.status || 'not_requested');
  return `<button type="button" class="evidence-tab${stage === selectedStage ? ' active' : ''}" data-evidence-stage="${escapeHtml(stage)}"><span>${escapeHtml(STAGE_META[stage] || stage)}</span><span class="status-chip ${status.className}">${escapeHtml(status.label)}</span></button>`;
}

function renderEvidenceBody(draft, stage, entry, summaryMode) {
  const summary = summaryMode === 'raw'
    ? (entry.summary_raw || '暂无摘要')
    : readableTextOr(entry.summary_zh, readableTextOr(entry.summary_compact_zh, '暂无摘要'));
  return `<div class="evidence-detail-grid"><div class="evidence-detail-card"><h4>中文摘要</h4><p class="detail-copy">${escapeHtml(summary)}</p></div><div class="evidence-detail-card"><h4>关键状态</h4>${renderFactGrid([['当前阶段', STAGE_META[stage] || stage], ['Task ID', entry.task_id || '未记录'], ['策略版本', draft?.site_policy_id ? `${draft.site_policy_id} / v${draft.site_policy_version || 0}` : '未发布'], ['失败信号', failureSignalLabel(entry.failure_signal)], ['重试次数', entry.retry_count != null ? entry.retry_count : '0'], ['Session 持久化', entry.session_persisted ? '是' : '否'], ['Ready Selector 命中', entry.success_ready_selector_seen ? '是' : '否'], ['首屏动作执行', entry.post_login_actions_executed ? '是' : '否']])}</div><div class="evidence-detail-card span-2"><h4>技术原文</h4><pre class="json-box mono">${escapeHtml(entry.summary_raw || '暂无技术摘要')}</pre></div></div>`;
}

export function renderSettings(state) {
  const settings = { ...DEFAULT_SETTINGS, ...(state.settings || {}) };
  populateSelect($('settingThemeMode'), THEME_OPTIONS, settings.themeMode, '选择主题模式');
  populateSelect($('settingAccentMode'), ACCENT_OPTIONS, settings.accentMode, '选择色调');
  populateSelect($('settingWallpaper'), WALLPAPER_PRESETS.map((item) => ({ value: item.id, label: item.name })), settings.wallpaper, '选择壁纸');
  populateSelect($('settingRefreshInterval'), REFRESH_OPTIONS.map((item) => ({ value: String(item), label: `${item} 秒` })), String(settings.refreshInterval), '选择刷新频率');
  populateSelect($('settingSummaryMode'), SUMMARY_OPTIONS, settings.summaryMode, '选择摘要模式');
  populateSelect($('settingDensity'), DENSITY_OPTIONS, settings.density, '选择卡片密度');
  populateSelect($('settingMotion'), MOTION_OPTIONS, settings.motion, '选择动效模式');
  $('wallpaperPreview').innerHTML = buildWallpaperPreviewCards(settings.wallpaper);
  $('settingsSummary').innerHTML = [renderSettingsCard('当前外观', `${settings.themeMode === 'light' ? '浅色' : '深色'} · ${WALLPAPER_PRESETS.find((item) => item.id === settings.wallpaper)?.name || settings.wallpaper}`), renderSettingsCard('刷新与摘要', `${settings.refreshInterval} 秒 · ${settings.summaryMode === 'raw' ? '技术原文' : '中文简化'}`), renderSettingsCard('密度与动画', `${settings.density === 'compact' ? '紧凑' : '标准'} · ${settings.motion === 'off' ? '减少动画' : '正常动画'}`), renderSettingsCard('组合预览', describeSettings(settings))].join('');
}

function renderSettingsCard(title, value) {
  return `<article class="settings-summary-card"><strong>${escapeHtml(title)}</strong><p>${escapeHtml(value)}</p></article>`;
}
function renderSelectionGrid(entries) {
  return `<div class="selection-grid">${entries.map(([label, value]) => `<div class="selection-item"><span class="label">${escapeHtml(label)}</span><strong>${escapeHtml(String(value))}</strong></div>`).join('')}</div>`;
}

function renderFactGrid(entries) {
  return `<div class="fact-grid">${entries.map(([label, value]) => `<div class="fact-item"><span>${escapeHtml(label)}</span><strong>${escapeHtml(String(value))}</strong></div>`).join('')}</div>`;
}

function renderEmptyState(message) {
  return `<div class="empty-state">${escapeHtml(message)}</div>`;
}

function severityMeta(severity) {
  if (severity === 'success') return { label: '正常', className: 'success' };
  if (severity === 'warning') return { label: '告警', className: 'warning' };
  if (severity === 'danger' || severity === 'error') return { label: '异常', className: 'failed' };
  return { label: severity || '信息', className: 'info' };
}

function runtimeLabel(mode) {
  if (mode === 'prod_live') return '生产 / Prod';
  if (mode === 'dev') return '开发 / Dev';
  if (mode === 'demo') return '演示 / Demo';
  return mode || '未识别';
}

function formatDateTime(value) {
  const date = parseDate(value);
  return date ? date.toLocaleString('zh-CN', { hour12: false }) : '--';
}

function formatAge(value) {
  const date = parseDate(value);
  if (!date) return '--';
  const diff = Math.max(0, Date.now() - date.getTime());
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时`;
  return `${Math.floor(hours / 24)} 天`;
}

function parseDate(value) {
  if (!value && value !== 0) return null;
  const text = String(value).trim();
  if (!text) return null;
  if (/^\d{10}$/.test(text)) return new Date(Number(text) * 1000);
  if (/^\d{13}$/.test(text)) return new Date(Number(text));
  const date = new Date(text);
  return Number.isNaN(date.getTime()) ? null : date;
}

function prettyJson(value) {
  return JSON.stringify(value || {}, null, 2);
}

function setValue(id, value) {
  const element = $(id);
  if (element) element.value = value;
}

function arrayToInput(value) {
  return Array.isArray(value) ? value.join(', ') : '';
}

function escapeHtml(value) {
  return String(value ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}

function formatNumber(value, digits = 0) {
  const number = Number(value || 0);
  return Number.isFinite(number) ? number.toLocaleString('zh-CN', { maximumFractionDigits: digits, minimumFractionDigits: digits }) : '0';
}

function percent(value, total) {
  const left = Number(value || 0);
  const right = Number(total || 0);
  return right ? (left / right) * 100 : 0;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, Number(value || 0)));
}

function getEvidenceEntries(evidence) {
  return Object.keys(STAGE_META).map((key) => ({ key, value: evidence?.[key] })).filter((item) => item.value && typeof item.value === 'object');
}

function applyActionState(id, enabled, reason) {
  const element = $(id);
  if (!element) return;
  element.disabled = !enabled;
  element.title = enabled ? '' : (reason || '当前前置条件不足');
}

function extractSummaryToken(item, key) {
  const raw = rawSummary(item);
  const match = String(raw || '').match(new RegExp(`${key}=([^\\s]+)`));
  return match ? match[1] : '';
}

function readableTextOr(value, fallback) {
  if (typeof value !== 'string') return fallback;
  const text = value.trim();
  if (!text) return fallback;
  if ((text.match(/\?/g) || []).length >= Math.max(3, Math.floor(text.length / 3))) return fallback;
  if (/(浠ｇ|楠岃|鏆傛棤|鎴愬姛|澶辫触|杩炴帴|鍋ュ悍|鐧诲綍|绔欑偣|鎵撳紑|鏈)/u.test(text)) return fallback;
  return text;
}
