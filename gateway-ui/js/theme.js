import { WALLPAPER_PRESETS, wallpaperById } from './settings.js';

export function applyTheme(settings) {
  const root = document.documentElement;
  const wallpaper = wallpaperById(settings.wallpaper);
  root.dataset.themeMode = settings.themeMode;
  root.dataset.wallpaper = wallpaper.id;
  root.dataset.density = settings.density === 'compact' ? 'compact' : 'standard';
  root.dataset.motion = settings.motion === 'off' ? 'off' : 'on';
  root.dataset.accentOverride = settings.accentMode === 'dynamic' ? wallpaper.tone === 'warm' ? 'warm' : 'cool' : settings.accentMode;
}

export function buildWallpaperPreviewCards(activeWallpaperId) {
  return WALLPAPER_PRESETS.map((preset) => {
    const selected = preset.id === activeWallpaperId ? ' selected' : '';
    return `<button type="button" class="wallpaper-preview-card${selected}" data-wallpaper-select="${preset.id}" data-label="${preset.name}" style="background: var(--wallpaper-overlay), var(--wallpaper-base);"></button>`;
  }).join('');
}

export function statusLightClass(online) {
  return online ? 'status-light' : 'status-light offline';
}
