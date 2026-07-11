package behavior

import (
	"encoding/json"
	"fmt"
	"hash/fnv"
	"sort"
	"strconv"
	"strings"
)

// EnvironmentInjectionProfile is the serializable subset of browser environment
// controls that can be injected before page scripts run.
type EnvironmentInjectionProfile struct {
	Seed                string            `json:"seed"`
	Languages           []string          `json:"languages"`
	Platform            string            `json:"platform"`
	Vendor              string            `json:"vendor"`
	UserAgent           string            `json:"userAgent"`
	HardwareConcurrency int               `json:"hardwareConcurrency"`
	DeviceMemory        int               `json:"deviceMemory"`
	Timezone            string            `json:"timezone"`
	TimezoneOffset      *int              `json:"timezoneOffset"`
	AcceptLanguage      string            `json:"acceptLanguage"`
	WebGLVendor         string            `json:"webglVendor"`
	WebGLRenderer       string            `json:"webglRenderer"`
	WebGLExtensions     []string          `json:"webglExtensions"`
	AudioNoise          float64           `json:"audioNoise"`
	WebRTCPolicy        WebRTCPolicy      `json:"webRtcPolicy"`
	MediaPermission     string            `json:"mediaPermission"`
	Plugins             []PluginInjection `json:"plugins"`
	MediaDevices        []MediaDeviceSpec `json:"mediaDevices"`
	FontAllowlist       []string          `json:"fontAllowlist"`
	Headers             map[string]string `json:"headers"`
	MaxTouchPoints      int               `json:"maxTouchPoints"`
	ColorDepth          int               `json:"colorDepth"`
	DoNotTrack          bool              `json:"doNotTrack"`
	DevicePixelRatio    string            `json:"devicePixelRatio"`
	WindowSize          string            `json:"windowSize"`
	ScreenWidth         int               `json:"screenWidth"`
	ScreenHeight        int               `json:"screenHeight"`
	BrandVersion        string            `json:"brandVersion"`
	PrefersColorScheme  string            `json:"prefersColorScheme"`
	NetworkEffectiveType string           `json:"networkEffectiveType"`
	NetworkRTT          int               `json:"networkRtt"`
	NetworkDownlink     float64           `json:"networkDownlink"`
}

type WebRTCPolicy struct {
	Mode       string   `json:"mode"`
	AllowHosts []string `json:"allowHosts"`
}

type PluginInjection struct {
	Name        string `json:"name"`
	Filename    string `json:"filename"`
	Description string `json:"description"`
	MimeType    string `json:"mimeType"`
}

type MediaDeviceSpec struct {
	Kind    string `json:"kind"`
	Label   string `json:"label"`
	GroupID string `json:"groupId"`
	ID      string `json:"id"`
}

type EnvironmentInjectionPlan struct {
	Script          string            `json:"script"`
	HeaderOverrides map[string]string `json:"headerOverrides"`
	AppliedFamilies []string          `json:"appliedFamilies"`
	Warnings        []string          `json:"warnings"`
}

// CompileEnvironmentInjectionScript compiles profile controls into one script so
// CDP can inject it with a single Page.addScriptToEvaluateOnNewDocument call.
// The script is self-contained for Worker/SharedWorker bootstrap (docs/50 C1).
func CompileEnvironmentInjectionScript(profile EnvironmentInjectionProfile) (EnvironmentInjectionPlan, error) {
	normalized := normalizeEnvironmentProfile(profile)
	encoded, err := json.Marshal(normalized)
	if err != nil {
		return EnvironmentInjectionPlan{}, fmt.Errorf("marshal environment profile: %w", err)
	}

	warnings := environmentInjectionWarnings(normalized)
	script := strings.TrimSpace(fmt.Sprintf(`(() => {
  const profile = %s;
  function installEnvironment(profile) {
    const root = globalThis;
    const originalFunctionToString = Function.prototype.toString;
    const nativeFunctionMap = new WeakMap();
    const makeNative = (fake, original) => {
      if (typeof fake === 'function') nativeFunctionMap.set(fake, typeof original === 'function' ? original : originalFunctionToString);
      return fake;
    };
    const patchedFunctionToString = function toString() {
      const original = nativeFunctionMap.get(this);
      return Reflect.apply(originalFunctionToString, original || this, []);
    };
    nativeFunctionMap.set(patchedFunctionToString, originalFunctionToString);
    const toStringDescriptor = Object.getOwnPropertyDescriptor(Function.prototype, 'toString') || {};
    Object.defineProperty(Function.prototype, 'toString', Object.assign({}, toStringDescriptor, { value: patchedFunctionToString }));
    const findPropertyDescriptor = (target, key) => {
      let current = target;
      while (current) {
        const descriptor = Object.getOwnPropertyDescriptor(current, key);
        if (descriptor) return descriptor;
        current = Object.getPrototypeOf(current);
      }
      return undefined;
    };
    const defineGetter = (target, key, value) => {
      if (!target) return;
      try {
        const descriptor = findPropertyDescriptor(target, key);
        const originalGetter = descriptor && descriptor.get;
        const getter = function() { return value; };
        makeNative(getter, originalGetter || originalFunctionToString);
        Object.defineProperty(target, key, {
          get: getter,
          configurable: true,
          enumerable: descriptor ? Boolean(descriptor.enumerable) : false
        });
      } catch (_) {}
    };
    const replaceMethod = (target, key, factory) => {
      if (!target || typeof target[key] !== 'function') return undefined;
      const original = target[key];
      const replacement = makeNative(factory(original), original);
      const descriptor = findPropertyDescriptor(target, key);
      try {
        Object.defineProperty(target, key, {
          value: replacement,
          writable: descriptor ? Boolean(descriptor.writable) : true,
          configurable: descriptor ? Boolean(descriptor.configurable) : true,
          enumerable: descriptor ? Boolean(descriptor.enumerable) : false
        });
      } catch (_) {
        try { target[key] = replacement; } catch (_) {}
      }
      return replacement;
    };
    const stableNoise = (salt) => {
      let h = 2166136261;
      const text = String(profile.seed || "personal-pilot") + ":" + salt;
      for (let i = 0; i < text.length; i++) { h ^= text.charCodeAt(i); h = Math.imul(h, 16777619); }
      return ((h >>> 0) %% 997) / 997000;
    };

    const navProtos = [];
    if (typeof Navigator !== 'undefined' && Navigator.prototype) navProtos.push(Navigator.prototype);
    if (typeof WorkerNavigator !== 'undefined' && WorkerNavigator.prototype) navProtos.push(WorkerNavigator.prototype);
    for (const navProto of navProtos) {
      defineGetter(navProto, 'webdriver', undefined);
      if (profile.languages && profile.languages.length) defineGetter(navProto, 'languages', Object.freeze(profile.languages.slice()));
      if (profile.platform) defineGetter(navProto, 'platform', profile.platform);
      if (profile.vendor) defineGetter(navProto, 'vendor', profile.vendor);
      if (profile.userAgent) defineGetter(navProto, 'userAgent', profile.userAgent);
      if (profile.hardwareConcurrency > 0) defineGetter(navProto, 'hardwareConcurrency', profile.hardwareConcurrency);
      if (profile.deviceMemory > 0) defineGetter(navProto, 'deviceMemory', profile.deviceMemory);
      if (profile.maxTouchPoints > 0) defineGetter(navProto, 'maxTouchPoints', profile.maxTouchPoints);
      if (profile.doNotTrack) defineGetter(navProto, 'doNotTrack', '1');
      const plugins = (profile.plugins || []).map((p) => ({ name: p.name, filename: p.filename, description: p.description }));
      defineGetter(navProto, 'plugins', Object.freeze(plugins));
      defineGetter(navProto, 'mimeTypes', Object.freeze((profile.plugins || []).filter((p) => p.mimeType).map((p) => ({ type: p.mimeType, enabledPlugin: p.name }))));
    }
    if (typeof root.chrome === 'undefined' && typeof document !== 'undefined') {
      const csiFn = makeNative(function csi() { return {}; });
      const loadTimesFn = makeNative(function loadTimes() { return {}; });
      root.chrome = {
        runtime: {},
        app: { isInstalled: false },
        csi: csiFn,
        loadTimes: loadTimesFn
      };
    }
    if (root.navigator && root.navigator.permissions && root.navigator.permissions.query) {
      replaceMethod(root.navigator.permissions, 'query', (original) => async function query(desc) {
        const name = desc && desc.name;
        if (name === 'notifications') {
          const perm = (typeof Notification !== 'undefined' && Notification.permission) || 'default';
          return { state: perm === 'default' ? 'prompt' : perm, onchange: null };
        }
        try { return await original.call(this, desc); } catch (e) {
          return { state: 'prompt', onchange: null };
        }
      });
    }
    if (profile.colorDepth > 0 && root.screen) defineGetter(root.screen, 'colorDepth', profile.colorDepth);

    if (profile.timezone && typeof Intl !== 'undefined' && Intl.DateTimeFormat) {
      replaceMethod(Intl.DateTimeFormat.prototype, 'resolvedOptions', (originalResolvedOptions) => function resolvedOptions() {
        const value = originalResolvedOptions.call(this);
        return Object.assign({}, value, { timeZone: profile.timezone, locale: (profile.languages && profile.languages[0]) || value.locale });
      });
    }
    if (Number.isFinite(profile.timezoneOffset)) {
      replaceMethod(Date.prototype, 'getTimezoneOffset', () => function getTimezoneOffset() { return profile.timezoneOffset; });
    }

    const canvasNoise = (data, salt) => {
      for (let i = 0; i < data.length; i += 4) {
        const px = (i / 4) | 0;
        data[i] = Math.max(0, Math.min(255, data[i] + stableNoise(salt + ':r:' + px)));
        data[i + 1] = Math.max(0, Math.min(255, data[i + 1] + stableNoise(salt + ':g:' + px)));
        data[i + 2] = Math.max(0, Math.min(255, data[i + 2] + stableNoise(salt + ':b:' + px)));
        // alpha channel left untouched for visual stability
      }
    };
    if (typeof CanvasRenderingContext2D !== 'undefined' && CanvasRenderingContext2D.prototype) {
      replaceMethod(CanvasRenderingContext2D.prototype, 'getImageData', (originalGetImageData) => function getImageData(...args) {
        const image = originalGetImageData.apply(this, args);
        canvasNoise(image.data, 'getImageData');
        return image;
      });
      replaceMethod(CanvasRenderingContext2D.prototype, 'fillText', (originalFillText) => function fillText(text, x, y, ...rest) {
        return originalFillText.call(this, text, x + stableNoise('fillText'), y, ...rest);
      });
    }
    if (typeof HTMLCanvasElement !== 'undefined' && HTMLCanvasElement.prototype) {
      replaceMethod(HTMLCanvasElement.prototype, 'toDataURL', (originalToDataURL) => function toDataURL(...args) {
        try {
          const ctx = this.getContext('2d');
          if (ctx) {
            const image = ctx.getImageData(0, 0, Math.min(this.width, 64), Math.min(this.height, 64));
            canvasNoise(image.data, 'toDataURL');
            ctx.putImageData(image, 0, 0);
          }
        } catch (_) {}
        return originalToDataURL.apply(this, args);
      });
    }

    const webglPatch = (proto) => {
      if (!proto) return;
      replaceMethod(proto, 'getParameter', (originalGetParameter) => function getParameter(parameter) {
        if (parameter === 37445 && profile.webglVendor) return profile.webglVendor;
        if (parameter === 37446 && profile.webglRenderer) return profile.webglRenderer;
        return originalGetParameter.call(this, parameter);
      });
      replaceMethod(proto, 'getSupportedExtensions', (originalGetSupportedExtensions) => function getSupportedExtensions() {
        const actual = originalGetSupportedExtensions.call(this) || [];
        const allowed = new Set(profile.webglExtensions || []);
        if (!allowed.size) return actual;
        return actual.filter((name) => allowed.has(name));
      });
      replaceMethod(proto, 'getShaderPrecisionFormat', (originalGetShaderPrecisionFormat) => function getShaderPrecisionFormat(...args) {
        return originalGetShaderPrecisionFormat.apply(this, args);
      });
      replaceMethod(proto, 'readPixels', (originalReadPixels) => function readPixels(x, y, width, height, format, type, pixels, ...rest) {
        originalReadPixels.call(this, x, y, width, height, format, type, pixels, ...rest);
        if (pixels && pixels.length) canvasNoise(pixels, 'readPixels');
      });
    };
    webglPatch(typeof WebGLRenderingContext !== 'undefined' && WebGLRenderingContext.prototype);
    webglPatch(typeof WebGL2RenderingContext !== 'undefined' && WebGL2RenderingContext.prototype);
    if (typeof OffscreenCanvas !== 'undefined' && OffscreenCanvas.prototype && OffscreenCanvas.prototype.getContext) {
      replaceMethod(OffscreenCanvas.prototype, 'getContext', (originalGetContext) => function getContext(type, ...rest) {
        const ctx = originalGetContext.call(this, type, ...rest);
        if (ctx && (type === 'webgl' || type === 'webgl2' || type === 'experimental-webgl')) {
          webglPatch(Object.getPrototypeOf(ctx));
        }
        return ctx;
      });
    }

    if (typeof AnalyserNode !== 'undefined' && AnalyserNode.prototype && AnalyserNode.prototype.getFloatFrequencyData) {
      replaceMethod(AnalyserNode.prototype, 'getFloatFrequencyData', (originalGetFloatFrequencyData) => function getFloatFrequencyData(array) {
        originalGetFloatFrequencyData.call(this, array);
        const base = Number(profile.audioNoise || 0) || 0;
        for (let i = 0; i < array.length; i++) {
          array[i] = array[i] + base + stableNoise('audio-frequency:' + i) / 1000;
        }
      });
    }
    if (typeof AudioBuffer !== 'undefined' && AudioBuffer.prototype && AudioBuffer.prototype.copyFromChannel) {
      replaceMethod(AudioBuffer.prototype, 'copyFromChannel', (originalCopyFromChannel) => function copyFromChannel(destination, channelNumber, startInChannel) {
        originalCopyFromChannel.call(this, destination, channelNumber, startInChannel);
        const base = Number(profile.audioNoise || 0) || 0;
        for (let i = 0; i < destination.length; i++) {
          destination[i] = destination[i] + base / 1000 + stableNoise('audio-buffer:' + channelNumber + ':' + i) / 100000;
        }
      });
    }

    // docs/51 S3 — keep WebGPU absent or adapter-null when WebGL persona is present (avoid dual-stack mismatch).
    if (profile.webglVendor || profile.webglRenderer) {
      try {
        if (root.navigator && 'gpu' in root.navigator) {
          defineGetter(root.navigator, 'gpu', undefined);
        }
      } catch (_) {}
    }

    const nav = root.navigator;

    // docs/51 S4 — speechSynthesis voices match primary locale.
    if (typeof speechSynthesis !== 'undefined' && speechSynthesis.getVoices) {
      replaceMethod(speechSynthesis, 'getVoices', (originalGetVoices) => function getVoices() {
        const voices = originalGetVoices.call(this) || [];
        const lang = (profile.languages && profile.languages[0]) || 'zh-CN';
        const alt = lang.startsWith('zh') ? 'zh-CN' : 'en-US';
        const hasMatch = voices.some((v) => v && v.lang && (v.lang === lang || v.lang === alt || v.lang.startsWith(String(lang).split('-')[0])));
        if (hasMatch) return voices;
        const seedVoice = voices[0] || { name: 'Google US English', lang: 'en-US', default: true, localService: true, voiceURI: 'Google US English' };
        return voices.concat([Object.assign({}, seedVoice, { lang, name: lang + ' Voice', voiceURI: lang + ' Voice', default: false })]);
      });
    }

    // docs/51 S5 — desktop battery persona (always charging, full).
    if (nav && nav.getBattery) {
      replaceMethod(nav, 'getBattery', () => function getBattery() {
        const battery = { charging: true, level: 1, chargingTime: 0, dischargingTime: Infinity };
        battery.addEventListener = makeNative(function addEventListener() {});
        battery.removeEventListener = makeNative(function removeEventListener() {});
        battery.dispatchEvent = makeNative(function dispatchEvent() { return true; });
        return Promise.resolve(battery);
      });
    }

    // docs/51 S6 — NetworkInformation stable desktop values.
    if (nav && nav.connection) {
      const conn = nav.connection;
      defineGetter(conn, 'effectiveType', profile.networkEffectiveType || '4g');
      defineGetter(conn, 'rtt', profile.networkRtt > 0 ? profile.networkRtt : 50);
      defineGetter(conn, 'downlink', profile.networkDownlink > 0 ? profile.networkDownlink : 10);
    }

    // docs/51 S7 — matchMedia prefers-color-scheme / reduced-motion stability.
    if (typeof root.matchMedia === 'function') {
      replaceMethod(root, 'matchMedia', (originalMatchMedia) => function matchMedia(query) {
        const q = String(query || '').toLowerCase();
        const mql = originalMatchMedia.call(this, query);
        if (q.includes('prefers-color-scheme')) {
          const scheme = String(profile.prefersColorScheme || 'light').toLowerCase();
          return Object.assign({}, mql, { matches: q.includes(scheme) });
        }
        if (q.includes('prefers-reduced-motion')) {
          return Object.assign({}, mql, { matches: q.includes('no-preference') });
        }
        return mql;
      });
    }

    // docs/51 S8 — ClientRects subpixel noise (seed-stable).
    const rectNoise = (rect, salt) => {
      const n = stableNoise(salt);
      return {
        x: rect.x + n, y: rect.y + n, width: rect.width, height: rect.height,
        top: rect.top + n, right: rect.right + n, bottom: rect.bottom + n, left: rect.left + n,
        toJSON: rect.toJSON ? rect.toJSON.bind(rect) : undefined
      };
    };
    if (typeof Element !== 'undefined' && Element.prototype) {
      replaceMethod(Element.prototype, 'getBoundingClientRect', (originalGetBoundingClientRect) => function getBoundingClientRect() {
        return rectNoise(originalGetBoundingClientRect.call(this), 'getBoundingClientRect');
      });
      replaceMethod(Element.prototype, 'getClientRects', (originalGetClientRects) => function getClientRects() {
        const list = originalGetClientRects.call(this);
        const noisy = [];
        for (let i = 0; i < list.length; i++) noisy.push(rectNoise(list[i], 'getClientRects:' + i));
        noisy.item = (i) => noisy[i] || null;
        return noisy;
      });
    }

    // docs/51 S9 — userAgentData high-entropy brands from profile.
    if (nav && nav.userAgentData && nav.userAgentData.getHighEntropyValues) {
      replaceMethod(nav.userAgentData, 'getHighEntropyValues', (originalGetHighEntropyValues) => async function getHighEntropyValues(hints) {
        const values = await originalGetHighEntropyValues.call(this, hints);
        if (!profile.brandVersion) return values;
        const version = String(profile.brandVersion);
        const brands = [
          { brand: 'Google Chrome', version },
          { brand: 'Chromium', version },
          { brand: 'Not_A Brand', version: '24' }
        ];
        return Object.assign({}, values, { brands, fullVersionList: brands });
      });
    }

    // docs/51 S13 — screen / devicePixelRatio geometry from profile.
    if (profile.screenWidth > 0 && root.screen) {
      defineGetter(root.screen, 'width', profile.screenWidth);
      defineGetter(root.screen, 'height', profile.screenHeight);
      defineGetter(root.screen, 'availWidth', profile.screenWidth);
      defineGetter(root.screen, 'availHeight', profile.screenHeight > 40 ? profile.screenHeight - 40 : profile.screenHeight);
    }
    if (profile.devicePixelRatio) {
      const dpr = parseFloat(profile.devicePixelRatio);
      if (Number.isFinite(dpr) && dpr > 0) defineGetter(root, 'devicePixelRatio', dpr);
    }

    // docs/51 S14 — storage.estimate quota/usage persona.
    if (nav && nav.storage && nav.storage.estimate) {
      replaceMethod(nav.storage, 'estimate', () => async function estimate() {
        return { quota: 1e10, usage: Math.floor(stableNoise('storage-usage') * 1e8) };
      });
    }

    if (typeof document !== 'undefined' && document.fonts && profile.fontAllowlist && profile.fontAllowlist.length && typeof Document !== 'undefined') {
      const allowedFonts = Object.freeze(profile.fontAllowlist.slice());
      const originalFontCheck = document.fonts.check && document.fonts.check.bind(document.fonts);
      const fontCheck = makeNative(function check(query) {
        return allowedFonts.some((font) => String(query || '').includes(font));
      }, originalFontCheck);
      const emptyFontIterator = makeNative(function* values() { yield* []; }, document.fonts[Symbol.iterator]);
      defineGetter(Document.prototype, 'fonts', new Proxy(document.fonts, {
        get(target, prop) {
          if (prop === 'check') return fontCheck;
          if (prop === Symbol.iterator) return emptyFontIterator;
          return Reflect.get(target, prop);
        }
      }));
      root.__personaPilotAllowedFonts = allowedFonts;
      // docs/51 S12 — measureText width drift for fonts outside allowlist.
      if (typeof CanvasRenderingContext2D !== 'undefined' && CanvasRenderingContext2D.prototype) {
        replaceMethod(CanvasRenderingContext2D.prototype, 'measureText', (originalMeasureText) => function measureText(text) {
          const result = originalMeasureText.call(this, text);
          const font = String(this.font || '');
          if (!allowedFonts.some((f) => font.includes(f))) {
            return Object.assign({}, result, { width: result.width + stableNoise('measureText:' + String(text)) * 2 });
          }
          return result;
        });
      }
    }

    if (nav && nav.mediaDevices && nav.mediaDevices.enumerateDevices) {
      replaceMethod(nav.mediaDevices, 'enumerateDevices', () => async function enumerateDevices() {
        return (profile.mediaDevices || []).map((d) => ({
          kind: d.kind, label: d.label, groupId: d.groupId, deviceId: d.id
        }));
      });
      if (nav.mediaDevices.getUserMedia) {
        replaceMethod(nav.mediaDevices, 'getUserMedia', () => async function getUserMedia() {
          if (profile.mediaPermission === 'deny') throw new DOMException('Permission denied', 'NotAllowedError');
          return new MediaStream();
        });
      }
    }

    if (root.RTCPeerConnection) {
      const OriginalRTCPeerConnection = root.RTCPeerConnection;
      const shouldAllowCandidate = (line) => {
        const policy = profile.webRtcPolicy || {};
        const candidate = String(line || '');
        if (policy.mode === 'allow_all') return true;
        if (policy.mode === 'block_all') return false;
        const privatePatterns = [/ 10\\./, / 172\\.(1[6-9]|2\\d|3[01])\\./, / 192\\.168\\./, / 127\\./, / 169\\.254\\./, / typ host/];
        if (policy.mode === 'block_private_candidates') {
          for (const re of privatePatterns) {
            if (re.test(candidate)) return false;
          }
          const hosts = new Set(policy.allowHosts || []);
          if (hosts.size === 0) return true;
          return Array.from(hosts).some((host) => candidate.includes(host));
        }
        const hosts = new Set(policy.allowHosts || []);
        if (!hosts.size) return false;
        return Array.from(hosts).some((host) => candidate.includes(host));
      };
      const filterSDP = (sdp) => String(sdp || '').split('\n').filter((line) => !line.startsWith('a=candidate:') || shouldAllowCandidate(line)).join('\n');
      root.RTCPeerConnection = makeNative(function RTCPeerConnection(...args) {
        const pc = new OriginalRTCPeerConnection(...args);
        const originalCreateOffer = pc.createOffer.bind(pc);
        pc.createOffer = makeNative(async function createOffer(...offerArgs) {
          const offer = await originalCreateOffer(...offerArgs);
          return Object.assign({}, offer, { sdp: filterSDP(offer.sdp) });
        }, originalCreateOffer);
        const originalSetLocalDescription = pc.setLocalDescription.bind(pc);
        pc.setLocalDescription = makeNative(async function setLocalDescription(description) {
          return originalSetLocalDescription(description && description.sdp ? Object.assign({}, description, { sdp: filterSDP(description.sdp) }) : description);
        }, originalSetLocalDescription);
        pc.addEventListener('icecandidate', (event) => {
          if (event.candidate && !shouldAllowCandidate(event.candidate.candidate)) event.stopImmediatePropagation();
        }, true);
        return pc;
      }, OriginalRTCPeerConnection);
    }
  }

  installEnvironment(profile);

  // Main-document only: wrap Worker/SharedWorker so classic workers inherit the same hooks.
  if (typeof document !== 'undefined') {
    const bootstrap = '(' + installEnvironment.toString() + ')(' + JSON.stringify(profile) + ');';
    const wrapClassicWorker = (NativeCtor, name) => {
      if (typeof NativeCtor !== 'function') return;
      const Wrapped = function WorkerProxy(scriptURL, options) {
        try {
          if (options && options.type === 'module') {
            const code = bootstrap + '\nimport ' + JSON.stringify(String(scriptURL)) + ';';
            const blobURL = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
            return new NativeCtor(blobURL, options);
          }
          const code = bootstrap + '\nimportScripts(' + JSON.stringify(String(scriptURL)) + ');';
          const blobURL = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
          return new NativeCtor(blobURL, options);
        } catch (_) {
          return new NativeCtor(scriptURL, options);
        }
      };
      try { Object.defineProperty(Wrapped, 'name', { value: name }); } catch (_) {}
      try { Wrapped.prototype = NativeCtor.prototype; } catch (_) {}
      globalThis[name] = Wrapped;
    };
    if (typeof Worker !== 'undefined') wrapClassicWorker(Worker, 'Worker');
    if (typeof SharedWorker !== 'undefined') wrapClassicWorker(SharedWorker, 'SharedWorker');
    if (typeof navigator !== 'undefined' && navigator.serviceWorker && navigator.serviceWorker.register) {
      const originalRegister = navigator.serviceWorker.register.bind(navigator.serviceWorker);
      navigator.serviceWorker.register = async function register(scriptURL, options) {
        try {
          if (typeof scriptURL === 'string') {
            const code = bootstrap + '\nimportScripts(' + JSON.stringify(scriptURL) + ');';
            const blobURL = URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
            return await originalRegister(blobURL, options);
          }
        } catch (_) {}
        return originalRegister(scriptURL, options);
      };
    }
  }
})();`, encoded))

	return EnvironmentInjectionPlan{
		Script:          script,
		HeaderOverrides: normalized.Headers,
		AppliedFamilies: []string{"browser_api_surface", "canvas_rendering", "timezone_locale", "webgl_gpu", "audio_stack", "speech_battery_network", "client_rects_match_media", "fonts_text_metrics", "screen_geometry", "storage_estimate", "media_devices", "webrtc_ip_leak", "worker_scope", "service_worker_scope"},
		Warnings:        warnings,
	}, nil
}

// SetGeolocationOverride applies CDP Emulation.setGeolocationOverride (docs/52 DP3).
func (e *CDPExecutor) SetGeolocationOverride(lat, lon, accuracy float64) error {
	if e == nil {
		return fmt.Errorf("cdp executor nil")
	}
	if accuracy <= 0 {
		accuracy = 100
	}
	_, err := e.sendCommand("Emulation.setGeolocationOverride", map[string]interface{}{
		"latitude":  lat,
		"longitude": lon,
		"accuracy":  accuracy,
	})
	if err != nil {
		return fmt.Errorf("set geolocation override: %w", err)
	}
	return nil
}

func (e *CDPExecutor) ApplyEnvironmentInjection(profile EnvironmentInjectionProfile) (EnvironmentInjectionPlan, error) {
	plan, err := CompileEnvironmentInjectionScript(profile)
	if err != nil {
		return plan, err
	}
	if _, err := e.sendCommand("Page.enable", map[string]interface{}{}); err != nil {
		return plan, fmt.Errorf("enable page domain for environment injection: %w", err)
	}
	if _, err := e.sendCommand("Runtime.enable", map[string]interface{}{}); err != nil {
		return plan, fmt.Errorf("enable runtime domain for environment injection: %w", err)
	}
	if len(plan.HeaderOverrides) > 0 {
		if _, err := e.sendCommand("Network.enable", map[string]interface{}{}); err != nil {
			return plan, fmt.Errorf("enable network domain for environment headers: %w", err)
		}
		if _, err := e.sendCommand("Network.setExtraHTTPHeaders", map[string]interface{}{"headers": plan.HeaderOverrides}); err != nil {
			return plan, fmt.Errorf("set environment headers: %w", err)
		}
	}
	if _, err := e.sendCommand("Page.addScriptToEvaluateOnNewDocument", map[string]interface{}{"source": plan.Script}); err != nil {
		return plan, fmt.Errorf("inject environment script: %w", err)
	}
	// Apply the same hook to the already-open page when it is still blank. Real http(s)
	// documents are covered by addScriptToEvaluateOnNewDocument on the next navigation.
	patchCurrent := true
	if currentURL, urlErr := e.currentDocumentURL(); urlErr == nil {
		patchCurrent = shouldPatchCurrentDocument(currentURL)
	}
	if patchCurrent {
		if _, err := e.sendCommand("Runtime.evaluate", map[string]interface{}{
			"expression":    plan.Script,
			"awaitPromise":  true,
			"returnByValue": true,
		}); err != nil {
			plan.Warnings = append(plan.Warnings, "current_page_environment_patch_failed")
		}
	} else {
		plan.Warnings = append(plan.Warnings, "current_page_environment_patch_deferred_to_navigation")
	}
	// A2/A4: drop long-lived Runtime domain after one-shot evaluate (CDP-minimal).
	if _, err := e.sendCommand("Runtime.disable", map[string]interface{}{}); err != nil {
		plan.Warnings = append(plan.Warnings, "runtime_disable_after_injection_failed")
	}
	return plan, nil
}

func shouldPatchCurrentDocument(url string) bool {
	u := strings.TrimSpace(url)
	if u == "" || strings.EqualFold(u, "about:blank") {
		return true
	}
	lower := strings.ToLower(u)
	if strings.HasPrefix(lower, "chrome://") || strings.HasPrefix(lower, "chrome-extension://") {
		return true
	}
	if strings.HasPrefix(lower, "http://") || strings.HasPrefix(lower, "https://") {
		return false
	}
	return true
}

func (e *CDPExecutor) currentDocumentURL() (string, error) {
	if e == nil {
		return "", fmt.Errorf("cdp executor not connected")
	}
	return e.EvaluateJSString(`location.href`)
}

func normalizeEnvironmentProfile(profile EnvironmentInjectionProfile) EnvironmentInjectionProfile {
	if strings.TrimSpace(profile.Seed) == "" {
		profile.Seed = "personal-pilot"
	}
	if len(profile.Languages) == 0 && strings.TrimSpace(profile.AcceptLanguage) != "" {
		profile.Languages = []string{strings.TrimSpace(strings.Split(profile.AcceptLanguage, ",")[0])}
	}
	if len(profile.Languages) == 0 {
		profile.Languages = []string{"en-US", "en"}
	}
	if profile.AcceptLanguage == "" {
		profile.AcceptLanguage = profile.Languages[0]
		if len(profile.Languages) > 1 {
			profile.AcceptLanguage += "," + profile.Languages[1] + ";q=0.9"
		}
	}
	if profile.Headers == nil {
		profile.Headers = map[string]string{}
	}
	profile.Headers["Accept-Language"] = profile.AcceptLanguage
	if profile.HardwareConcurrency < 0 {
		profile.HardwareConcurrency = 0
	}
	if profile.DeviceMemory < 0 {
		profile.DeviceMemory = 0
	}
	sort.Strings(profile.FontAllowlist)
	sort.Strings(profile.WebGLExtensions)
	sort.Strings(profile.WebRTCPolicy.AllowHosts)
	if profile.WebRTCPolicy.Mode == "" {
		profile.WebRTCPolicy.Mode = "block_private_candidates"
	}
	if profile.MediaPermission == "" {
		profile.MediaPermission = "prompt"
	}
	if len(profile.Plugins) == 0 {
		profile.Plugins = defaultChromePDFPlugins()
	}
	if profile.ScreenWidth == 0 && strings.TrimSpace(profile.WindowSize) != "" {
		parts := strings.Split(profile.WindowSize, ",")
		if len(parts) == 2 {
			profile.ScreenWidth = parseEnvPositiveInt(parts[0])
			profile.ScreenHeight = parseEnvPositiveInt(parts[1])
		}
	}
	if profile.PrefersColorScheme == "" {
		profile.PrefersColorScheme = "light"
	}
	if profile.NetworkEffectiveType == "" {
		profile.NetworkEffectiveType = "4g"
	}
	if profile.NetworkRTT <= 0 {
		profile.NetworkRTT = 50
	}
	if profile.NetworkDownlink <= 0 {
		profile.NetworkDownlink = 10
	}
	return profile
}

func parseEnvPositiveInt(value string) int {
	n, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || n < 0 {
		return 0
	}
	return n
}

func defaultChromePDFPlugins() []PluginInjection {
	return []PluginInjection{
		{Name: "PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Chrome PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Chromium PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "Microsoft Edge PDF Viewer", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
		{Name: "WebKit built-in PDF", Filename: "internal-pdf-viewer", Description: "Portable Document Format", MimeType: "application/pdf"},
	}
}

func environmentInjectionWarnings(profile EnvironmentInjectionProfile) []string {
	var warnings []string
	if profile.Timezone == "" {
		warnings = append(warnings, "timezone_not_configured")
	}
	if profile.Platform == "" || profile.UserAgent == "" || profile.Vendor == "" {
		warnings = append(warnings, "user_agent_platform_vendor_chain_incomplete")
	}
	if profile.WebGLVendor == "" || profile.WebGLRenderer == "" {
		warnings = append(warnings, "webgl_identity_incomplete")
	}
	return warnings
}

func stableProfileSeed(text string) uint64 {
	h := fnv.New64a()
	_, _ = h.Write([]byte(text))
	return h.Sum64()
}
