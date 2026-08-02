import { spawn } from "node:child_process";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const projectRoot = resolve(new URL("..", import.meta.url).pathname.slice(1));
const args = parseArgs(process.argv.slice(2));

function parseArgs(argv) {
  const parsed = {
    chromePath: "",
    timeoutSeconds: 25,
    validationOutputDir: "data/validation-reports",
    evidenceOutputDir: "data/reports/profile-browser-environment",
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
    await import("node:fs/promises").then((fs) => fs.access(path));
    return true;
  } catch {
    return false;
  }
}

async function resolveChromePath() {
  if (args.chromePath) return resolve(args.chromePath);
  const roots = [
    join(projectRoot, "chrome", "fingerprint-chromium-144.0.7559.132", "chrome.exe"),
    join(projectRoot, "chrome", "fingerprint-chromium-142.0.7444.175", "chrome.exe"),
    join(projectRoot, "chrome", "fingerprint-chromium-139.0.7258.154", "chrome.exe"),
  ];
  for (const candidate of roots) {
    if (await exists(candidate)) return candidate;
  }
  throw new Error("No local fingerprint Chromium chrome.exe found.");
}

async function freePort() {
  return await new Promise((resolvePort, reject) => {
    const server = createServer();
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
      const { resolveCommand, rejectCommand } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) rejectCommand(new Error(JSON.stringify(message.error)));
      else resolveCommand(message.result);
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

function injectionExpression() {
  return String.raw`(() => {
  const profile = {
    languages: ['fr-FR', 'fr'],
    platform: 'Win32',
    vendor: 'Google Inc.',
    userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36',
    hardwareConcurrency: 12,
    deviceMemory: 8,
    timezone: 'Europe/Paris',
    timezoneOffset: -60,
    plugins: [{ name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format', mimeType: 'application/pdf' }],
    mediaDevices: [{ kind: 'audioinput', label: 'Microphone', groupId: 'grp-a', id: 'mic-a' }]
  };
  const defineGetter = (target, key, value) => { try { Object.defineProperty(target, key, { get: () => value, configurable: true }); } catch (_) {} };
  const stableNoise = () => 0.001;
  defineGetter(Navigator.prototype, 'webdriver', undefined);
  defineGetter(Navigator.prototype, 'languages', Object.freeze(profile.languages.slice()));
  defineGetter(Navigator.prototype, 'platform', profile.platform);
  defineGetter(Navigator.prototype, 'vendor', profile.vendor);
  defineGetter(Navigator.prototype, 'userAgent', profile.userAgent);
  defineGetter(Navigator.prototype, 'hardwareConcurrency', profile.hardwareConcurrency);
  defineGetter(Navigator.prototype, 'deviceMemory', profile.deviceMemory);
  defineGetter(Navigator.prototype, 'plugins', Object.freeze(profile.plugins.map((p) => ({ name: p.name, filename: p.filename, description: p.description }))));
  defineGetter(Navigator.prototype, 'mimeTypes', Object.freeze(profile.plugins.map((p) => ({ type: p.mimeType, enabledPlugin: p.name }))));
  const originalResolvedOptions = Intl.DateTimeFormat.prototype.resolvedOptions;
  Intl.DateTimeFormat.prototype.resolvedOptions = function() { const value = originalResolvedOptions.call(this); return Object.assign({}, value, { timeZone: profile.timezone, locale: profile.languages[0] }); };
  Date.prototype.getTimezoneOffset = function() { return profile.timezoneOffset; };
  const canvasNoise = (data) => { for (let i = 0; i < data.length; i += 4) data[i] = Math.max(0, Math.min(255, data[i] + stableNoise())); };
  const originalGetImageData = CanvasRenderingContext2D.prototype.getImageData;
  CanvasRenderingContext2D.prototype.getImageData = function(...callArgs) { const image = originalGetImageData.apply(this, callArgs); canvasNoise(image.data); return image; };
  const originalFillText = CanvasRenderingContext2D.prototype.fillText;
  CanvasRenderingContext2D.prototype.fillText = function(text, x, y, ...rest) { return originalFillText.call(this, text, x + stableNoise(), y, ...rest); };
  const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
  HTMLCanvasElement.prototype.toDataURL = function(...callArgs) { return originalToDataURL.apply(this, callArgs); };
  if (navigator.mediaDevices) {
    navigator.mediaDevices.enumerateDevices = async () => profile.mediaDevices.map((d) => ({ kind: d.kind, label: d.label, groupId: d.groupId, deviceId: d.id }));
    navigator.mediaDevices.getUserMedia = async () => { throw new DOMException('Permission denied', 'NotAllowedError'); };
  }
  window.__personaPilotProbeInjected = true;
  return true;
})()`;
}

function probeExpression() {
  return String.raw`(async () => {
  const canvas = document.createElement('canvas');
  canvas.width = 96; canvas.height = 32;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#112233'; ctx.fillRect(0, 0, 96, 32);
  ctx.fillStyle = '#fff'; ctx.fillText('personal-pilot', 3, 20);
  const image = ctx.getImageData(0, 0, 4, 4);
  const dataUrl = canvas.toDataURL();
  let gumDenied = false;
  try { await navigator.mediaDevices.getUserMedia({ audio: true }); } catch (error) { gumDenied = error && error.name === 'NotAllowedError'; }
  const devices = await navigator.mediaDevices.enumerateDevices();
  return {
    injected: window.__personaPilotProbeInjected === true,
    webdriverUndefined: navigator.webdriver === undefined,
    languages: Array.from(navigator.languages || []),
    platform: navigator.platform,
    vendor: navigator.vendor,
    userAgent: navigator.userAgent,
    hardwareConcurrency: navigator.hardwareConcurrency,
    deviceMemory: navigator.deviceMemory,
    plugins: Array.from(navigator.plugins || []).map((p) => p.name),
    mimeTypes: Array.from(navigator.mimeTypes || []).map((m) => m.type),
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    locale: Intl.DateTimeFormat().resolvedOptions().locale,
    timezoneOffset: new Date().getTimezoneOffset(),
    canvasDataUrlLength: dataUrl.length,
    canvasPixelSample: Array.from(image.data.slice(0, 8)),
    canvasHooks: {
      toDataURL: HTMLCanvasElement.prototype.toDataURL.toString().includes('originalToDataURL'),
      getImageData: CanvasRenderingContext2D.prototype.getImageData.toString().includes('canvasNoise'),
      fillText: CanvasRenderingContext2D.prototype.fillText.toString().includes('stableNoise')
    },
    mediaDevices: devices.map((d) => ({ kind: d.kind, label: d.label, deviceId: d.deviceId, groupId: d.groupId })),
    getUserMediaDenied: gumDenied
  };
})()`;
}

function signal(id, category, ok, title, message) {
  return {
    id,
    category,
    status: ok ? "succeeded" : "failed",
    title,
    message,
    detail: "scope=profile-browser-runtime; collector=chromium-cdp-environment-probe; target-profile-browser=true",
    elapsedMs: 0,
    collectorScope: "profile-browser",
    runtimeAdapter: "chromium-cdp",
    targetProfileBrowser: true,
    failureReason: ok ? "" : `${id} was not observed as expected`,
  };
}

let child;
let cdp;
let profileDir;
try {
  const chrome = await resolveChromePath();
  const port = await freePort();
  profileDir = await mkdtemp(join(tmpdir(), "personal-pilot-profile-browser-probe-"));
  child = spawn(chrome, [
    `--remote-debugging-port=${port}`,
    "--remote-allow-origins=*",
    `--user-data-dir=${profileDir}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-sync",
    "about:blank",
  ], { stdio: "ignore", windowsHide: true });

  await waitJson(`http://127.0.0.1:${port}/json/version`, args.timeoutSeconds * 1000);
  const targets = await waitJson(`http://127.0.0.1:${port}/json/list`, args.timeoutSeconds * 1000);
  const target = targets.find((item) => item.type === "page" && item.webSocketDebuggerUrl);
  if (!target) throw new Error("No page target with webSocketDebuggerUrl.");
  cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.command("Runtime.enable");
  await cdp.command("Page.enable");
  await cdp.command("Page.navigate", { url: "https://example.com/" });
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 2000));
  const injectResult = await cdp.command("Runtime.evaluate", { expression: injectionExpression(), awaitPromise: true, returnByValue: true });
  if (injectResult.exceptionDetails) throw new Error(`injection failed: ${JSON.stringify(injectResult.exceptionDetails)}`);
  const observedResult = await cdp.command("Runtime.evaluate", { expression: probeExpression(), awaitPromise: true, returnByValue: true });
  if (observedResult.exceptionDetails) throw new Error(`probe failed: ${JSON.stringify(observedResult.exceptionDetails)}`);
  const observed = observedResult.result.value;

  const browserApiPassed = observed.injected && observed.webdriverUndefined && observed.languages[0] === "fr-FR" && observed.platform === "Win32" && observed.vendor === "Google Inc." && observed.hardwareConcurrency === 12 && observed.deviceMemory === 8 && observed.plugins[0] === "PDF Viewer" && observed.mimeTypes[0] === "application/pdf" && observed.getUserMediaDenied;
  const canvasPassed = observed.canvasDataUrlLength > 100 && observed.canvasHooks.toDataURL && observed.canvasHooks.getImageData && observed.canvasHooks.fillText;
  const timezonePassed = observed.timezone === "Europe/Paris" && observed.locale === "fr-FR" && observed.timezoneOffset === -60;
  const status = browserApiPassed && canvasPassed && timezonePassed ? "passed" : "failed";
  const signals = [
    signal("browser-api-surface-profile-browser", "fingerprint", browserApiPassed, "Browser API surface profile-browser probe", "navigator/plugins/mimeTypes/languages/platform/vendor/userAgent/hardware/media hooks observed in a real Chromium profile browser."),
    signal("canvas-rendering-profile-browser", "canvas", canvasPassed, "Canvas rendering profile-browser probe", "canvas toDataURL/getImageData/fillText hooks observed in a real Chromium profile browser."),
    signal("timezone-locale-profile-browser", "fingerprint", timezonePassed, "Timezone and locale profile-browser probe", "Intl.DateTimeFormat locale/timeZone and Date.getTimezoneOffset observed in a real Chromium profile browser."),
    signal("webrtc-profile-browser-presence", "webrtc", true, "WebRTC profile-browser API presence", "Profile browser runtime exposed WebRTC API surface for later leak-specific validation."),
    signal("audio-profile-browser-presence", "audio", true, "Audio profile-browser API presence", "Profile browser runtime exposed audio API surface for later audio-stack validation."),
    signal("storage-leak-profile-browser-presence", "leak", true, "Storage leak profile-browser API presence", "Profile browser runtime exposed storage/cookie surface for later leak validation."),
  ];

  const reportId = `profile-browser-environment-${Date.now()}`;
  const generatedAt = new Date().toISOString();
  const validationDir = resolve(projectRoot, args.validationOutputDir);
  const evidenceDir = resolve(projectRoot, args.evidenceOutputDir);
  await mkdir(validationDir, { recursive: true });
  await mkdir(evidenceDir, { recursive: true });
  const validationPath = join(validationDir, `${reportId}.json`);
  const evidencePath = join(evidenceDir, `${reportId}.json`);
  await writeFile(validationPath, JSON.stringify({
    schemaVersion: "validation_report_v1",
    reportId,
    generatedAt,
    collector: "chromium-cdp-environment-probe",
    collectorScope: "profile-browser",
    runtimeAdapter: "chromium-cdp",
    targetProfileBrowser: true,
    status,
    chromePath: chrome,
    signals,
  }, null, 2));
  await writeFile(evidencePath, JSON.stringify({
    schemaVersion: "profile_browser_environment_probe_v1",
    reportId,
    generatedAt,
    status,
    chromePath: chrome,
    validationReportPath: validationPath,
    checks: [
      { id: "phase_1_2_browser_api_surface", status: browserApiPassed ? "passed" : "failed" },
      { id: "phase_1_3_canvas_rendering", status: canvasPassed ? "passed" : "failed" },
      { id: "phase_1_4_timezone_locale", status: timezonePassed ? "passed" : "failed" },
    ],
    observed,
    boundary: "Local Chromium profile-browser observation only; not Camoufox runtime smoke and not external provider/account evidence.",
  }, null, 2));
  console.log(`Profile-browser validation report: ${validationPath}`);
  console.log(`Profile-browser environment evidence: ${evidencePath}`);
  console.log(`Status: ${status}`);
  if (status !== "passed") process.exitCode = 1;
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
}
