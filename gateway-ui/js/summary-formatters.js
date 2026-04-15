const ICON_PATHS = {
  overview: 'M4 11.5 12 4l8 7.5v8a1 1 0 0 1-1 1h-4v-5H9v5H5a1 1 0 0 1-1-1z',
  onboarding: 'M5 6h14M5 12h14M5 18h8',
  settings: 'M12 8.5A3.5 3.5 0 1 1 8.5 12 3.5 3.5 0 0 1 12 8.5Zm8 3-.9-.2a7.2 7.2 0 0 0-.6-1.5l.5-.8-1.9-1.9-.8.5a7.2 7.2 0 0 0-1.5-.6L14.5 4h-3l-.2.9a7.2 7.2 0 0 0-1.5.6l-.8-.5-1.9 1.9.5.8a7.2 7.2 0 0 0-.6 1.5L4 11.5v1l.9.2c.1.5.3 1 .6 1.5l-.5.8 1.9 1.9.8-.5c.5.3 1 .5 1.5.6l.2.9h3l.2-.9c.5-.1 1-.3 1.5-.6l.8.5 1.9-1.9-.5-.8c.3-.5.5-1 .6-1.5l.9-.2z',
  shield: 'M12 4 19 7v5c0 4.4-2.8 8.4-7 9-4.2-.6-7-4.6-7-9V7z',
  globe: 'M12 4a8 8 0 1 1 0 16 8 8 0 0 1 0-16Zm0 0c2.2 2 3.5 5 3.5 8S14.2 18 12 20c-2.2-2-3.5-5-3.5-8S9.8 6 12 4Zm-7.3 5h14.6M4.7 15h14.6',
  camera: 'M4.5 8h3l1.2-2h6.6l1.2 2h3A1.5 1.5 0 0 1 21 9.5v7A1.5 1.5 0 0 1 19.5 18h-15A1.5 1.5 0 0 1 3 16.5v-7A1.5 1.5 0 0 1 4.5 8Zm7.5 2.3A3.7 3.7 0 1 0 15.7 14 3.7 3.7 0 0 0 12 10.3Z',
  scroll: 'M9 5h6M9 9h6M9 13h4M7 3h10a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z',
  file: 'M8 4h6l4 4v12H8a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Zm6 1.5V9h3.5',
  list: 'M7 6h11M7 12h11M7 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01',
  spark: 'M12 3l1.8 5.1L19 10l-5.2 1.9L12 17l-1.8-5.1L5 10l5.2-1.9z',
  lock: 'M7.5 10V8a4.5 4.5 0 0 1 9 0v2M7 10h10a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1Z',
  userplus: 'M15.5 18.5a5.5 5.5 0 1 0-7 0M12 6.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5Zm7 3v4M17 11.5h4',
  usercheck: 'M15.5 18.5a5.5 5.5 0 1 0-7 0M12 6.5a2.5 2.5 0 1 1 0 5 2.5 2.5 0 0 1 0-5Zm4.8 4.8 1.4 1.4 3-3',
  tag: 'M4 12 12 4h7a1 1 0 0 1 1 1v7l-8 8-8-8zm11-5h.01',
  code: 'M8 8 4 12l4 4M16 8l4 4-4 4M13 5l-2 14',
  link: 'M10 13a5 5 0 0 1 0-7l1.5-1.5a5 5 0 0 1 7 7L17 13M14 11a5 5 0 0 1 0 7L12.5 19.5a5 5 0 1 1-7-7L7 11',
  warning: 'M12 4 21 20H3L12 4Zm0 5.5V13m0 3h.01',
  activity: 'M4 13h4l2-4 4 8 2-4h4',
  database: 'M6 6c0-1.1 2.7-2 6-2s6 .9 6 2-2.7 2-6 2-6-.9-6-2Zm0 6c0 1.1 2.7 2 6 2s6-.9 6-2M6 6v12c0 1.1 2.7 2 6 2s6-.9 6-2V6',
  refresh: 'M20 5v5h-5M4 19v-5h5M6.9 9A7 7 0 0 1 18 6l2 4M17.1 15A7 7 0 0 1 6 18l-2-4',
};

export const NAV_ICONS = {
  overview: makeIcon(ICON_PATHS.overview),
  onboarding: makeIcon(ICON_PATHS.onboarding),
  settings: makeIcon(ICON_PATHS.settings),
};

export const TASK_KIND_META = {
  verify_proxy: { label: '\u4ee3\u7406\u9a8c\u8bc1', subtitle: 'verify_proxy', icon: 'shield' },
  browse_site: { label: '\u7ad9\u70b9\u6d4f\u89c8', subtitle: 'browse_site', icon: 'globe' },
  open_page: { label: '\u6253\u5f00\u9875\u9762', subtitle: 'open_page', icon: 'globe' },
  screenshot: { label: '\u9875\u9762\u622a\u56fe', subtitle: 'screenshot', icon: 'camera' },
  scroll_page: { label: '\u9875\u9762\u6eda\u52a8', subtitle: 'scroll_page', icon: 'scroll' },
  extract_content: { label: '\u5185\u5bb9\u63d0\u53d6', subtitle: 'extract_content', icon: 'file' },
  extract_text: { label: '\u6587\u672c\u63d0\u53d6', subtitle: 'extract_text', icon: 'file' },
  scrape_list: { label: '\u5217\u8868\u6293\u53d6', subtitle: 'scrape_list', icon: 'list' },
  parse_api: { label: '\u63a5\u53e3\u89e3\u6790', subtitle: 'parse_api', icon: 'spark' },
  get_title: { label: '\u6807\u9898\u63d0\u53d6', subtitle: 'get_title', icon: 'tag' },
  get_html: { label: 'HTML \u83b7\u53d6', subtitle: 'get_html', icon: 'code' },
  get_final_url: { label: '\u6700\u7ec8\u5730\u5740', subtitle: 'get_final_url', icon: 'link' },
  login: { label: '\u767b\u5f55', subtitle: 'login', icon: 'lock' },
  register: { label: '\u6ce8\u518c', subtitle: 'register', icon: 'userplus' },
  check_session: { label: '\u4f1a\u8bdd\u6821\u9a8c', subtitle: 'check_session', icon: 'usercheck' },
  unknown: { label: '\u4efb\u52a1', subtitle: 'unknown', icon: 'spark' },
};

export const TASK_ICONS = Object.fromEntries(Object.entries(ICON_PATHS).map(([key, path]) => [key, makeIcon(path)]));

const STATUS_META = {
  succeeded: { label: '\u6210\u529f', className: 'success' },
  failed: { label: '\u5931\u8d25', className: 'failed' },
  timed_out: { label: '\u8d85\u65f6', className: 'warning' },
  cancelled: { label: '\u53d6\u6d88', className: 'neutral' },
  running: { label: '\u8fd0\u884c\u4e2d', className: 'info' },
  queued: { label: '\u6392\u961f\u4e2d', className: 'neutral' },
  blocked: { label: '\u963b\u6b62', className: 'warning' },
  shadow_only: { label: '\u4ec5 Shadow', className: 'neutral' },
  not_requested: { label: '\u672a\u8bf7\u6c42', className: 'neutral' },
  unknown: { label: '\u672a\u77e5', className: 'neutral' },
};

const FAILURE_SIGNAL_LABELS = {
  login_error: '\u8d26\u53f7\u6216\u5bc6\u7801\u9519\u8bef',
  field_error: '\u5b57\u6bb5\u6821\u9a8c\u5931\u8d25',
  account_locked: '\u8d26\u53f7\u5df2\u9501\u5b9a',
  missing_required_field: '\u7f3a\u5c11\u5fc5\u586b\u5b57\u6bb5',
  submit_no_effect: '\u63d0\u4ea4\u672a\u751f\u6548',
  transient_dom_error: '\u9875\u9762\u77ac\u65f6 DOM \u5f02\u5e38',
  timeout_waiting_success: '\u7b49\u5f85\u6210\u529f\u8d85\u65f6',
  inline_secret_unavailable: '\u4e00\u6b21\u6027\u51ed\u8bc1\u5df2\u5931\u6548',
  runner_timeout: '\u6267\u884c\u8d85\u65f6',
  browser_launch_failed: '\u6d4f\u89c8\u5668\u542f\u52a8\u5931\u8d25',
  navigation_failed: '\u9875\u9762\u8df3\u8f6c\u5931\u8d25',
};

export function statusMeta(status) {
  return STATUS_META[status] || { label: status || '\u672a\u77e5', className: 'neutral' };
}

export function failureSignalLabel(signal) {
  return FAILURE_SIGNAL_LABELS[signal] || signal || '\u672a\u63d0\u4f9b';
}

export function taskMeta(kind) {
  return TASK_KIND_META[kind] || TASK_KIND_META.unknown;
}

export function taskIcon(kindOrIcon) {
  const iconKey = TASK_KIND_META[kindOrIcon]?.icon || kindOrIcon;
  return TASK_ICONS[iconKey] || TASK_ICONS.spark;
}

export function navIcon(name) {
  return NAV_ICONS[name] || NAV_ICONS.overview;
}

export function displayStatus(item) {
  return item?.display_status || item?.form_action_status || item?.status || 'unknown';
}

export function taskDisplayLabel(item) {
  const localizedName = buildLocalizedTaskDisplayName(item);
  if (localizedName) return localizedName;
  if (isReadableText(item?.task_display_name)) return item.task_display_name.trim();
  if (isReadableText(item?.task_kind_display)) return item.task_kind_display.trim();
  if (isReadableText(item?.title)) return item.title.trim();
  return taskMeta(item?.kind).label;
}

export function summaryText(item, mode = 'zh') {
  if (!item) return '\u6682\u65e0\u6458\u8981';
  if (mode === 'raw') return rawSummary(item) || '\u6682\u65e0\u6280\u672f\u6458\u8981';
  return pickLocalizedSummaryForItem(item, item?.summary_compact_zh) || buildLocalSummary(item, false);
}

export function detailSummaryText(item, mode = 'zh') {
  if (!item) return '\u6682\u65e0\u6458\u8981';
  if (mode === 'raw') return rawSummary(item) || '\u6682\u65e0\u6280\u672f\u6458\u8981';
  return pickLocalizedSummaryForItem(item, item?.summary_zh)
    || pickLocalizedSummaryForItem(item, item?.summary_compact_zh)
    || buildLocalSummary(item, true);
}

export function rawSummary(item) {
  return String(item?.summary_raw || item?.summary || '').trim();
}

function buildLocalSummary(item, detailed) {
  if (isFormLikeTask(item)) return summarizeFormAction(item, detailed);
  if (item?.kind === 'verify_proxy') return summarizeVerifyProxy(item, detailed);
  return summarizeGenericTask(item, detailed);
}

function buildLocalizedTaskDisplayName(item) {
  const kind = item?.kind || 'unknown';
  if (kind === 'verify_proxy') {
    const proxyLabel = buildProxyLabel(item);
    return proxyLabel ? `代理验证 · ${proxyLabel}` : '代理验证';
  }
  if (kind === 'open_page' && isFormLikeTask(item)) {
    return `站点登录验证 · ${buildTargetLabel(item)}`;
  }
  if (['login', 'register', 'check_session'].includes(kind)) {
    return `${taskMeta(kind).label} · ${buildTargetLabel(item)}`;
  }
  if (buildTargetLabel(item)) {
    return `${taskMeta(kind).label} · ${buildTargetLabel(item)}`;
  }
  return '';
}

function isFormLikeTask(item) {
  const kind = item?.kind || '';
  if (['login', 'register', 'check_session'].includes(kind)) return true;
  if (kind === 'open_page') {
    if (typeof item?.form_action_mode === 'string' && item.form_action_mode.trim()) return true;
    if (typeof item?.failure_signal === 'string' && item.failure_signal.trim()) return true;
    if (typeof item?.summary_kind === 'string' && /auth|form/i.test(item.summary_kind)) return true;
    const status = String(item?.form_action_status || '').trim();
    return ['succeeded', 'failed', 'blocked', 'shadow_only', 'running'].includes(status);
  }
  return Boolean(item?.form_action_mode);
}

function summarizeFormAction(item, detailed) {
  const mode = item?.form_action_mode === 'form' ? '\u8868\u5355' : '\u767b\u5f55';
  const status = item?.form_action_status || 'not_requested';
  const retry = Number(item?.retry_count ?? item?.form_action_retry_count ?? 0);
  const failure = failureSignalLabel(item?.failure_signal || item?.form_action_summary_json?.failure_signal || item?.browser_failure_signal);
  if (status === 'succeeded') return detailed ? `${mode}\u6210\u529f\uff1a\u5df2\u5b8c\u6210\u63d0\u4ea4\u4e0e\u9996\u5c4f\u52a8\u4f5c${item?.session_persisted ? '\uff0c\u5e76\u6301\u4e45\u5316\u4f1a\u8bdd' : ''}` : `${mode}\u6210\u529f`;
  if (status === 'shadow_only') return detailed ? `${mode}\u4ec5\u8fd0\u884c Shadow \u8ba1\u5212\uff0c\u672a\u6267\u884c\u771f\u5b9e\u63d0\u4ea4\u6d41\u7a0b` : `${mode}\u4ec5 Shadow`;
  if (status === 'blocked') return detailed ? `${mode}\u88ab\u963b\u6b62\uff1a\u7f3a\u5c11\u5173\u952e contract \u6216 ready selector` : `${mode}\u88ab\u963b\u6b62`;
  if (status === 'failed') return retry > 0 && detailed ? `${mode}\u5931\u8d25\uff1a${failure}\uff0c\u5df2\u6267\u884c ${retry} \u6b21\u77ac\u65f6\u91cd\u8bd5` : `${mode}\u5931\u8d25\uff1a${failure}`;
  if (status === 'running') return `${mode}\u6267\u884c\u4e2d`;
  return `${mode}\u672a\u8bf7\u6c42`;
}

function summarizeVerifyProxy(item, detailed) {
  const raw = rawSummary(item).toLowerCase();
  const proxyLabel = buildProxyLabel(item) || extractToken(rawSummary(item), 'proxy_id') || item?.id || '';
  if (displayStatus(item) === 'succeeded') return detailed ? `\u4ee3\u7406\u9a8c\u8bc1\u6210\u529f\uff1a${proxyLabel || '\u5df2\u901a\u8fc7\u5065\u5eb7\u68c0\u67e5'}` : '\u4ee3\u7406\u9a8c\u8bc1\u6210\u529f';
  let reason = '\u4ee3\u7406\u4e0d\u53ef\u7528';
  if (raw.includes('timeout')) reason = '\u8fde\u63a5\u8d85\u65f6';
  else if (raw.includes('replenishment') || raw.includes('availability')) reason = '\u4ee3\u7406\u6c60\u4f59\u91cf\u4e0d\u8db3';
  else if (raw.includes('proxy not found')) reason = '\u4ee3\u7406\u4e0d\u5b58\u5728';
  else if (raw.includes('refused') || raw.includes('connect') || raw.includes('connection')) reason = '\u8fde\u63a5\u5931\u8d25';
  else if (raw.includes('auth')) reason = '\u4ee3\u7406\u9274\u6743\u5931\u8d25';
  else if (raw.includes('geo') || raw.includes('region')) reason = '\u5730\u57df\u6821\u9a8c\u5931\u8d25';
  return detailed ? `\u9a8c\u8bc1\u4ee3\u7406 ${proxyLabel || '\u672a\u547d\u540d\u4ee3\u7406'} \u5931\u8d25\uff1a${reason}` : `\u9a8c\u8bc1\u5931\u8d25\uff1a${reason}`;
}

function summarizeGenericTask(item, detailed) {
  const label = taskMeta(item?.kind).label;
  const status = displayStatus(item);
  const target = buildTargetLabel(item) || '\u76ee\u6807\u7ad9\u70b9';
  if (status === 'succeeded') return detailed ? `${label}\u6210\u529f\uff1a${target}` : `${label}\u6210\u529f`;
  if (status === 'failed') return detailed ? `${label}\u5931\u8d25\uff1a${failureSignalLabel(item?.browser_failure_signal)}` : `${label}\u5931\u8d25`;
  if (status === 'timed_out') return detailed ? `${label}\u8d85\u65f6\uff1a${target}` : `${label}\u8d85\u65f6`;
  if (status === 'running') return detailed ? `${label}\u8fd0\u884c\u4e2d\uff1a${target}` : `${label}\u8fd0\u884c\u4e2d`;
  if (status === 'blocked') return detailed ? `${label}\u88ab\u963b\u6b62\uff1a\u5f53\u524d\u7ad9\u70b9 contract \u4e0d\u6ee1\u8db3\u8981\u6c42` : `${label}\u88ab\u963b\u6b62`;
  if (status === 'shadow_only') return detailed ? `${label}\u4ec5\u8bb0\u5f55 Shadow \u8ba1\u5212\uff0c\u672a\u6267\u884c\u771f\u5b9e\u52a8\u4f5c` : `${label}\u4ec5 Shadow`;
  return `${label}\uff1a${statusMeta(status).label}`;
}

function chooseReadableText(value) {
  return isReadableText(value) ? String(value).trim() : '';
}

function pickLocalizedSummary(value) {
  const text = chooseReadableText(value);
  if (!text) return '';
  return isMostlyAscii(text) ? '' : text;
}

function pickLocalizedSummaryForItem(item, value) {
  const text = pickLocalizedSummary(value);
  if (!text) return '';
  if (summaryLooksMismatched(item, text)) return '';
  return text;
}

function summaryLooksMismatched(item, text) {
  const kind = item?.kind || '';
  if (kind === 'verify_proxy') {
    return /登录|表单|ready selector|会话/i.test(text);
  }
  if (!isFormLikeTask(item) && /登录未执行|登录未请求|表单未执行|Auth /i.test(text)) {
    return true;
  }
  return false;
}

function isMostlyAscii(text) {
  if (!text) return true;
  const asciiCount = text.replace(/[^\x00-\x7F]/g, '').length;
  return asciiCount / text.length > 0.9;
}

function isReadableText(value) {
  if (typeof value !== 'string') return false;
  const text = value.trim();
  if (!text || text === '????' || text === '???' || text === '--') return false;
  if ((text.match(/\?/g) || []).length >= Math.max(3, Math.floor(text.length / 3))) return false;
  if (looksLikeMojibake(text)) return false;
  return !text.includes('\ufffd');
}

function looksLikeMojibake(text) {
  const suspiciousTokens = ['\u6d60', '\u608a', '\u6960', '\u7609', '\u6fb6', '\u8fab', '\u89e6', '\u951b', '\u6c2b', '\u55d5', '\u7b09', '\u9359', '\u9422'];
  const hits = suspiciousTokens.reduce((count, token) => count + (text.split(token).length - 1), 0);
  if (hits >= 2) return true;
  return /(\u6d60\uff47|\u6960\u5c83|\u6fb6\u8fab|\u951b\u6c2b|\u55d5\u7b09|\u9359\u9422|浠ｇ|楠岃|鏆傛棤|鎴愬姛|澶辫触|杩炴帴|鍋ュ悍|鐧诲綍|鎵撳紑|绔欑偣|鏈)/u.test(text);
}

function extractToken(raw, key) {
  const match = String(raw || '').match(new RegExp(`${key}=([^\\s]+)`));
  return match ? match[1] : '';
}

function buildProxyLabel(item) {
  return [item?.proxy_provider, item?.proxy_region, item?.proxy_id]
    .map((value) => (typeof value === 'string' ? value.trim() : ''))
    .filter(Boolean)
    .join(' / ');
}

function buildTargetLabel(item) {
  return [item?.site_key, item?.title, item?.final_url]
    .map((value) => (typeof value === 'string' ? value.trim() : ''))
    .find(Boolean) || '';
}

function makeIcon(path) {
  return `<svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"${path}\" /></svg>`;
}
