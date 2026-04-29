package behavior

// RecordingInjectJS is the JavaScript snippet injected into a browser page.
// It records down/up atomics instead of click to avoid duplicate playback.
// It also persists state before navigation so the same CDP target can be
// re-injected after refresh/navigation and continue accumulating events.
const RecordingInjectJS = `
(function() {
  if (window.__antRecorderVersion === 3 && window.__antRecorder) return;

  const STORE_KEY = '__antRecorderStateV1';
  const NAME_PREFIX = '__antRecorderStateV1__:';
  const MAX_TEXT_LENGTH = 512;
  const FRAME_EVENT_TYPE = '__antRecorderFrameEventV1';

  function parseStored(raw) {
    if (!raw || typeof raw !== 'string') return null;
    try {
      const payload = raw.startsWith(NAME_PREFIX) ? raw.slice(NAME_PREFIX.length) : raw;
      const parsed = JSON.parse(payload);
      if (parsed && Array.isArray(parsed.events)) return parsed;
    } catch (_) {}
    return null;
  }

  function restoreState() {
    const candidates = [];
    try {
      const fromSession = parseStored(window.sessionStorage && window.sessionStorage.getItem(STORE_KEY));
      if (fromSession) candidates.push(fromSession);
    } catch (_) {}
    try {
      const fromName = parseStored(window.name);
      if (fromName) candidates.push(fromName);
    } catch (_) {}
    if (Array.isArray(window.__antRecordedEvents)) {
      candidates.push({ events: window.__antRecordedEvents, meta: window.__antRecorderMeta || {} });
    }
    candidates.sort(function(a, b) {
      return (b.events || []).length - (a.events || []).length;
    });
    return candidates[0] || { events: [], meta: {} };
  }

  const restored = restoreState();
  const events = Array.isArray(restored.events) ? restored.events : [];
  const meta = restored.meta || {};
  const lastT = events.length ? Number(events[events.length - 1].t || 0) : 0;
  const baseElapsed = Math.max(Number(restored.elapsedMs || 0), lastT, 0);
  const start = performance.now() - baseElapsed;

  if (!meta.startUrl) meta.startUrl = String(window.location && window.location.href || '');
  if (!meta.startTitle) meta.startTitle = String(document.title || '');

  window.__antRecordedEvents = events;
  window.__antRecorderMeta = meta;

  function isTopWindow() {
    try {
      return window.top === window;
    } catch (_) {
      return false;
    }
  }

  const isTop = isTopWindow();

  function eventTime() {
    return Math.round(performance.now() - start);
  }

  function currentMeta() {
    meta.currentUrl = String(window.location && window.location.href || '');
    meta.title = String(document.title || '');
    meta.devicePixelRatio = Number(window.devicePixelRatio || 1);
    meta.scale = Number(window.visualViewport && window.visualViewport.scale || 1);
    meta.viewportW = Number(window.innerWidth || 0);
    meta.viewportH = Number(window.innerHeight || 0);
    return meta;
  }

  function persistState() {
    const payload = JSON.stringify({
      events: events,
      meta: currentMeta(),
      elapsedMs: Math.max(eventTime(), 0)
    });
    try {
      if (window.sessionStorage) window.sessionStorage.setItem(STORE_KEY, payload);
    } catch (_) {}
    try {
      window.name = NAME_PREFIX + payload;
    } catch (_) {}
  }

  let persistTimer = 0;
  function schedulePersist() {
    if (persistTimer) return;
    persistTimer = window.setTimeout(function() {
      persistTimer = 0;
      persistState();
    }, 250);
  }

  function push(evt) {
    events.push(evt);
    publishFrameEvent(evt);
    schedulePersist();
  }

  function escapeCSS(value) {
    try {
      if (window.CSS && CSS.escape) return CSS.escape(String(value));
    } catch (_) {}
    return String(value).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
  }

  function cssPath(el) {
    if (!el || el === window || el === document || el.nodeType !== 1) return '';
    const parts = [];
    let cur = el;
    while (cur && cur.nodeType === 1 && cur !== document.documentElement) {
      const tag = String(cur.tagName || '').toLowerCase();
      if (!tag) break;
      if (cur.id) {
        parts.unshift(tag + '#' + escapeCSS(cur.id));
        break;
      }
      let part = tag;
      if (cur.classList && cur.classList.length) {
        const classes = Array.prototype.slice.call(cur.classList, 0, 3).map(escapeCSS);
        if (classes.length) part += '.' + classes.join('.');
      }
      let nth = 1;
      let sib = cur;
      while ((sib = sib.previousElementSibling)) {
        if (sib.tagName === cur.tagName) nth++;
      }
      part += ':nth-of-type(' + nth + ')';
      parts.unshift(part);
      cur = cur.parentElement;
    }
    return parts.length ? parts.join(' > ') : '';
  }

  function ownFramePath() {
    if (isTop) return '';
    try {
      return cssPath(window.frameElement);
    } catch (_) {
      return '';
    }
  }

  const localFramePath = ownFramePath();

  function findFrameElement(frameWindow) {
    try {
      const frames = document.querySelectorAll('iframe, frame');
      for (let i = 0; i < frames.length; i++) {
        try {
          if (frames[i].contentWindow === frameWindow) return cssPath(frames[i]);
        } catch (_) {}
      }
    } catch (_) {}
    return '';
  }

  function withFrameContext(evt, framePath) {
    if (!framePath) return evt;
    const out = {};
    for (const key in evt) out[key] = evt[key];
    out.targetPath = out.targetPath ? framePath + ' >> ' + out.targetPath : framePath;
    return out;
  }

  function publishFrameEvent(evt) {
    if (isTop) return;
    try {
      window.top.postMessage({
        type: FRAME_EVENT_TYPE,
        event: evt,
        framePath: localFramePath
      }, '*');
    } catch (_) {}
  }

  function scrollContainer(target) {
    let cur = target && target.nodeType === 1 ? target : target && target.parentElement;
    while (cur && cur !== document.documentElement && cur !== document.body) {
      const style = window.getComputedStyle ? window.getComputedStyle(cur) : null;
      const overflow = style ? (style.overflow + style.overflowX + style.overflowY) : '';
      const scrollable = /(auto|scroll|overlay)/.test(overflow) &&
        (cur.scrollHeight > cur.clientHeight || cur.scrollWidth > cur.clientWidth);
      if (scrollable) return cur;
      cur = cur.parentElement;
    }
    return document.scrollingElement || document.documentElement || document.body;
  }

  function recordPointer(type, e) {
    const t = Math.round(performance.now() - start);
    const evt = { t, type };
    if (e) {
      if (typeof e.clientX === 'number') { evt.x = e.clientX; evt.y = e.clientY; }
      if (typeof e.button === 'number') evt.btn = e.button;
      if (e.deltaX !== undefined) { evt.dx = e.deltaX; evt.dy = e.deltaY; }
      if (type === 'scroll') {
        const path = cssPath(scrollContainer(e.target));
        if (path) evt.targetPath = path;
      }
    }
    push(evt);
  }

  function textTarget(target) {
    if (!target) return null;
    if (target.closest) {
      const matched = target.closest('input, textarea, [contenteditable="true"], [contenteditable="plaintext-only"]');
      if (matched) return matched;
    }
    return target;
  }

  function targetText(target) {
    const el = textTarget(target);
    if (!el) return '';
    if (typeof el.value === 'string') return el.value;
    if (el.isContentEditable) return el.innerText || el.textContent || '';
    return '';
  }

  function isSensitiveTarget(target) {
    const el = textTarget(target);
    if (!el) return false;
    const type = String(el.type || '').toLowerCase();
    if (type === 'password') return true;
    const attrs = [
      el.name, el.id, el.autocomplete, el.placeholder,
      el.getAttribute && el.getAttribute('aria-label'),
      el.getAttribute && el.getAttribute('data-testid'),
      el.getAttribute && el.getAttribute('data-test')
    ].filter(Boolean).join(' ').toLowerCase();
    return /(password|passwd|pwd|otp|captcha|verification|verify|code|sms|token|secret|auth|\u9a8c\u8bc1\u7801|\u6821\u9a8c|\u77ed\u4fe1|\u52a8\u6001\u7801)/.test(attrs);
  }

  function safeText(value) {
    const text = String(value || '');
    if (text.length <= MAX_TEXT_LENGTH) return text;
    return text.slice(0, MAX_TEXT_LENGTH);
  }

  function recordText(type, e, textOverride) {
    const evt = { t: eventTime(), type };
    const sensitive = isSensitiveTarget(e && e.target);
    if (e && e.inputType) evt.inputType = e.inputType;
    if (type === 'paste') evt.inputType = 'paste';
    if (type === 'composition' && e && e.type) evt.inputType = e.type;
    if (sensitive) {
      evt.sensitive = true;
    } else {
      const text = textOverride !== undefined ? textOverride : targetText(e && e.target);
      if (text !== undefined && text !== null && String(text) !== '') evt.text = safeText(text);
    }
    push(evt);
  }

  function withModifiers(e, key) {
    let out = key;
    if (e.ctrlKey) out = 'Control+' + out;
    if (e.altKey) out = 'Alt+' + out;
    if (e.shiftKey) out = 'Shift+' + out;
    return out;
  }

  function throttle(fn, ms) {
    let last = 0;
    return function(...args) {
      const now = performance.now();
      if (now - last >= ms) { last = now; return fn.apply(this, args); }
    };
  }

  const disposers = [];
  function listen(type, fn) {
    document.addEventListener(type, fn, true);
    disposers.push(function() { document.removeEventListener(type, fn, true); });
  }
  function listenWindow(type, fn) {
    window.addEventListener(type, fn, true);
    disposers.push(function() { window.removeEventListener(type, fn, true); });
  }

  if (isTop) {
    listenWindow('message', function(e) {
      const data = e && e.data;
      if (!data || data.type !== FRAME_EVENT_TYPE || !data.event) return;
      const framePath = data.framePath || findFrameElement(e.source);
      push(withFrameContext(data.event, framePath));
    });
  }

  listen('mousemove', throttle(function(e) { recordPointer('move', e); }, 50));
  listen('mousedown', function(e) { recordPointer('down', e); });
  listen('mouseup', function(e) { recordPointer('up', e); });
  listen('keydown', function(e) {
    const sensitive = isSensitiveTarget(e.target);
    const evt = { t: eventTime(), type: 'key' };
    const rawKey = String(e.key || '');
    evt.key = sensitive && rawKey.length === 1 ? 'Character' : withModifiers(e, rawKey);
    if (!sensitive && rawKey.length === 1) evt.text = rawKey;
    if (sensitive) evt.sensitive = true;
    push(evt);
  });
  listen('wheel', function(e) { recordPointer('scroll', e); });
  listen('input', function(e) { recordText('input', e); });
  listen('change', function(e) { recordText('change', e); });
  listen('paste', function(e) {
    let text = '';
    try {
      text = e.clipboardData ? e.clipboardData.getData('text') : '';
    } catch (_) {}
    recordText('paste', e, text);
  });
  listen('compositionstart', function(e) { recordText('composition', e, ''); });
  listen('compositionend', function(e) { recordText('composition', e, e.data || ''); });
  listenWindow('pagehide', persistState);
  listen('visibilitychange', function() {
    if (document.visibilityState === 'hidden') persistState();
  });
  listenWindow('beforeunload', persistState);

  window.__antRecorderSnapshot = function() {
    persistState();
    return { events: events.slice(), meta: currentMeta() };
  };
  window.__antRecorderTeardown = function() {
    while (disposers.length) {
      try { disposers.pop()(); } catch (_) {}
    }
    try {
      if (window.sessionStorage) window.sessionStorage.removeItem(STORE_KEY);
    } catch (_) {}
    try {
      if (typeof window.name === 'string' && window.name.startsWith(NAME_PREFIX)) window.name = '';
    } catch (_) {}
    delete window.__antRecordedEvents;
    delete window.__antRecorderMeta;
    delete window.__antRecorderSnapshot;
    delete window.__antRecorderTeardown;
    delete window.__antRecorder;
    delete window.__antRecorderVersion;
  };
  persistState();
  window.__antRecorder = true;
  window.__antRecorderVersion = 3;
})();
`
