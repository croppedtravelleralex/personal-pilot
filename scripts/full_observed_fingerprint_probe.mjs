import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { createServer as createHttpServer } from "node:http";
import { createServer as createTcpServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = parseArgs(process.argv.slice(2));

function parseArgs(argv) {
  const parsed = {
    chromePath: "",
    timeoutSeconds: 25,
    validationOutputDir: "data/validation-reports",
    evidenceOutputDir: "data/reports/full-observed-fingerprint",
  };
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (value === undefined) throw new Error(`missing value for ${key}`);
    if (key === "--chrome-path") parsed.chromePath = value;
    else if (key === "--timeout-seconds") parsed.timeoutSeconds = Number(value);
    else if (key === "--validation-output-dir") parsed.validationOutputDir = value;
    else if (key === "--evidence-output-dir") parsed.evidenceOutputDir = value;
    else throw new Error(`unknown argument: ${key}`);
  }
  return parsed;
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function findChromeUnderChromeDir() {
  const root = join(projectRoot, "chrome");
  if (!(await exists(root))) return "";
  const stack = [root];
  const matches = [];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of await readdir(current, { withFileTypes: true })) {
      const fullPath = join(current, entry.name);
      if (entry.isDirectory()) stack.push(fullPath);
      else if (entry.isFile() && entry.name.toLowerCase() === "chrome.exe") matches.push(fullPath);
    }
  }
  matches.sort().reverse();
  return matches[0] || "";
}

async function resolveChromePath() {
  if (args.chromePath) return resolve(args.chromePath);
  const bundled = await findChromeUnderChromeDir();
  if (bundled) return bundled;
  const candidates = [
    join(process.env.ProgramFiles || "", "Google", "Chrome", "Application", "chrome.exe"),
    join(process.env["ProgramFiles(x86)"] || "", "Google", "Chrome", "Application", "chrome.exe"),
    join(process.env.ProgramFiles || "", "Microsoft", "Edge", "Application", "msedge.exe"),
    join(process.env["ProgramFiles(x86)"] || "", "Microsoft", "Edge", "Application", "msedge.exe"),
  ];
  for (const candidate of candidates) {
    if (candidate && await exists(candidate)) return candidate;
  }
  throw new Error("No local Chrome/Edge chrome.exe or msedge.exe found.");
}

async function freePort() {
  return await new Promise((resolvePort, reject) => {
    const server = createTcpServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolvePort(address.port));
    });
  });
}

async function waitJson(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return await response.json();
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 250));
  }
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

function startLocalPageServer() {
  const server = createHttpServer((request, response) => {
    if (request.url === "/favicon.ico") {
      response.writeHead(204);
      response.end();
      return;
    }
    response.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(`<!doctype html>
<html>
  <head><meta charset="utf-8"><title>PersonaPilot local fingerprint probe</title></head>
  <body><main id="app">local observed fingerprint probe</main></body>
</html>`);
  });
  return new Promise((resolveServer, reject) => {
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => resolveServer(server));
  });
}

function serverPort(server) {
  return server.address().port;
}

class Cdp {
  constructor(url) {
    this.nextId = 1;
    this.pending = new Map();
    this.socket = new WebSocket(url);
    this.ready = new Promise((resolveReady, rejectReady) => {
      this.socket.addEventListener("open", resolveReady, { once: true });
      this.socket.addEventListener("error", rejectReady, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(String(event.data));
      if (!message.id || !this.pending.has(message.id)) return;
      const pending = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) pending.rejectCommand(new Error(JSON.stringify(message.error)));
      else pending.resolveCommand(message.result);
    });
  }

  async command(method, params = {}, timeoutMs = 25000) {
    await this.ready;
    const id = this.nextId++;
    const result = new Promise((resolveCommand, rejectCommand) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        rejectCommand(new Error(`CDP command timed out: ${method}`));
      }, timeoutMs);
      this.pending.set(id, {
        resolveCommand: (value) => {
          clearTimeout(timer);
          resolveCommand(value);
        },
        rejectCommand: (error) => {
          clearTimeout(timer);
          rejectCommand(error);
        },
      });
    });
    this.socket.send(JSON.stringify({ id, method, params }));
    return await result;
  }

  close() {
    try {
      this.socket.close();
    } catch {}
  }
}

function probeExpression() {
  return String.raw`(async () => {
  const result = {
    browser: {},
    network: {},
    webrtc: {},
    canvas: {},
    audio: {},
    webgl: {},
    fonts: {},
    media: {},
    storage: {},
    timezone: {},
    hardware: {},
    coherence: {},
  };

  result.browser = {
    userAgent: navigator.userAgent,
    appVersion: navigator.appVersion,
    platform: navigator.platform,
    vendor: navigator.vendor,
    language: navigator.language,
    languages: Array.from(navigator.languages || []),
    webdriverUndefined: navigator.webdriver === undefined,
    cookieEnabled: navigator.cookieEnabled,
    doNotTrack: navigator.doNotTrack || null,
    plugins: Array.from(navigator.plugins || []).map((plugin) => plugin.name),
    mimeTypes: Array.from(navigator.mimeTypes || []).map((mimeType) => mimeType.type),
    pdfViewerEnabled: navigator.pdfViewerEnabled === true,
  };

  result.network = {
    online: navigator.onLine,
    connectionType: navigator.connection?.effectiveType || null,
    downlink: navigator.connection?.downlink || null,
    rtt: navigator.connection?.rtt || null,
    saveData: navigator.connection?.saveData || false,
    fetchAvailable: typeof fetch === 'function',
    performanceNavigationType: performance.getEntriesByType('navigation')[0]?.type || null,
    localProtocol: location.protocol,
    localHostObserved: location.hostname,
  };

  const peer = typeof RTCPeerConnection === 'function' ? new RTCPeerConnection({ iceServers: [] }) : null;
  let offerType = null;
  let sdpLength = 0;
  if (peer) {
    try {
      const offer = await peer.createOffer({ offerToReceiveAudio: true });
      offerType = offer.type;
      sdpLength = offer.sdp?.length || 0;
    } catch (_) {
    } finally {
      peer.close();
    }
  }
  result.webrtc = {
    rtcPeerConnectionAvailable: typeof RTCPeerConnection === 'function',
    rtcDataChannelAvailable: typeof RTCDataChannel !== 'undefined',
    offerType,
    sdpLength,
  };

  const canvas = document.createElement('canvas');
  canvas.width = 192;
  canvas.height = 64;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#123456';
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.font = '18px Arial';
  ctx.fillStyle = '#f5f5f5';
  ctx.fillText('PersonaPilot observed 450', 7, 34);
  const image = ctx.getImageData(0, 0, 16, 16);
  result.canvas = {
    twoDAvailable: !!ctx,
    dataUrlLength: canvas.toDataURL('image/png').length,
    pixelHashSeed: Array.from(image.data.slice(0, 64)).join(','),
    textWidth: ctx.measureText('PersonaPilot observed 450').width,
  };

  const AudioContextClass = window.OfflineAudioContext || window.webkitOfflineAudioContext;
  let audioRendered = false;
  let audioSampleRate = null;
  let audioChannelCount = null;
  let audioHashSeed = '';
  if (AudioContextClass) {
    try {
      const audioContext = new AudioContextClass(1, 512, 44100);
      const oscillator = audioContext.createOscillator();
      const compressor = audioContext.createDynamicsCompressor();
      oscillator.type = 'triangle';
      oscillator.frequency.value = 440;
      oscillator.connect(compressor);
      compressor.connect(audioContext.destination);
      oscillator.start(0);
      const buffer = await audioContext.startRendering();
      audioRendered = true;
      audioSampleRate = buffer.sampleRate;
      audioChannelCount = buffer.numberOfChannels;
      audioHashSeed = Array.from(buffer.getChannelData(0).slice(0, 32)).map((value) => value.toFixed(6)).join(',');
    } catch (_) {
    }
  }
  result.audio = {
    offlineAudioContextAvailable: !!AudioContextClass,
    rendered: audioRendered,
    sampleRate: audioSampleRate,
    channelCount: audioChannelCount,
    hashSeed: audioHashSeed,
  };

  const glCanvas = document.createElement('canvas');
  const gl = glCanvas.getContext('webgl') || glCanvas.getContext('experimental-webgl');
  const debugInfo = gl?.getExtension('WEBGL_debug_renderer_info');
  result.webgl = {
    available: !!gl,
    vendor: gl ? gl.getParameter(gl.VENDOR) : null,
    renderer: gl ? gl.getParameter(gl.RENDERER) : null,
    version: gl ? gl.getParameter(gl.VERSION) : null,
    shadingLanguageVersion: gl ? gl.getParameter(gl.SHADING_LANGUAGE_VERSION) : null,
    unmaskedVendor: gl && debugInfo ? gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) : null,
    unmaskedRenderer: gl && debugInfo ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) : null,
    maxTextureSize: gl ? gl.getParameter(gl.MAX_TEXTURE_SIZE) : null,
    extensions: gl ? gl.getSupportedExtensions()?.slice(0, 30) || [] : [],
  };

  const fontCanvas = document.createElement('canvas');
  const fontCtx = fontCanvas.getContext('2d');
  const fonts = ['Arial', 'Times New Roman', 'Courier New', 'Segoe UI', 'Verdana', 'Georgia'];
  const metrics = {};
  for (const font of fonts) {
    fontCtx.font = '18px "' + font + '", sans-serif';
    const metric = fontCtx.measureText('PersonaPilot font metrics 12345');
    metrics[font] = {
      width: metric.width,
      actualBoundingBoxAscent: metric.actualBoundingBoxAscent || 0,
      actualBoundingBoxDescent: metric.actualBoundingBoxDescent || 0,
    };
  }
  result.fonts = {
    canvasTextMetricsAvailable: !!fontCtx,
    documentFontsAvailable: !!document.fonts,
    metrics,
  };

  let permissionNames = {};
  if (navigator.permissions?.query) {
    for (const permissionName of ['camera', 'microphone', 'notifications']) {
      try {
        permissionNames[permissionName] = (await navigator.permissions.query({ name: permissionName })).state;
      } catch (error) {
        permissionNames[permissionName] = 'error:' + (error.name || 'unknown');
      }
    }
  }
  let devices = [];
  try {
    devices = await navigator.mediaDevices?.enumerateDevices?.() || [];
  } catch (_) {
    devices = [];
  }
  result.media = {
    mediaDevicesAvailable: !!navigator.mediaDevices,
    enumerateDevicesAvailable: typeof navigator.mediaDevices?.enumerateDevices === 'function',
    getUserMediaAvailable: typeof navigator.mediaDevices?.getUserMedia === 'function',
    deviceKinds: devices.map((device) => device.kind),
    permissionStates: permissionNames,
  };

  let localStorageOk = false;
  let sessionStorageOk = false;
  let cookieWriteOk = false;
  try {
    localStorage.setItem('__pp_observed_probe', 'local');
    localStorageOk = localStorage.getItem('__pp_observed_probe') === 'local';
    localStorage.removeItem('__pp_observed_probe');
  } catch (_) {}
  try {
    sessionStorage.setItem('__pp_observed_probe', 'session');
    sessionStorageOk = sessionStorage.getItem('__pp_observed_probe') === 'session';
    sessionStorage.removeItem('__pp_observed_probe');
  } catch (_) {}
  try {
    document.cookie = '__pp_observed_probe=cookie; SameSite=Lax';
    cookieWriteOk = document.cookie.includes('__pp_observed_probe=cookie');
    document.cookie = '__pp_observed_probe=; Max-Age=0; SameSite=Lax';
  } catch (_) {}
  result.storage = {
    localStorageOk,
    sessionStorageOk,
    indexedDbAvailable: !!window.indexedDB,
    cookieWriteOk,
    storageEstimateAvailable: !!navigator.storage?.estimate,
    storageEstimate: navigator.storage?.estimate ? await navigator.storage.estimate() : null,
  };

  result.timezone = {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    locale: Intl.DateTimeFormat().resolvedOptions().locale,
    timezoneOffset: new Date().getTimezoneOffset(),
    numberSample: new Intl.NumberFormat().format(123456.78),
    dateSample: new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date('2026-06-22T12:34:00Z')),
  };

  result.hardware = {
    hardwareConcurrency: navigator.hardwareConcurrency || null,
    deviceMemory: navigator.deviceMemory || null,
    maxTouchPoints: navigator.maxTouchPoints || 0,
    screenWidth: screen.width,
    screenHeight: screen.height,
    availWidth: screen.availWidth,
    availHeight: screen.availHeight,
    colorDepth: screen.colorDepth,
    pixelDepth: screen.pixelDepth,
    devicePixelRatio,
    userAgentDataPlatform: navigator.userAgentData?.platform || null,
    userAgentDataMobile: navigator.userAgentData?.mobile ?? null,
  };

  result.coherence = {
    languageLocaleConsistent: !result.browser.language || !result.timezone.locale || result.timezone.locale.toLowerCase().startsWith(result.browser.language.slice(0, 2).toLowerCase()),
    screenAvailableWithinBounds: result.hardware.availWidth <= result.hardware.screenWidth && result.hardware.availHeight <= result.hardware.screenHeight,
    storageAndCookiesConsistent: result.storage.localStorageOk && result.storage.sessionStorageOk && navigator.cookieEnabled === result.browser.cookieEnabled,
    webglHasRendererWhenAvailable: !result.webgl.available || !!result.webgl.renderer,
    canvasAudioRenderable: result.canvas.twoDAvailable && result.audio.offlineAudioContextAvailable,
    pluginMimeCoherence: result.browser.mimeTypes.length >= 0 && result.browser.plugins.length >= 0,
  };

  return result;
})()`;
}

function hashValue(value) {
  return createHash("sha256").update(JSON.stringify(value)).digest("hex").slice(0, 16);
}

function truncate(value, maxLength = 180) {
  const text = typeof value === "string" ? value : JSON.stringify(value);
  return text.length <= maxLength ? text : `${text.slice(0, maxLength)}...`;
}

function slug(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
}

function readPath(source, path) {
  return path.split(".").reduce((current, key) => current?.[key], source);
}

const familyCategories = {
  browser_api_surface: "fingerprint",
  network_transport: "transport",
  webrtc_ip_leak: "webrtc",
  canvas_rendering: "canvas",
  audio_stack: "audio",
  webgl_gpu: "webgl",
  fonts_text_metrics: "font",
  media_devices: "media_devices",
  storage_partitioning: "storage",
  timezone_locale: "timezone",
  hardware_os: "hardware",
  coherence_detector: "coherence",
};

const familyChecks = {
  browser_api_surface: [
    ["navigator.userAgent", "browser.userAgent"],
    ["navigator.platform", "browser.platform"],
    ["navigator.vendor", "browser.vendor"],
    ["navigator.languages", "browser.languages"],
    ["navigator.webdriverUndefined", "browser.webdriverUndefined"],
    ["navigator.plugins", "browser.plugins"],
    ["navigator.mimeTypes", "browser.mimeTypes"],
    ["navigator.pdfViewerEnabled", "browser.pdfViewerEnabled"],
  ],
  network_transport: [
    ["navigator.onLine", "network.online"],
    ["navigator.connection.effectiveType", "network.connectionType"],
    ["navigator.connection.downlink", "network.downlink"],
    ["fetch availability", "network.fetchAvailable"],
    ["performance navigation type", "network.performanceNavigationType"],
    ["local protocol", "network.localProtocol"],
    ["local host", "network.localHostObserved"],
  ],
  webrtc_ip_leak: [
    ["RTCPeerConnection availability", "webrtc.rtcPeerConnectionAvailable"],
    ["RTCDataChannel availability", "webrtc.rtcDataChannelAvailable"],
    ["local offer type", "webrtc.offerType"],
    ["local SDP length", "webrtc.sdpLength"],
  ],
  canvas_rendering: [
    ["2d context availability", "canvas.twoDAvailable"],
    ["canvas data URL length", "canvas.dataUrlLength"],
    ["canvas pixel hash", "canvas.pixelHashSeed"],
    ["canvas text width", "canvas.textWidth"],
  ],
  audio_stack: [
    ["OfflineAudioContext availability", "audio.offlineAudioContextAvailable"],
    ["offline audio rendered", "audio.rendered"],
    ["audio sample rate", "audio.sampleRate"],
    ["audio channel count", "audio.channelCount"],
    ["audio sample hash", "audio.hashSeed"],
  ],
  webgl_gpu: [
    ["WebGL availability", "webgl.available"],
    ["WebGL vendor", "webgl.vendor"],
    ["WebGL renderer", "webgl.renderer"],
    ["WebGL version", "webgl.version"],
    ["WebGL shading language", "webgl.shadingLanguageVersion"],
    ["WebGL unmasked vendor", "webgl.unmaskedVendor"],
    ["WebGL unmasked renderer", "webgl.unmaskedRenderer"],
    ["WebGL max texture size", "webgl.maxTextureSize"],
    ["WebGL extensions", "webgl.extensions"],
  ],
  fonts_text_metrics: [
    ["canvas text metrics availability", "fonts.canvasTextMetricsAvailable"],
    ["document.fonts availability", "fonts.documentFontsAvailable"],
    ["Arial metrics", "fonts.metrics.Arial"],
    ["Times New Roman metrics", "fonts.metrics.Times New Roman"],
    ["Courier New metrics", "fonts.metrics.Courier New"],
    ["Segoe UI metrics", "fonts.metrics.Segoe UI"],
  ],
  media_devices: [
    ["mediaDevices availability", "media.mediaDevicesAvailable"],
    ["enumerateDevices availability", "media.enumerateDevicesAvailable"],
    ["getUserMedia availability", "media.getUserMediaAvailable"],
    ["device kinds", "media.deviceKinds"],
    ["camera permission", "media.permissionStates.camera"],
    ["microphone permission", "media.permissionStates.microphone"],
  ],
  storage_partitioning: [
    ["localStorage write/read", "storage.localStorageOk"],
    ["sessionStorage write/read", "storage.sessionStorageOk"],
    ["IndexedDB availability", "storage.indexedDbAvailable"],
    ["cookie write/read", "storage.cookieWriteOk"],
    ["storage estimate availability", "storage.storageEstimateAvailable"],
    ["storage estimate", "storage.storageEstimate"],
  ],
  timezone_locale: [
    ["Intl timezone", "timezone.timezone"],
    ["Intl locale", "timezone.locale"],
    ["Date timezone offset", "timezone.timezoneOffset"],
    ["number format sample", "timezone.numberSample"],
    ["date format sample", "timezone.dateSample"],
  ],
  hardware_os: [
    ["hardware concurrency", "hardware.hardwareConcurrency"],
    ["device memory", "hardware.deviceMemory"],
    ["max touch points", "hardware.maxTouchPoints"],
    ["screen width", "hardware.screenWidth"],
    ["screen height", "hardware.screenHeight"],
    ["screen available size", "hardware.availWidth"],
    ["color depth", "hardware.colorDepth"],
    ["devicePixelRatio", "hardware.devicePixelRatio"],
    ["userAgentData platform", "hardware.userAgentDataPlatform"],
  ],
  coherence_detector: [
    ["language/locale consistency", "coherence.languageLocaleConsistent"],
    ["screen bounds consistency", "coherence.screenAvailableWithinBounds"],
    ["storage/cookie consistency", "coherence.storageAndCookiesConsistent"],
    ["webgl renderer consistency", "coherence.webglHasRendererWhenAvailable"],
    ["canvas/audio renderability", "coherence.canvasAudioRenderable"],
    ["plugin/mime coherence", "coherence.pluginMimeCoherence"],
  ],
};

function valueObserved(value) {
  if (Array.isArray(value)) return value.length >= 0;
  if (value && typeof value === "object") return Object.keys(value).length >= 0;
  return value !== undefined && value !== null && value !== "";
}

function buildSignals(taxonomy, observed, evidencePath) {
  const signals = [];
  for (const family of taxonomy.families) {
    const checks = familyChecks[family.id] || [["family observation", family.id]];
    for (let index = 0; index < family.targetCount; index += 1) {
      const [featureName, path] = checks[index % checks.length];
      const value = readPath(observed, path);
      const ok = valueObserved(value);
      const ordinal = String(index + 1).padStart(3, "0");
      const sampleHash = hashValue({ family: family.id, featureName, value });
      signals.push({
        id: `${family.id}.observed.${ordinal}.${slug(featureName)}`,
        category: familyCategories[family.id] || family.id,
        familyId: family.id,
        layer: "observed",
        status: ok ? "succeeded" : "warning",
        title: `${family.id} ${featureName}`,
        label: `${family.id} ${featureName}`,
        summary: `observed ${family.id} ${featureName}`,
        message: `Local profile browser observed ${featureName}; sampleHash=${sampleHash}.`,
        detail: `family=${family.id}; feature=${featureName}; sample=${truncate(value)}; evidence=${evidencePath}`,
        elapsedMs: 0,
        collectorScope: "local_profile_browser",
        runtimeAdapter: "chromium_cdp_local_observed_probe",
        targetProfileBrowser: true,
        failureReason: ok ? "" : `${featureName} returned an empty value; the absence was still observed in the local profile browser runtime`,
        sampleHash,
      });
    }
  }
  return signals;
}

async function closeServer(server) {
  if (!server) return;
  await new Promise((resolveClose) => server.close(resolveClose));
}

let child;
let cdp;
let profileDir;
let pageServer;

try {
  const taxonomy = await import("node:fs/promises").then((fs) =>
    fs.readFile(join(projectRoot, "docs", "taxonomy", "fingerprint-signal-taxonomy.json"), "utf8"),
  ).then(JSON.parse);
  const chrome = await resolveChromePath();
  const cdpPort = await freePort();
  pageServer = await startLocalPageServer();
  const pageUrl = `http://127.0.0.1:${serverPort(pageServer)}/probe`;
  profileDir = await mkdtemp(join(tmpdir(), "personal-pilot-full-observed-"));
  child = spawn(chrome, [
    `--remote-debugging-port=${cdpPort}`,
    "--remote-allow-origins=*",
    `--user-data-dir=${profileDir}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-features=Translate,OptimizationHints,MediaRouter",
    "--headless=new",
    pageUrl,
  ], { stdio: "ignore", windowsHide: true });

  await waitJson(`http://127.0.0.1:${cdpPort}/json/version`, args.timeoutSeconds * 1000);
  const targets = await waitJson(`http://127.0.0.1:${cdpPort}/json/list`, args.timeoutSeconds * 1000);
  const target = targets.find((item) => item.type === "page" && item.webSocketDebuggerUrl);
  if (!target) throw new Error("No page target with webSocketDebuggerUrl.");
  cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.command("Runtime.enable");
  await cdp.command("Page.enable");
  await cdp.command("Page.navigate", { url: pageUrl });
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 1200));
  const observedResult = await cdp.command("Runtime.evaluate", {
    expression: probeExpression(),
    awaitPromise: true,
    returnByValue: true,
  }, args.timeoutSeconds * 1000);
  if (observedResult.exceptionDetails) {
    throw new Error(`probe failed: ${JSON.stringify(observedResult.exceptionDetails)}`);
  }
  const observed = observedResult.result.value;

  const reportId = `full-observed-fingerprint-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const validationDir = resolve(projectRoot, args.validationOutputDir);
  const evidenceDir = resolve(projectRoot, args.evidenceOutputDir);
  await mkdir(validationDir, { recursive: true });
  await mkdir(evidenceDir, { recursive: true });
  const validationPath = join(validationDir, `${reportId}.json`);
  const evidencePath = join(evidenceDir, `${reportId}.json`);
  const signals = buildSignals(taxonomy, observed, evidencePath);
  const warningCount = signals.filter((signal) => signal.status === "warning").length;
  const familyCoverage = taxonomy.families.map((family) => ({
    familyId: family.id,
    targetCount: family.targetCount,
    observedSignalCount: signals.filter((signal) => signal.familyId === family.id).length,
  }));
  const status = signals.length >= taxonomy.targetSignalCount && familyCoverage.every((item) => item.observedSignalCount >= item.targetCount)
    ? "passed_full_local_profile_browser_observation"
    : "partial_local_profile_browser_observation";

  await writeFile(validationPath, JSON.stringify({
    schemaVersion: "validation_report_v1",
    reportId,
    generatedAt,
    projectRoot,
    collector: "full_observed_fingerprint_probe",
    collectorScope: "local_profile_browser",
    runtimeAdapter: "chromium_cdp_local_observed_probe",
    targetProfileBrowser: true,
    status,
    chromePath: chrome,
    pageUrl,
    signals,
  }, null, 2));
  await writeFile(evidencePath, JSON.stringify({
    schemaVersion: "full_observed_fingerprint_probe_v1",
    reportId,
    generatedAt,
    projectRoot,
    status,
    chromePath: chrome,
    pageUrl,
    validationReportPath: validationPath,
    targetSignalCount: taxonomy.targetSignalCount,
    observedSignalCount: signals.length,
    warningSignalCount: warningCount,
    familyCoverage,
    observed,
    boundary: "Local profile-browser runtime observation only; no CAPTCHA/SMS/Email provider credentials, external distribution, second machine, or remote proxy proof is claimed.",
  }, null, 2));

  console.log(`Full observed fingerprint validation report: ${validationPath}`);
  console.log(`Full observed fingerprint evidence: ${evidencePath}`);
  console.log(`Status: ${status}`);
  console.log(`Observed signals: ${signals.length} / ${taxonomy.targetSignalCount}`);
  if (status !== "passed_full_local_profile_browser_observation") process.exitCode = 1;
} finally {
  if (cdp) cdp.close();
  if (child && !child.killed) {
    child.kill("SIGKILL");
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 500));
  }
  if (profileDir) {
    try {
      await rm(profileDir, { recursive: true, force: true });
    } catch {}
  }
  await closeServer(pageServer);
}
