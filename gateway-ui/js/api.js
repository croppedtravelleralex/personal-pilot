import { getToken } from './settings.js';

export async function api(path, options = {}) {
  const token = getToken();
  const headers = {
    'Content-Type': 'application/json',
    ...(options.headers || {}),
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  const response = await fetch(path, { ...options, headers, signal: options.signal });
  if (!response.ok) {
    const contentType = response.headers.get('content-type') || '';
    if (contentType.includes('application/json')) {
      const payload = await response.json().catch(() => null);
      const message = payload?.error?.message || payload?.message || `HTTP ${response.status}`;
      const error = new Error(message);
      error.status = response.status;
      error.code = payload?.error?.code || null;
      throw error;
    }
    const text = await response.text();
    const error = new Error(text || `HTTP ${response.status}`);
    error.status = response.status;
    throw error;
  }
  const contentType = response.headers.get('content-type') || '';
  if (!contentType.includes('application/json')) {
    return null;
  }
  return response.json();
}

export async function loadBootstrap(signal) {
  return api('/admin/control/bootstrap', { signal });
}

export async function loadPublicBootstrap(signal, path = '/public/dashboard/bootstrap') {
  return api(path, { signal });
}

export async function restoreDraftByShareToken(shareToken, signal) {
  return api(`/admin/control/onboarding-drafts?share_token=${encodeURIComponent(shareToken)}`, { signal });
}

export async function createDraft(payload) {
  return api('/admin/control/onboarding-drafts', {
    method: 'POST',
    body: JSON.stringify(payload),
  });
}

export async function patchDraft(id, payload) {
  return api(`/admin/control/onboarding-drafts/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(payload),
  });
}

export async function discoverDraft(id) {
  return api(`/admin/control/onboarding-drafts/${id}/discover`, { method: 'POST' });
}

export async function publishDraft(id) {
  return api(`/admin/control/onboarding-drafts/${id}/publish`, { method: 'POST' });
}

export async function runValidation(id, payload) {
  return api(`/admin/control/onboarding-drafts/${id}/run-validation`, {
    method: 'POST',
    body: JSON.stringify(payload),
  });
}
