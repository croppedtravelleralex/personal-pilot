export const STORAGE_KEYS = {
  token: 'gateway_admin_token',
  settings: 'dashboard_ui_settings',
  lastSelectionPrefix: 'dashboard_last_',
};

export const WALLPAPER_PRESETS = [
  { id: 'ventura-iris', name: 'Ventura 紫鸢', tone: 'cool' },
  { id: 'ventura-midnight', name: 'Ventura 深夜', tone: 'cool' },
  { id: 'ventura-orbit', name: 'Ventura 轨道', tone: 'cool' },
  { id: 'ventura-atlas', name: 'Ventura 星图', tone: 'cool' },
  { id: 'sonoma-sunrise', name: 'Sonoma 日出', tone: 'warm' },
  { id: 'sonoma-amber', name: 'Sonoma 琥珀', tone: 'warm' },
  { id: 'sonoma-coral', name: 'Sonoma 珊瑚', tone: 'warm' },
  { id: 'sonoma-dune', name: 'Sonoma 沙丘', tone: 'warm' },
  { id: 'aurora-slate', name: '极光 石板', tone: 'neutral' },
  { id: 'aurora-glacier', name: '极光 冰川', tone: 'cool' },
  { id: 'aurora-forest', name: '极光 森林', tone: 'neutral' },
  { id: 'aurora-rose', name: '极光 玫瑰', tone: 'warm' },
];

export const REFRESH_OPTIONS = [5, 10, 30];

export const DEFAULT_SETTINGS = {
  themeMode: 'dark',
  accentMode: 'dynamic',
  wallpaper: 'ventura-iris',
  refreshInterval: 10,
  summaryMode: 'zh',
  density: 'standard',
  motion: 'on',
};

export function loadSettings() {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.settings);
    if (!raw) return { ...DEFAULT_SETTINGS };
    return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

export function saveSettings(settings) {
  localStorage.setItem(STORAGE_KEYS.settings, JSON.stringify(settings));
}

export function getToken() {
  return localStorage.getItem(STORAGE_KEYS.token) || '';
}

export function setToken(value) {
  localStorage.setItem(STORAGE_KEYS.token, value.trim());
}

export function rememberSelection(key, value) {
  localStorage.setItem(`${STORAGE_KEYS.lastSelectionPrefix}${key}`, value || '');
}

export function loadRememberedSelection(key) {
  return localStorage.getItem(`${STORAGE_KEYS.lastSelectionPrefix}${key}`) || '';
}

export function wallpaperById(id) {
  return WALLPAPER_PRESETS.find((item) => item.id === id) || WALLPAPER_PRESETS[0];
}

export function describeSettings(settings) {
  const wallpaper = wallpaperById(settings.wallpaper);
  const themeLabel = settings.themeMode === 'light' ? '浅色' : '深色';
  const accentLabel = settings.accentMode === 'dynamic' ? '随壁纸动态适配' : settings.accentMode === 'warm' ? '暖色' : '冷色';
  const densityLabel = settings.density === 'compact' ? '紧凑' : '标准';
  const motionLabel = settings.motion === 'off' ? '关闭动画' : '开启动画';
  const summaryLabel = settings.summaryMode === 'raw' ? '技术原文' : '中文简化';
  return `${themeLabel} / ${wallpaper.name} / ${accentLabel} / ${densityLabel} / ${motionLabel} / ${summaryLabel} / ${settings.refreshInterval} 秒`;
}
