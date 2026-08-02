import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import net from "node:net";
import os from "node:os";
import path from "node:path";

const projectRoot = process.cwd();
const stamp = Date.now();
const args = process.argv.slice(2);
const personalPilotKey = process.env.PERSONAL_PILOT_API_KEY || "";
const baseOutputDir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "deep-matrix", `deep-${stamp}`);
const products = parseListArg("products", ["personalPilot", "bitBrowser"]).map(normalizeProduct).filter(Boolean);
const submatrices = parseListArg("submatrices", ["clash", "udeal"]).map((value) => value.toLowerCase());
const regionFilters = new Set(parseListArg("regions", []).map((value) => value.toLowerCase()));
const launchesPerCell = Number.parseInt(argValue("launches", "1"), 10);
const smoke = args.includes("--smoke");
const writeRaw = !args.includes("--no-raw");
const externalDetectorsEnabled = !args.includes("--no-external-detectors");
const captureExternalScreenshots = args.includes("--allow-ip-screenshots");
const detectorRateLimitMs = Number.parseInt(argValue("rate-limit-ms", "5000"), 10);
const targetUrl = "https://browserleaks.com/ip";
const ipProbeUrl = "https://ipwho.is/?fields=success,message,ip,country,country_code,region,city,timezone,connection";
const transportProbeUrl = "https://tls.peet.ws/api/all";
const behaviorUrl = `data:text/html;charset=utf-8,${encodeURIComponent(`<!doctype html>
<html>
<head><meta charset="utf-8"><title>benchmark behavior probe</title></head>
<body style="font-family:Arial,sans-serif">
  <button id="btn">click target</button>
  <input id="ascii" />
  <input id="unicode" />
  <textarea id="area"></textarea>
  <div id="drag" draggable="true" style="width:120px;height:40px;background:#ccc;margin-top:16px">drag</div>
  <iframe id="frame" srcdoc="<input id='frameInput'><button id='frameBtn'>frame</button>"></iframe>
  <script>
    window.behaviorLog = [];
    document.getElementById('btn').addEventListener('click', () => window.behaviorLog.push('clicked'));
    document.getElementById('drag').addEventListener('dragstart', () => window.behaviorLog.push('dragstart'));
  </script>
</body>
</html>`)}`;

const detectorDefinitions = {
  "creepjs": {
    id: "creepjs",
    url: "https://abrahamjuliot.github.io/creepjs/",
    waitMs: 12000,
    external: true,
  },
  "browserleaks-canvas": {
    id: "browserleaks-canvas",
    url: "https://browserleaks.com/canvas",
    waitMs: 5000,
    external: true,
  },
  "browserleaks-webgl": {
    id: "browserleaks-webgl",
    url: "https://browserleaks.com/webgl",
    waitMs: 5000,
    external: true,
  },
  "browserleaks-webrtc": {
    id: "browserleaks-webrtc",
    url: "https://browserleaks.com/webrtc",
    waitMs: 7000,
    external: true,
  },
  "browserscan": {
    id: "browserscan",
    url: "https://www.browserscan.net/",
    waitMs: 10000,
    external: true,
  },
  "pixelscan": {
    id: "pixelscan",
    url: "https://pixelscan.net/",
    waitMs: 10000,
    external: true,
  },
};
const detectorIds = parseListArg("detectors", smoke
  ? ["browserleaks-canvas"]
  : ["creepjs", "browserleaks-canvas", "browserleaks-webgl", "browserleaks-webrtc", "browserscan", "pixelscan"]);

const endpoints = {
  personalPilot: "http://127.0.0.1:19876",
  bitBrowser: "http://127.0.0.1:54345",
  clashController: "http://127.0.0.1:9097",
};

const productLabels = {
  personalPilot: "personal-pilot",
  bitBrowser: "bitbrowser",
};

const regions = {
  us: {
    key: "us",
    locale: "en-US",
    acceptLanguage: "en-US,en;q=0.9",
    timezone: "America/Los_Angeles",
    expectedCountryCode: "US",
  },
  jp: {
    key: "jp",
    locale: "ja-JP",
    acceptLanguage: "ja-JP,ja;q=0.9,en;q=0.8",
    timezone: "Asia/Tokyo",
    expectedCountryCode: "JP",
  },
  la: {
    key: "la",
    locale: "en-US",
    acceptLanguage: "en-US,en;q=0.9",
    timezone: "America/Los_Angeles",
    expectedCountryCode: "US",
  },
};

function argValue(name, fallback = "") {
  const prefix = `--${name}=`;
  return args.find((arg) => arg.startsWith(prefix))?.slice(prefix.length) || fallback;
}

function parseListArg(name, fallback) {
  const raw = argValue(name, "");
  if (!raw) return fallback;
  return raw.split(",").map((value) => value.trim()).filter(Boolean);
}

function normalizeProduct(value) {
  const normalized = String(value).trim().toLowerCase();
  if (["personalpilot", "personal-pilot", "pp"].includes(normalized)) return "personalPilot";
  if (["bitbrowser", "bit-browser", "bit"].includes(normalized)) return "bitBrowser";
  return "";
}

function redact(value) {
  if (typeof value !== "string") return value;
  return value
    .replace(/(https?|socks5?|ssh):\/\/([^:@/\s]+):([^@/\s]+)@/gi, "$1://***:***@")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, "Bearer ***")
    .replace(/("?(?:password|token|secret|api[_-]?key|authorization|cookie)"?\s*[:=]\s*)["']?[^"',\s}]+/gi, "$1***")
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, "[ipv4-redacted]")
    .replace(/\b(?:[a-f0-9]{0,4}:){2,}[a-f0-9]{0,4}\b/gi, "[ipv6-redacted]");
}

function hashId(value) {
  return createHash("sha256").update(String(value ?? "")).digest("hex");
}

function shortHash(value) {
  return hashId(value).slice(0, 16);
}

function normalizeError(error) {
  if (!error) return "";
  return redact(error.message || String(error));
}

function safeFileName(value) {
  return String(value)
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 120);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function httpJSON(url, options = {}) {
  const timeoutMs = options.timeoutMs ?? 20000;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(new Error(`timeout after ${timeoutMs}ms`)), timeoutMs);
  const { timeoutMs: _timeoutMs, ...fetchOptions } = options;
  let response;
  try {
    response = await fetch(url, {
      ...fetchOptions,
      signal: controller.signal,
      headers: {
        Accept: "application/json",
        ...(fetchOptions.body ? { "Content-Type": "application/json" } : {}),
        ...(fetchOptions.headers || {}),
      },
    });
  } catch (error) {
    throw new Error(`${fetchOptions.method || "GET"} ${redact(url)} failed: ${normalizeError(error)}`);
  } finally {
    clearTimeout(timer);
  }
  const text = await response.text();
  let body = text;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    body = redact(text.slice(0, 1000));
  }
  return { ok: response.ok, status: response.status, body };
}

async function tcpCheck(host, port, timeoutMs = 1500) {
  return new Promise((resolve) => {
    const socket = new net.Socket();
    let settled = false;
    const finish = (ok, error = "") => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve({ ok, error: redact(error) });
    };
    socket.setTimeout(timeoutMs);
    socket.once("connect", () => finish(true));
    socket.once("timeout", () => finish(false, "timeout"));
    socket.once("error", (error) => finish(false, normalizeError(error)));
    socket.connect(port, host);
  });
}

function localHeaders() {
  return personalPilotKey ? { "X-Personal-Pilot-Api-Key": personalPilotKey } : {};
}

async function loadLatestProxyPreflight() {
  const dir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "proxy-preflight");
  const files = await readdir(dir);
  const clashFile = files.filter((name) => /^clash-airport-preflight-.*\.json$/.test(name)).sort().at(-1);
  const udealFile = files.filter((name) => /^udeal-la-existing-local-bridge-preflight-.*\.json$/.test(name)).sort().at(-1);
  const summaryFile = files.filter((name) => /^proxy-preflight-summary-.*\.json$/.test(name)).sort().at(-1);
  const read = async (file) => JSON.parse(await readFile(path.join(dir, file), "utf8"));
  return {
    summaryFile: summaryFile ? path.join("data", "reports", "three-browser-benchmark", "proxy-preflight", summaryFile) : null,
    clash: clashFile ? await read(clashFile) : null,
    udeal: udealFile ? await read(udealFile) : null,
  };
}

function selectClashNodes(preflight) {
  const ok = (preflight?.results || []).filter((item) => item.status === "ok" && item.nodeName);
  const byRegion = new Map();
  for (const item of ok) {
    const key = String(item.regionExpected || "").toLowerCase();
    if (!["us", "jp"].includes(key)) continue;
    const list = byRegion.get(key) || [];
    list.push(item);
    byRegion.set(key, list);
  }
  const selected = {};
  for (const [key, list] of byRegion.entries()) {
    list.sort((a, b) => (a.elapsedMs || 999999) - (b.elapsedMs || 999999));
    selected[key] = list[0];
  }
  return selected;
}

function readClashSecret() {
  if (process.env.CLASH_CONTROLLER_SECRET) return process.env.CLASH_CONTROLLER_SECRET;
  const candidates = [
    path.join(os.homedir(), "AppData", "Roaming", "io.github.clash-verge-rev.clash-verge-rev", "config.yaml"),
    path.join(os.homedir(), "AppData", "Roaming", "io.github.clash-verge-rev.clash-verge-rev", "clash-verge.yaml"),
    path.join(os.homedir(), ".config", "mihomo", "config.yaml"),
  ];
  for (const file of candidates) {
    if (!existsSync(file)) continue;
    const content = spawnSync("powershell", [
      "-NoProfile",
      "-Command",
      `[Console]::OutputEncoding=[Text.UTF8Encoding]::new(); Get-Content -Raw -LiteralPath ${JSON.stringify(file)}`,
    ], { encoding: "utf8" }).stdout;
    const match = content.match(/^\s*secret\s*:\s*["']?([^"'\r\n#]+)["']?\s*(?:#.*)?$/m);
    if (match?.[1]?.trim()) return match[1].trim();
  }
  return "";
}

function clashHeaders(secret) {
  return secret ? { Authorization: `Bearer ${secret}` } : {};
}

async function getClashState(secret) {
  const [configs, globalProxy] = await Promise.all([
    httpJSON(`${endpoints.clashController}/configs`, { headers: clashHeaders(secret) }),
    httpJSON(`${endpoints.clashController}/proxies/GLOBAL`, { headers: clashHeaders(secret) }),
  ]);
  return {
    mode: configs.body?.mode || null,
    globalNow: globalProxy.body?.now || null,
  };
}

async function setClashMode(secret, mode) {
  await httpJSON(`${endpoints.clashController}/configs`, {
    method: "PATCH",
    headers: clashHeaders(secret),
    body: JSON.stringify({ mode }),
    timeoutMs: 8000,
  });
}

async function setClashGlobal(secret, nodeName) {
  await httpJSON(`${endpoints.clashController}/proxies/GLOBAL`, {
    method: "PUT",
    headers: clashHeaders(secret),
    body: JSON.stringify({ name: nodeName }),
    timeoutMs: 8000,
  });
}

function matrixProfileName(product, cell) {
  return `benchmark-matrix-${productLabels[product]}-${cell.proxyClass}-${cell.region.key}`;
}

async function findPersonalPilotProfile(name) {
  const profiles = await httpJSON(`${endpoints.personalPilot}/api/profiles`, {
    headers: localHeaders(),
    timeoutMs: 45000,
  });
  const items = Array.isArray(profiles.body?.items) ? profiles.body.items : [];
  return items.find((item) => item.profileName === name) || null;
}

async function ensurePersonalPilotProfile(cell) {
  const profileName = matrixProfileName("personalPilot", cell);
  const existing = await findPersonalPilotProfile(profileName);
  if (existing?.profileId) {
    return { product: "personalPilot", profileName, profileId: existing.profileId, reused: true };
  }
  const payload = {
    profile: buildPersonalPilotProfile(profileName, cell),
    launchCode: "",
    autoLaunch: false,
  };
  const created = await httpJSON(`${endpoints.personalPilot}/api/profiles`, {
    method: "POST",
    headers: localHeaders(),
    body: JSON.stringify(payload),
    timeoutMs: 30000,
  });
  if (!created.body?.ok || !created.body?.profileId) {
    throw new Error(`PersonalPilot profile create failed: ${redact(JSON.stringify(created.body))}`);
  }
  return { product: "personalPilot", profileName, profileId: created.body.profileId, reused: false };
}

function buildPersonalPilotProfile(profileName, cell) {
  const region = cell.region;
  return {
    profileName,
    coreId: "core-fingerprint-chromium-139-0-7258-154",
    proxyConfig: cell.proxyServer,
    fingerprintArgs: [
      "--fingerprint-brand=Chrome",
      "--fingerprint-platform=windows",
      "--fingerprint-platform-version=10.0.0",
      `--lang=${region.locale}`,
      `--accept-lang=${region.acceptLanguage}`,
      `--timezone=${region.timezone}`,
      "--fingerprint-hardware-concurrency=8",
      "--fingerprint-device-memory=8",
      "--fingerprint-canvas-noise=true",
      "--fingerprint-audio-noise=true",
    ],
    launchArgs: [
      "--disable-sync",
      "--no-first-run",
      "--window-size=1280,720",
      "--force-device-scale-factor=1",
      "--webrtc-ip-handling-policy=disable_non_proxied_udp",
      "--disable-blink-features=AutomationControlled",
      "--no-default-browser-check",
      "--disable-popup-blocking",
      "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1",
    ],
    tags: ["three-browser-benchmark", "deep-matrix", `proxy:${cell.proxyClass}`, `region:${region.key}`],
  };
}

async function findBitBrowserProfile(name) {
  const list = await httpJSON(`${endpoints.bitBrowser}/browser/list`, {
    method: "POST",
    body: JSON.stringify({ page: 0, pageSize: 200 }),
    timeoutMs: 45000,
  });
  const items = Array.isArray(list.body?.data?.list) ? list.body.data.list : [];
  return items.find((item) => item.name === name) || null;
}

async function ensureBitBrowserProfile(cell) {
  const profileName = matrixProfileName("bitBrowser", cell);
  const existing = await findBitBrowserProfile(profileName);
  if (existing?.id) {
    return { product: "bitBrowser", profileName, profileId: existing.id, reused: true };
  }
  const payload = buildBitBrowserProfile(profileName, cell, "");
  const updated = await httpJSON(`${endpoints.bitBrowser}/browser/update`, {
    method: "POST",
    body: JSON.stringify(payload),
    timeoutMs: 30000,
  });
  if (!updated.body?.success) {
    throw new Error(`BitBrowser profile update failed: ${redact(JSON.stringify(updated.body))}`);
  }
  const profileId = updated.body?.data?.id || updated.body?.data?.browserId || updated.body?.id;
  if (!profileId) throw new Error("BitBrowser profile update returned no id");
  return { product: "bitBrowser", profileName, profileId, reused: false };
}

function buildBitBrowserProfile(profileName, cell, existingId = "") {
  const region = cell.region;
  const [host, port] = cell.proxyServer.replace(/^https?:\/\//, "").split(":");
  return {
    ...(existingId ? { id: existingId } : {}),
    platform: "https://browserleaks.com",
    platformIcon: "browserleaks.com",
    url: targetUrl,
    name: profileName,
    remark: `three-browser-benchmark deep ${cell.proxyClass} ${region.key}`,
    userName: "",
    password: "",
    proxyMethod: 2,
    proxyType: "http",
    host,
    port: Number.parseInt(port, 10),
    browserFingerPrint: {
      coreProduct: "chrome",
      coreVersion: "126",
      ostype: "PC",
      os: "Win32",
      isIpCreateTimeZone: true,
      webRTC: "0",
      position: "1",
      isIpCreatePosition: true,
      isIpCreateLanguage: true,
      openWidth: 1280,
      openHeight: 720,
      hardwareConcurrency: "8",
      deviceMemory: "8",
      canvas: "0",
      webGL: "0",
      webGLMeta: "0",
      audioContext: "0",
      mediaDevice: "0",
      clientRectNoiseEnabled: true,
      language: region.locale,
      timeZone: region.timezone,
    },
  };
}

async function openPersonalPilot(profileId) {
  const startedAt = Date.now();
  const opened = await httpJSON(`${endpoints.personalPilot}/api/launch`, {
    method: "POST",
    headers: localHeaders(),
    body: JSON.stringify({ profileId, startUrls: [targetUrl], skipDefaultStartUrls: true }),
    timeoutMs: 90000,
  });
  return {
    ok: Boolean(opened.body?.ok),
    status: opened.status,
    latencyMs: Date.now() - startedAt,
    debugPort: opened.body?.debugPort || null,
    pid: opened.body?.pid || null,
    rawError: opened.body?.error || "",
  };
}

async function closePersonalPilot(profileId) {
  const closed = await httpJSON(`${endpoints.personalPilot}/api/instances/stop`, {
    method: "POST",
    headers: localHeaders(),
    body: JSON.stringify({ profileId }),
    timeoutMs: 30000,
  });
  return { ok: Boolean(closed.body?.ok), status: closed.status };
}

async function openBitBrowser(profileId) {
  const startedAt = Date.now();
  const opened = await httpJSON(`${endpoints.bitBrowser}/browser/open`, {
    method: "POST",
    body: JSON.stringify({ id: profileId, queue: true }),
    timeoutMs: 90000,
  });
  const httpDebug = opened.body?.data?.http || "";
  return {
    ok: Boolean(opened.body?.success),
    status: opened.status,
    latencyMs: Date.now() - startedAt,
    debugPort: Number.parseInt(String(httpDebug).split(":").at(-1), 10) || null,
    pid: opened.body?.data?.pid || null,
    rawError: opened.body?.msg || opened.body?.message || "",
  };
}

async function closeBitBrowser(profileId) {
  const closed = await httpJSON(`${endpoints.bitBrowser}/browser/close`, {
    method: "POST",
    body: JSON.stringify({ id: profileId }),
    timeoutMs: 30000,
  });
  return { ok: Boolean(closed.body?.success), status: closed.status };
}

async function connectFirstBenchmarkPage(debugPort) {
  const base = `http://127.0.0.1:${debugPort}`;
  const version = await httpJSON(`${base}/json/version`, { timeoutMs: 10000 });
  const tabs = await httpJSON(`${base}/json/list`, { timeoutMs: 10000 });
  const list = Array.isArray(tabs.body) ? tabs.body : [];
  const page = list.find((tab) => tab.type === "page" && String(tab.url || "").includes("browserleaks.com/ip") && tab.webSocketDebuggerUrl)
    || list.find((tab) => tab.type === "page" && tab.webSocketDebuggerUrl)
    || list.find((tab) => tab.webSocketDebuggerUrl);
  if (!page?.webSocketDebuggerUrl) throw new Error("No CDP page target found");
  const client = await CdpClient.connect(page.webSocketDebuggerUrl);
  await client.send("Page.enable").catch(() => null);
  await client.send("Runtime.enable").catch(() => null);
  await client.send("Network.enable").catch(() => null);
  await client.send("Performance.enable").catch(() => null);
  return {
    client,
    cdpVersion: {
      ok: version.ok,
      browser: version.body?.Browser || null,
      protocolVersion: version.body?.["Protocol-Version"] || null,
    },
    tabs: list.slice(0, 5).map((tab) => ({
      type: tab.type || null,
      title: redact(tab.title || ""),
      url: redact(tab.url || ""),
    })),
  };
}

async function waitForReadyState(client, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await client.send("Runtime.evaluate", {
      expression: "document.readyState",
      returnByValue: true,
    }).catch(() => null);
    if (result?.result?.value === "complete" || result?.result?.value === "interactive") return true;
    await sleep(500);
  }
  return false;
}

async function evaluateValue(client, expression, timeoutMs = 15000) {
  const result = await client.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  }, timeoutMs);
  return result?.result?.value;
}

async function navigateAndSummarize(client, url, waitMs = 3000, navTimeoutMs = 20000) {
  const before = client.networkEvents.length;
  const startedAt = Date.now();
  await client.send("Page.navigate", { url }, navTimeoutMs);
  const ready = await waitForReadyState(client, 25000);
  if (waitMs > 0) await sleep(waitMs);
  const page = await evaluateValue(client, `(() => {
    const text = document.body ? document.body.innerText || "" : "";
    const html = document.documentElement ? document.documentElement.outerHTML || "" : "";
    const nav = performance.getEntriesByType("navigation")[0];
    return {
      url: location.href,
      title: document.title,
      readyState: document.readyState,
      textLength: text.length,
      textSampleRedacted: text.slice(0, 600),
      textHashInput: text.slice(0, 20000),
      htmlLength: html.length,
      navigation: nav ? {
        type: nav.type,
        domContentLoadedEventEnd: Math.round(nav.domContentLoadedEventEnd),
        loadEventEnd: Math.round(nav.loadEventEnd),
        duration: Math.round(nav.duration),
        transferSize: nav.transferSize || 0,
        encodedBodySize: nav.encodedBodySize || 0
      } : null
    };
  })()`);
  const networkEvents = client.networkEvents.slice(before).map(sanitizeNetworkEvent);
  return {
    ok: ready,
    latencyMs: Date.now() - startedAt,
    url: redact(page?.url || ""),
    title: redact(page?.title || ""),
    readyState: page?.readyState || null,
    textLength: page?.textLength || 0,
    textHash: shortHash(page?.textHashInput || ""),
    textSampleRedacted: redact(page?.textSampleRedacted || ""),
    htmlLength: page?.htmlLength || 0,
    navigation: page?.navigation || null,
    network: summarizeNetwork(networkEvents),
    networkEvents,
  };
}

function sanitizeNetworkEvent(event) {
  const params = event.params || {};
  if (event.method === "Network.requestWillBeSent") {
    return {
      method: event.method,
      type: params.type || null,
      requestIdHash: shortHash(params.requestId || ""),
      url: redact(params.request?.url || ""),
      requestMethod: params.request?.method || null,
    };
  }
  if (event.method === "Network.responseReceived") {
    return {
      method: event.method,
      type: params.type || null,
      requestIdHash: shortHash(params.requestId || ""),
      url: redact(params.response?.url || ""),
      status: params.response?.status || null,
      protocol: params.response?.protocol || null,
      mimeType: params.response?.mimeType || null,
    };
  }
  if (event.method === "Network.loadingFinished") {
    return {
      method: event.method,
      requestIdHash: shortHash(params.requestId || ""),
      encodedDataLength: params.encodedDataLength || 0,
    };
  }
  if (event.method === "Network.loadingFailed") {
    return {
      method: event.method,
      requestIdHash: shortHash(params.requestId || ""),
      errorText: redact(params.errorText || ""),
      canceled: Boolean(params.canceled),
    };
  }
  return { method: event.method };
}

function summarizeNetwork(events) {
  const responses = events.filter((event) => event.method === "Network.responseReceived");
  const failures = events.filter((event) => event.method === "Network.loadingFailed");
  return {
    requestCount: events.filter((event) => event.method === "Network.requestWillBeSent").length,
    responseCount: responses.length,
    failureCount: failures.length,
    statusCounts: countBy(responses.map((event) => String(event.status || "unknown"))),
    protocolCounts: countBy(responses.map((event) => String(event.protocol || "unknown"))),
  };
}

function countBy(values) {
  const out = {};
  for (const value of values) out[value] = (out[value] || 0) + 1;
  return out;
}

async function captureScreenshot(client, relativePath) {
  try {
    const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false }, 20000);
    if (!shot?.data) return { ok: false, error: "no screenshot data" };
    const absolute = path.join(baseOutputDir, relativePath);
    await mkdir(path.dirname(absolute), { recursive: true });
    await writeFile(absolute, Buffer.from(shot.data, "base64"));
    return { ok: true, path: path.relative(projectRoot, absolute), sizeBytes: Buffer.byteLength(shot.data, "base64") };
  } catch (error) {
    return { ok: false, error: normalizeError(error) };
  }
}

async function runInternalFingerprintProbe(client) {
  const value = await evaluateValue(client, `(() => {
    function hash(input) {
      const text = String(input || "");
      let h = 2166136261;
      for (let i = 0; i < text.length; i++) {
        h ^= text.charCodeAt(i);
        h = Math.imul(h, 16777619);
      }
      return (h >>> 0).toString(16).padStart(8, "0");
    }
    function webglInfo() {
      try {
        const canvas = document.createElement("canvas");
        const gl = canvas.getContext("webgl") || canvas.getContext("experimental-webgl");
        if (!gl) return null;
        const ext = gl.getExtension("WEBGL_debug_renderer_info");
        return {
          vendor: ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR),
          renderer: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
          version: gl.getParameter(gl.VERSION),
          extensionsHash: hash((gl.getSupportedExtensions() || []).join("|"))
        };
      } catch (error) {
        return { error: String(error && error.message || error) };
      }
    }
    function canvasHash() {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = 320;
        canvas.height = 96;
        const ctx = canvas.getContext("2d");
        ctx.textBaseline = "top";
        ctx.font = "16px Arial";
        ctx.fillStyle = "#f60";
        ctx.fillRect(10, 10, 120, 35);
        ctx.fillStyle = "#069";
        ctx.fillText("Deep benchmark 字形 123", 15, 18);
        return hash(canvas.toDataURL());
      } catch (error) {
        return "error:" + String(error && error.message || error);
      }
    }
    function audioHash() {
      try {
        const Ctx = window.OfflineAudioContext || window.webkitOfflineAudioContext;
        if (!Ctx) return null;
        return "available";
      } catch (error) {
        return "error:" + String(error && error.message || error);
      }
    }
    return {
      url: location.href,
      title: document.title,
      userAgent: navigator.userAgent,
      webdriver: navigator.webdriver,
      language: navigator.language,
      languages: Array.from(navigator.languages || []),
      platform: navigator.platform,
      vendor: navigator.vendor,
      productSub: navigator.productSub,
      hardwareConcurrency: navigator.hardwareConcurrency,
      deviceMemory: navigator.deviceMemory || null,
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
      screen: {
        width: screen.width,
        height: screen.height,
        availWidth: screen.availWidth,
        availHeight: screen.availHeight,
        colorDepth: screen.colorDepth,
        pixelDepth: screen.pixelDepth,
        devicePixelRatio,
      },
      pluginsLength: navigator.plugins ? navigator.plugins.length : null,
      mimeTypesLength: navigator.mimeTypes ? navigator.mimeTypes.length : null,
      permissionsAvailable: Boolean(navigator.permissions && navigator.permissions.query),
      mediaDevicesAvailable: Boolean(navigator.mediaDevices && navigator.mediaDevices.enumerateDevices),
      webgpuAvailable: Boolean(navigator.gpu),
      webgl: webglInfo(),
      canvasHash: canvasHash(),
      audioProbe: audioHash(),
      clientRectsHash: hash(JSON.stringify(document.body.getBoundingClientRect())),
      storage: {
        localStorage: Boolean(window.localStorage),
        sessionStorage: Boolean(window.sessionStorage),
        indexedDB: Boolean(window.indexedDB)
      }
    };
  })()`);
  return {
    ...value,
    fingerprintHash: shortHash(JSON.stringify(value || {})),
  };
}

async function runIpProbe(client) {
  await client.send("Page.navigate", { url: ipProbeUrl }, 20000);
  await waitForReadyState(client, 20000);
  const value = await evaluateValue(client, `(() => {
    const text = document.body ? document.body.innerText.trim() : "";
    try { return JSON.parse(text); }
    catch (error) { return { success: false, parseError: String(error && error.message || error), text: text.slice(0, 300) }; }
  })()`);
  const rawIp = value?.ip || "";
  if (value && typeof value === "object") {
    delete value.ip;
  }
  return {
    ok: Boolean(value?.success !== false && value?.country_code),
    exitIpHash: rawIp ? hashId(rawIp) : null,
    data: value || null,
  };
}

async function runTransportProbe(client) {
  const nav = await navigateAndSummarize(client, transportProbeUrl, 1000);
  const value = await evaluateValue(client, `(() => {
    const text = document.body ? document.body.innerText.trim() : "";
    try { return JSON.parse(text); }
    catch (error) { return { parseError: String(error && error.message || error), text: text.slice(0, 300) }; }
  })()`);
  return {
    navigation: {
      ok: nav.ok,
      latencyMs: nav.latencyMs,
      network: nav.network,
    },
    observed: summarizeTransportPayload(value || {}),
  };
}

function summarizeTransportPayload(payload) {
  const ip = payload.ip || payload.remote_addr || "";
  const tls = payload.tls || {};
  const http2 = payload.http2 || {};
  const httpVersion = payload.http_version || payload.httpVersion || payload.http?.version || null;
  const headers = payload.http?.headers || payload.headers || [];
  return {
    ok: Boolean(tls.ja3_hash || tls.ja4 || httpVersion || http2.akamai_fingerprint),
    exitIpHash: ip ? hashId(ip) : null,
    httpVersion,
    tls: {
      ja3Hash: tls.ja3_hash || null,
      ja4: tls.ja4 || null,
      tlsVersion: tls.tls_version_record || tls.version || null,
      cipherSuite: tls.ciphersuite || null,
      extensionsHash: tls.extensions ? shortHash(JSON.stringify(tls.extensions)) : null,
    },
    http2: {
      akamaiFingerprintHash: http2.akamai_fingerprint ? shortHash(http2.akamai_fingerprint) : null,
      sentFramesHash: http2.sent_frames ? shortHash(JSON.stringify(http2.sent_frames)) : null,
    },
    headers: {
      orderHash: headers ? shortHash(JSON.stringify(headers)) : null,
      count: Array.isArray(headers) ? headers.length : Object.keys(headers || {}).length,
    },
  };
}

async function runWebRtcProbe(client) {
  const value = await evaluateValue(client, `(() => new Promise((resolve) => {
    const RTCPeer = window.RTCPeerConnection || window.webkitRTCPeerConnection;
    if (!RTCPeer) {
      resolve({ available: false, candidates: [], error: "RTCPeerConnection missing" });
      return;
    }
    const pc = new RTCPeer({ iceServers: [{ urls: "stun:stun.l.google.com:19302" }] });
    const candidates = [];
    const done = () => {
      try { pc.close(); } catch {}
      resolve({ available: true, candidates });
    };
    pc.onicecandidate = (event) => {
      if (event && event.candidate && event.candidate.candidate) candidates.push(event.candidate.candidate);
      if (!event || !event.candidate) done();
    };
    try {
      pc.createDataChannel("probe");
      pc.createOffer().then((offer) => pc.setLocalDescription(offer)).catch((error) => {
        resolve({ available: true, candidates, error: String(error && error.message || error) });
      });
      setTimeout(done, 7000);
    } catch (error) {
      resolve({ available: true, candidates, error: String(error && error.message || error) });
    }
  }))`, 12000);
  const candidates = Array.isArray(value?.candidates) ? value.candidates : [];
  return {
    available: Boolean(value?.available),
    error: redact(value?.error || ""),
    candidateCount: candidates.length,
    candidates: candidates.map(summarizeIceCandidate),
  };
}

function summarizeIceCandidate(candidate) {
  const type = candidate.match(/\styp\s+([a-z0-9]+)/i)?.[1] || null;
  const protocol = candidate.match(/\s(udp|tcp)\s/i)?.[1]?.toLowerCase() || null;
  const ipMatches = candidate.match(/\b(?:\d{1,3}\.){3}\d{1,3}\b|\b(?:[a-f0-9]{0,4}:){2,}[a-f0-9]{0,4}\b/gi) || [];
  return {
    candidateHash: shortHash(candidate),
    type,
    protocol,
    addressHashes: ipMatches.map(hashId),
    rawRedacted: redact(candidate),
  };
}

async function runDetectorSuite(client, launchKey) {
  if (!externalDetectorsEnabled) {
    return {
      skipped: true,
      reason: "external detectors disabled by --no-external-detectors",
      detectors: [],
    };
  }
  const out = [];
  for (const detectorId of detectorIds) {
    const detector = detectorDefinitions[detectorId];
    if (!detector) {
      out.push({ id: detectorId, ok: false, error: "detector not in allowlist" });
      continue;
    }
    try {
      const nav = await navigateAndSummarize(client, detector.url, detector.waitMs, Math.max(30000, detector.waitMs + 20000));
      const screenshot = captureExternalScreenshots
        ? await captureScreenshot(client, path.join("screenshots", `${launchKey}_${detector.id}.png`))
        : { ok: false, skipped: true, reason: "external screenshots disabled to avoid raw IP leakage" };
      const detectorOk = nav.ok && !String(nav.url || "").startsWith("chrome-error:") && nav.textLength > 0;
      out.push({
        id: detector.id,
        url: detector.url,
        ok: detectorOk,
        pageError: String(nav.url || "").startsWith("chrome-error:") ? "chrome-error" : "",
        latencyMs: nav.latencyMs,
        title: nav.title,
        finalUrl: nav.url,
        textLength: nav.textLength,
        textHash: nav.textHash,
        textSampleRedacted: nav.textSampleRedacted,
        htmlLength: nav.htmlLength,
        network: nav.network,
        screenshot,
      });
      await writeJSON(path.join(baseOutputDir, "har-lite", `${launchKey}_${detector.id}.json`), {
        detector: detector.id,
        networkEvents: nav.networkEvents,
      });
    } catch (error) {
      out.push({
        id: detector.id,
        url: detector.url,
        ok: false,
        error: normalizeError(error),
      });
      await client.send("Page.stopLoading", {}, 5000).catch(() => null);
    }
    await sleep(detectorRateLimitMs);
  }
  return { skipped: false, detectors: out };
}

async function runBehaviorProbe(client, launchKey) {
  const nav = await navigateAndSummarize(client, behaviorUrl, 500);
  const screenshotBefore = await captureScreenshot(client, path.join("screenshots", `${launchKey}_behavior-before.png`));
  const box = await evaluateValue(client, `(() => {
    const r = document.getElementById("btn").getBoundingClientRect();
    return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) };
  })()`);
  const mouseClick = {
    attempted: false,
    ok: false,
    box: box && Number.isFinite(box.x) && Number.isFinite(box.y) ? box : null,
    error: "",
  };
  if (box && Number.isFinite(box.x) && Number.isFinite(box.y)) {
    mouseClick.attempted = true;
    try {
      await client.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: box.x, y: box.y }, 5000);
      await client.send("Input.dispatchMouseEvent", { type: "mousePressed", x: box.x, y: box.y, button: "left", clickCount: 1 }, 5000);
      await client.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: box.x, y: box.y, button: "left", clickCount: 1 }, 5000);
      mouseClick.ok = true;
    } catch (error) {
      mouseClick.error = normalizeError(error);
    }
  }
  await evaluateValue(client, `document.getElementById("ascii").focus(); true;`);
  await client.send("Input.insertText", { text: "ascii-123" }, 5000).catch(() => null);
  await evaluateValue(client, `document.getElementById("unicode").focus(); true;`);
  await client.send("Input.insertText", { text: "中文输入" }, 5000).catch(() => null);
  await evaluateValue(client, `(() => {
    document.getElementById("area").focus();
    document.getElementById("area").value = "line1\\nline2";
    window.scrollTo(0, document.body.scrollHeight);
    const frame = document.getElementById("frame");
    const input = frame.contentDocument.getElementById("frameInput");
    input.value = "iframe-ok";
    window.behaviorLog.push("scripted-area");
    return true;
  })()`);
  const state = await evaluateValue(client, `(() => ({
    clicked: window.behaviorLog.includes("clicked"),
    log: window.behaviorLog.slice(),
    ascii: document.getElementById("ascii").value,
    unicode: document.getElementById("unicode").value,
    area: document.getElementById("area").value,
    scrollY: window.scrollY,
    iframe: document.getElementById("frame").contentDocument.getElementById("frameInput").value
  }))()`);
  const screenshotAfter = await captureScreenshot(client, path.join("screenshots", `${launchKey}_behavior-after.png`));
  const checks = {
    click: mouseClick.ok && state?.clicked === true,
    asciiInput: state?.ascii === "ascii-123",
    unicodeInput: state?.unicode === "中文输入",
    textarea: state?.area === "line1\nline2",
    scroll: Number(state?.scrollY || 0) >= 0,
    iframe: state?.iframe === "iframe-ok",
  };
  return {
    navigation: {
      ok: nav.ok,
      latencyMs: nav.latencyMs,
      network: nav.network,
    },
    inputPath: "cdp_synthetic_cross_product_probe",
    note: "This does not validate PersonaPilot OS SendInput Unicode path; it validates browser/CDP behavior parity across products.",
    actions: { mouseClick },
    checks,
    ok: Object.values(checks).every(Boolean),
    state,
    screenshots: { before: screenshotBefore, after: screenshotAfter },
  };
}

async function safeDeepStep(name, fn) {
  try {
    return await fn();
  } catch (error) {
    return {
      ok: false,
      probe: name,
      error: normalizeError(error),
    };
  }
}

async function runDeepProbe(opened, context) {
  const launchKey = context.launchKey;
  const { client, cdpVersion, tabs } = await connectFirstBenchmarkPage(opened.debugPort);
  try {
    const initialFingerprint = await safeDeepStep("initialFingerprint", () => runInternalFingerprintProbe(client));
    const ip = await safeDeepStep("ip", () => runIpProbe(client));
    const transport = await safeDeepStep("transport", () => runTransportProbe(client));
    const webrtc = await safeDeepStep("webrtc", () => runWebRtcProbe(client));
    const behavior = await safeDeepStep("behavior", () => runBehaviorProbe(client, launchKey));
    const detectorSuite = await safeDeepStep("detectorSuite", () => runDetectorSuite(client, launchKey));
    const finalFingerprint = await runInternalFingerprintProbe(client).catch((error) => ({ error: normalizeError(error) }));
    const metrics = await client.send("Performance.getMetrics", {}, 10000).catch((error) => ({ error: normalizeError(error) }));
    await writeJSON(path.join(baseOutputDir, "cdp-trace", `${launchKey}.json`), {
      cdpVersion,
      tabs,
      performanceMetrics: metrics,
      networkEventCount: client.networkEvents.length,
    });
    return {
      cdpVersion,
      tabs,
      initialFingerprint,
      ip,
      transport,
      webrtc,
      behavior,
      detectorSuite,
      finalFingerprint,
      fingerprintHash: shortHash(JSON.stringify(finalFingerprint?.fingerprint || finalFingerprint || initialFingerprint || {})),
    };
  } finally {
    client.close();
  }
}

class CdpClient {
  constructor(ws) {
    this.ws = ws;
    this.nextId = 1;
    this.pending = new Map();
    this.networkEvents = [];
    ws.addEventListener("message", (event) => {
      let msg = null;
      try {
        msg = JSON.parse(event.data);
      } catch {
        return;
      }
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject, timer } = this.pending.get(msg.id);
        clearTimeout(timer);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(msg.error.message || JSON.stringify(msg.error)));
        else resolve(msg.result || {});
        return;
      }
      if (msg.method && msg.method.startsWith("Network.")) {
        this.networkEvents.push({ method: msg.method, params: msg.params || {} });
      }
    });
  }

  static connect(wsUrl) {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(wsUrl);
      const timer = setTimeout(() => {
        try { ws.close(); } catch {}
        reject(new Error("CDP websocket open timeout"));
      }, 10000);
      ws.addEventListener("open", () => {
        clearTimeout(timer);
        resolve(new CdpClient(ws));
      }, { once: true });
      ws.addEventListener("error", () => {
        clearTimeout(timer);
        reject(new Error("CDP websocket error"));
      }, { once: true });
    });
  }

  send(method, params = {}, timeoutMs = 15000) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP ${method} timeout`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.ws.send(payload);
    });
  }

  close() {
    for (const { reject, timer } of this.pending.values()) {
      clearTimeout(timer);
      reject(new Error("CDP websocket closed"));
    }
    this.pending.clear();
    try { this.ws.close(); } catch {}
  }
}

function buildCells(preflight) {
  const cells = [];
  const selectedNodes = selectClashNodes(preflight.clash);
  if (submatrices.includes("clash")) {
    for (const key of ["us", "jp"]) {
      if (regionFilters.size && !regionFilters.has(key)) continue;
      if (selectedNodes[key]) {
        cells.push({
          submatrix: "clash",
          proxyClass: "clash_airport",
          region: regions[key],
          proxyServer: "http://127.0.0.1:7897",
          clashNodeName: selectedNodes[key].nodeName,
          proxyMeta: {
            nodeNameHash: shortHash(selectedNodes[key].nodeName),
            nodeType: selectedNodes[key].nodeType || null,
            expectedCountryCode: selectedNodes[key].countryCode || null,
            timezone: selectedNodes[key].timezone || null,
            asn: selectedNodes[key].asn || null,
            ipApiProxy: selectedNodes[key].ipApiProxy ?? null,
            ipApiHosting: selectedNodes[key].ipApiHosting ?? null,
            exitIpHash: selectedNodes[key].exitIpHash || null,
          },
        });
      }
    }
  }
  if (submatrices.includes("udeal") && (!regionFilters.size || regionFilters.has("la") || regionFilters.has("us"))) {
    cells.push({
      submatrix: "udeal",
      proxyClass: "udeal_la_via_panda",
      region: regions.la,
      proxyServer: "http://127.0.0.1:18082",
      proxyMeta: {
        providerAlias: "udeal-la-via-panda-existing-local-bridge",
        localEndpoint: "127.0.0.1:18082",
        expectedCountryCode: "US",
        timezone: "America/Los_Angeles",
      },
    });
  }
  return cells;
}

async function prepareProfiles(cells) {
  const prepared = [];
  for (const cell of cells) {
    for (const product of products) {
      if (product === "personalPilot") prepared.push({ cell, profile: await ensurePersonalPilotProfile(cell) });
      if (product === "bitBrowser") prepared.push({ cell, profile: await ensureBitBrowserProfile(cell) });
    }
  }
  return prepared;
}

async function runLaunch(prepared, attempt, clashSecret) {
  const { cell, profile } = prepared;
  const launchKey = [
    String(attempt).padStart(2, "0"),
    productLabels[profile.product],
    cell.submatrix,
    cell.region.key,
  ].map(safeFileName).join("_");
  const result = {
    schema: "two_browser_benchmark_deep_launch_v1",
    generatedAt: new Date().toISOString(),
    product: profile.product,
    profileName: profile.profileName,
    profileIdHash: shortHash(profile.profileId),
    submatrix: cell.submatrix,
    proxyClass: cell.proxyClass,
    region: cell.region.key,
    attempt,
    targetUrl,
    proxyServerHash: shortHash(cell.proxyServer),
    proxyMeta: cell.proxyMeta,
    status: "pending",
  };
  try {
    if (cell.submatrix === "clash") {
      await setClashMode(clashSecret, "global");
      await setClashGlobal(clashSecret, cell.clashNodeName);
      result.clashNodeHash = shortHash(cell.clashNodeName);
    }
    const opened = profile.product === "personalPilot"
      ? await openPersonalPilot(profile.profileId)
      : await openBitBrowser(profile.profileId);
    result.open = {
      ok: opened.ok,
      status: opened.status,
      latencyMs: opened.latencyMs,
      debugPortPresent: Boolean(opened.debugPort),
      pidPresent: Boolean(opened.pid),
      error: redact(opened.rawError || ""),
    };
    if (!opened.ok || !opened.debugPort) {
      result.status = "open_failed";
      return result;
    }
    result.deep = await runDeepProbe(opened, { launchKey, cell, profile });
    const ipCountry = result.deep?.ip?.data?.country_code || result.deep?.ip?.data?.countryCode || null;
    result.countryMatchesExpected = ipCountry ? ipCountry === cell.region.expectedCountryCode : null;
    result.status = "ok";
  } catch (error) {
    result.status = "error";
    result.error = normalizeError(error);
  } finally {
    try {
      result.close = profile.product === "personalPilot"
        ? await closePersonalPilot(profile.profileId)
        : await closeBitBrowser(profile.profileId);
    } catch (error) {
      result.close = { ok: false, error: normalizeError(error) };
    }
  }
  return result;
}

function summarize(results, prepared, config, blockers) {
  const byProduct = {};
  const byCell = {};
  for (const result of results) {
    const product = result.product;
    const cellKey = `${result.submatrix}:${result.region}`;
    byProduct[product] ||= { total: 0, ok: 0, failed: 0, countriesMatched: 0, countriesObserved: 0, transportObserved: 0, behaviorOk: 0, detectorPagesOk: 0, detectorPagesTotal: 0 };
    byCell[cellKey] ||= { total: 0, ok: 0, failed: 0 };
    byProduct[product].total += 1;
    byCell[cellKey].total += 1;
    if (result.status === "ok") {
      byProduct[product].ok += 1;
      byCell[cellKey].ok += 1;
    } else {
      byProduct[product].failed += 1;
      byCell[cellKey].failed += 1;
    }
    if (typeof result.countryMatchesExpected === "boolean") {
      byProduct[product].countriesObserved += 1;
      if (result.countryMatchesExpected) byProduct[product].countriesMatched += 1;
    }
    if (result.deep?.transport?.observed?.ok) byProduct[product].transportObserved += 1;
    if (result.deep?.behavior?.ok) byProduct[product].behaviorOk += 1;
    const detectorSuite = result.deep?.detectorSuite;
    const detectors = detectorSuite?.detectors || [];
    if (detectors.length) {
      byProduct[product].detectorPagesTotal += detectors.length;
      byProduct[product].detectorPagesOk += detectors.filter((item) => item.ok).length;
    } else if (detectorSuite?.probe === "detectorSuite" && detectorSuite?.ok === false) {
      byProduct[product].detectorPagesTotal += detectorIds.length;
    }
  }
  return {
    schema: "two_browser_benchmark_deep_summary_v1",
    generatedAt: new Date().toISOString(),
    outputDir: path.relative(projectRoot, baseOutputDir),
    config,
    plannedLaunches: prepared.length * config.launchesPerCell,
    completedLaunches: results.length,
    blockers,
    byProduct,
    byCell,
    detectorIds,
  };
}

async function writeScorecard(summary) {
  const lines = [
    "# PersonaPilot / BitBrowser Deep Matrix Scorecard",
    "",
    `Generated: ${summary.generatedAt}`,
    "",
    `Planned launches: ${summary.plannedLaunches}`,
    `Completed launches: ${summary.completedLaunches}`,
    "",
    "## Products",
    "",
    "| Product | Total | OK | Failed | Country matched | Transport | Behavior | Detector pages |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  for (const [product, item] of Object.entries(summary.byProduct)) {
    lines.push(`| ${product} | ${item.total} | ${item.ok} | ${item.failed} | ${item.countriesMatched}/${item.countriesObserved} | ${item.transportObserved}/${item.total} | ${item.behaviorOk}/${item.total} | ${item.detectorPagesOk}/${item.detectorPagesTotal} |`);
  }
  lines.push("", "## Cells", "", "| Cell | Total | OK | Failed |", "| --- | ---: | ---: | ---: |");
  for (const [cell, item] of Object.entries(summary.byCell)) {
    lines.push(`| ${cell} | ${item.total} | ${item.ok} | ${item.failed} |`);
  }
  if (summary.blockers.length) {
    lines.push("", "## Blockers", "");
    for (const blocker of summary.blockers) lines.push(`- ${blocker}`);
  }
  await writeFile(path.join(baseOutputDir, "scorecard.md"), `${lines.join("\n")}\n`, "utf8");
}

async function writeJSON(file, value) {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, JSON.stringify(value, null, 2), "utf8");
}

async function main() {
  if (!Number.isFinite(launchesPerCell) || launchesPerCell < 1) throw new Error("--launches must be >= 1");
  for (const dir of ["raw", "screenshots", "har-lite", "cdp-trace", "detector-reports", "transport", "behavior"]) {
    await mkdir(path.join(baseOutputDir, dir), { recursive: true });
  }
  const blockers = [];
  const ports = {
    personalPilot: products.includes("personalPilot") ? await tcpCheck("127.0.0.1", 19876) : null,
    bitBrowser: products.includes("bitBrowser") ? await tcpCheck("127.0.0.1", 54345) : null,
    clashMixed: submatrices.includes("clash") ? await tcpCheck("127.0.0.1", 7897) : null,
    udealBridgeHttp: submatrices.includes("udeal") ? await tcpCheck("127.0.0.1", 18082) : null,
  };
  if (products.includes("personalPilot") && !personalPilotKey) blockers.push("PERSONAL_PILOT_API_KEY is required for PersonaPilot LaunchServer.");
  for (const [name, status] of Object.entries(ports)) {
    if (status && !status.ok) blockers.push(`${name} port is not reachable: ${status.error}`);
  }

  const preflight = await loadLatestProxyPreflight();
  const cells = buildCells(preflight);
  let clashSecret = "";
  let clashOriginal = null;
  if (submatrices.includes("clash")) {
    clashSecret = readClashSecret();
    if (!clashSecret) blockers.push("Clash controller secret was not found.");
    else {
      try {
        clashOriginal = await getClashState(clashSecret);
      } catch (error) {
        blockers.push(`Clash controller state read failed: ${normalizeError(error)}`);
      }
    }
  }

  const selectedDetectors = detectorIds.map((id) => detectorDefinitions[id]).filter(Boolean).map((item) => ({
    id: item.id,
    url: item.url,
    external: item.external,
    screenshotsEnabled: captureExternalScreenshots,
  }));
  const config = {
    products,
    submatrices,
    regionFilters: Array.from(regionFilters),
    launchesPerCell: smoke ? 1 : launchesPerCell,
    targetUrl,
    ipProbe: "ipwho.is via browser page; raw ip removed, SHA256 hash retained",
    transportProbe: "tls.peet.ws via browser page; raw ip removed, hashes retained",
    externalDetectorsEnabled,
    detectorRateLimitMs,
    detectorAllowlist: selectedDetectors,
    externalScreenshots: captureExternalScreenshots ? "enabled_by_flag" : "disabled_to_avoid_raw_ip_leakage",
    preflightSummary: preflight.summaryFile,
    rawEnabled: writeRaw,
    ports,
    cells: cells.map((cell) => ({
      submatrix: cell.submatrix,
      proxyClass: cell.proxyClass,
      region: cell.region.key,
      proxyServerHash: shortHash(cell.proxyServer),
      proxyMeta: cell.proxyMeta,
      clashNodeHash: cell.clashNodeName ? shortHash(cell.clashNodeName) : null,
    })),
    clashController: submatrices.includes("clash") ? {
      endpoint: "127.0.0.1:9097",
      secretPresent: Boolean(clashSecret),
      originalMode: clashOriginal?.mode || null,
      originalGlobalHash: clashOriginal?.globalNow ? shortHash(clashOriginal.globalNow) : null,
    } : null,
    blockedExternalItems: [
      "DE same-class proxy node is unavailable locally.",
      "AdsPower Free account API/MCP is paywalled, so AdsPower automation remains manual-only.",
      "Authoritative DNS-token leak proof needs a controlled domain and is not faked by this script.",
    ],
  };
  await writeJSON(path.join(baseOutputDir, "config.redacted.json"), config);

  let results = [];
  if (blockers.length === 0) {
    const prepared = await prepareProfiles(cells);
    await writeJSON(path.join(baseOutputDir, "profiles.redacted.json"), prepared.map(({ cell, profile }) => ({
      product: profile.product,
      profileName: profile.profileName,
      profileIdHash: shortHash(profile.profileId),
      reused: profile.reused,
      submatrix: cell.submatrix,
      region: cell.region.key,
      proxyClass: cell.proxyClass,
    })));

    const launchCount = smoke ? 1 : launchesPerCell;
    try {
      for (let attempt = 1; attempt <= launchCount; attempt += 1) {
        for (const item of prepared) {
          const result = await runLaunch(item, attempt, clashSecret);
          results.push(result);
          const rawName = [
            String(attempt).padStart(2, "0"),
            productLabels[result.product],
            result.submatrix,
            result.region,
            result.status,
          ].map(safeFileName).join("_");
          if (writeRaw) {
            await writeJSON(path.join(baseOutputDir, "raw", `${rawName}.json`), result);
            if (result.deep?.detectorSuite) await writeJSON(path.join(baseOutputDir, "detector-reports", `${rawName}.json`), result.deep.detectorSuite);
            if (result.deep?.transport) await writeJSON(path.join(baseOutputDir, "transport", `${rawName}.json`), result.deep.transport);
            if (result.deep?.behavior) await writeJSON(path.join(baseOutputDir, "behavior", `${rawName}.json`), result.deep.behavior);
          }
          console.log(JSON.stringify({
            attempt,
            product: result.product,
            submatrix: result.submatrix,
            region: result.region,
            status: result.status,
            countryMatchesExpected: result.countryMatchesExpected,
            transport: Boolean(result.deep?.transport?.observed?.ok),
            behavior: Boolean(result.deep?.behavior?.ok),
            detectorsOk: (result.deep?.detectorSuite?.detectors || []).filter((detector) => detector.ok).length,
            detectorsTotal: (result.deep?.detectorSuite?.detectors || []).length,
          }));
          await sleep(1200);
        }
      }
    } finally {
      if (clashOriginal && clashSecret) {
        try {
          if (clashOriginal.globalNow) await setClashGlobal(clashSecret, clashOriginal.globalNow);
          if (clashOriginal.mode) await setClashMode(clashSecret, clashOriginal.mode);
        } catch (error) {
          blockers.push(`Clash restore failed: ${normalizeError(error)}`);
        }
      }
    }
    const summary = summarize(results, prepared, config, blockers);
    await writeJSON(path.join(baseOutputDir, "summary.json"), summary);
    await writeScorecard(summary);
    console.log(JSON.stringify({
      ok: blockers.length === 0 && results.every((result) => result.status === "ok"),
      outputDir: baseOutputDir,
      completedLaunches: results.length,
      blockers,
      byProduct: summary.byProduct,
    }, null, 2));
    return;
  }

  const summary = summarize(results, [], config, blockers);
  await writeJSON(path.join(baseOutputDir, "summary.json"), summary);
  await writeScorecard(summary);
  console.log(JSON.stringify({ ok: false, outputDir: baseOutputDir, completedLaunches: 0, blockers }, null, 2));
  process.exitCode = 1;
}

main().catch(async (error) => {
  console.error(normalizeError(error));
  try {
    await mkdir(baseOutputDir, { recursive: true });
    await writeFile(path.join(baseOutputDir, "fatal-error.txt"), normalizeError(error), "utf8");
  } catch {}
  process.exit(1);
});
