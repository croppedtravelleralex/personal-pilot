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
const baseOutputDir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "missing-probes", `missing-${stamp}`);
const products = parseListArg("products", ["personalPilot", "bitBrowser"]).map(normalizeProduct).filter(Boolean);
const cellFilters = parseListArg("cells", ["clash:us", "clash:jp", "udeal:la"]).map((value) => value.toLowerCase());
const maxBitBrowserOpens = Number.parseInt(argValue("max-bitbrowser-opens", "3"), 10);
const rateLimitMs = Number.parseInt(argValue("rate-limit-ms", "3000"), 10);

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
    expectedCountryCode: "US",
    timezone: "America/Los_Angeles",
  },
  jp: {
    key: "jp",
    expectedCountryCode: "JP",
    timezone: "Asia/Tokyo",
  },
  la: {
    key: "la",
    expectedCountryCode: "US",
    timezone: "America/Los_Angeles",
  },
};

const urls = {
  ip: "https://ipwho.is/?fields=success,message,ip,country,country_code,region,city,timezone,connection",
  tls: "https://tls.peet.ws/api/all",
  creepjs: "https://abrahamjuliot.github.io/creepjs/",
  browserLeaksDns: "https://browserleaks.com/dns",
  browserLeaksWebrtc: "https://browserleaks.com/webrtc",
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

function hashId(value) {
  return createHash("sha256").update(String(value ?? "")).digest("hex");
}

function shortHash(value) {
  return hashId(value).slice(0, 16);
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

function normalizeError(error) {
  if (!error) return "";
  return redact(error.message || String(error));
}

function safeFileName(value) {
  return String(value).toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 120);
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
    httpJSON(`${endpoints.clashController}/configs`, { headers: clashHeaders(secret), timeoutMs: 8000 }),
    httpJSON(`${endpoints.clashController}/proxies/GLOBAL`, { headers: clashHeaders(secret), timeoutMs: 8000 }),
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

async function loadLatestProxyPreflight() {
  const dir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "proxy-preflight");
  const files = await readdir(dir);
  const clashFile = files.filter((name) => /^clash-airport-preflight-.*\.json$/.test(name)).sort().at(-1);
  const read = async (file) => JSON.parse(await readFile(path.join(dir, file), "utf8"));
  return {
    clash: clashFile ? await read(clashFile) : null,
  };
}

function selectClashNodes(preflight) {
  const ok = (preflight?.results || []).filter((item) => item.status === "ok" && item.nodeName);
  const selected = {};
  for (const key of ["us", "jp"]) {
    const list = ok.filter((item) => String(item.regionExpected || "").toLowerCase() === key);
    list.sort((a, b) => (a.elapsedMs || 999999) - (b.elapsedMs || 999999));
    if (list[0]) selected[key] = list[0];
  }
  return selected;
}

function buildCells(preflight) {
  const selectedNodes = selectClashNodes(preflight.clash);
  const cells = [];
  for (const cell of cellFilters) {
    const [submatrix, regionKey] = cell.split(":");
    if (submatrix === "clash" && ["us", "jp"].includes(regionKey) && selectedNodes[regionKey]) {
      cells.push({
        submatrix,
        proxyClass: "clash_airport",
        region: regions[regionKey],
        clashNodeName: selectedNodes[regionKey].nodeName,
        proxyMeta: {
          nodeNameHash: shortHash(selectedNodes[regionKey].nodeName),
          expectedCountryCode: selectedNodes[regionKey].countryCode || null,
          asn: selectedNodes[regionKey].asn || null,
          ipApiProxy: selectedNodes[regionKey].ipApiProxy ?? null,
          ipApiHosting: selectedNodes[regionKey].ipApiHosting ?? null,
          exitIpHash: selectedNodes[regionKey].exitIpHash || null,
        },
      });
    }
    if (submatrix === "udeal" && regionKey === "la") {
      cells.push({
        submatrix,
        proxyClass: "udeal_la_via_panda",
        region: regions.la,
        proxyMeta: {
          providerAlias: "udeal-la-via-panda-existing-local-bridge",
          localEndpoint: "127.0.0.1:18082",
          expectedCountryCode: "US",
        },
      });
    }
  }
  return cells;
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

async function findBitBrowserProfile(name) {
  const list = await httpJSON(`${endpoints.bitBrowser}/browser/list`, {
    method: "POST",
    body: JSON.stringify({ page: 0, pageSize: 200 }),
    timeoutMs: 45000,
  });
  const items = Array.isArray(list.body?.data?.list) ? list.body.data.list : [];
  return items.find((item) => item.name === name) || null;
}

async function createBitBrowserProfile(profileName, cell) {
  const port = cell.submatrix === "clash" ? 7897 : 18082;
  const region = cell.region;
  const payload = {
    platform: "https://browserleaks.com",
    platformIcon: "browserleaks.com",
    url: urls.ip,
    name: profileName,
    remark: `three-browser-benchmark missing-probe ${cell.proxyClass} ${region.key}`,
    userName: "",
    password: "",
    proxyMethod: 2,
    proxyType: "http",
    host: "127.0.0.1",
    port,
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
      language: region.key === "jp" ? "ja" : "en-US",
      timeZone: region.timezone,
    },
  };
  let updated = null;
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    updated = await httpJSON(`${endpoints.bitBrowser}/browser/update`, {
      method: "POST",
      body: JSON.stringify(payload),
      timeoutMs: 30000,
    });
    const message = String(updated.body?.msg || updated.body?.message || "");
    if (updated.body?.success || !message.includes("请求频繁")) break;
    await sleep(5000 * attempt);
  }
  if (!updated.body?.success) {
    throw new Error(`BitBrowser profile create failed: ${redact(JSON.stringify(updated.body))}`);
  }
  const profileId = updated.body?.data?.id || updated.body?.data?.browserId || updated.body?.id;
  if (!profileId) throw new Error("BitBrowser profile create returned no id");
  return { id: profileId, name: profileName, created: true };
}

async function resolveProfiles(cells) {
  const out = [];
  for (const cell of cells) {
    for (const product of products) {
      const profileName = matrixProfileName(product, cell);
      let found = product === "personalPilot"
        ? await findPersonalPilotProfile(profileName)
        : await findBitBrowserProfile(profileName);
      if (!found && product === "bitBrowser") {
        found = await createBitBrowserProfile(profileName, cell);
        await sleep(3000);
      }
      const profileId = found?.profileId || found?.id || "";
      out.push({ cell, product, profileName, profileId, found: Boolean(profileId), created: Boolean(found?.created) });
    }
  }
  return out;
}

async function openPersonalPilot(profileId) {
  const startedAt = Date.now();
  const opened = await httpJSON(`${endpoints.personalPilot}/api/launch`, {
    method: "POST",
    headers: localHeaders(),
    body: JSON.stringify({ profileId, startUrls: [urls.ip], skipDefaultStartUrls: true }),
    timeoutMs: 90000,
  });
  return {
    ok: Boolean(opened.body?.ok),
    latencyMs: Date.now() - startedAt,
    debugPort: opened.body?.debugPort || null,
    error: redact(opened.body?.error || ""),
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
    latencyMs: Date.now() - startedAt,
    debugPort: Number.parseInt(String(httpDebug).split(":").at(-1), 10) || null,
    error: redact(opened.body?.msg || opened.body?.message || ""),
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

async function connectFirstPage(debugPort) {
  const base = `http://127.0.0.1:${debugPort}`;
  const version = await httpJSON(`${base}/json/version`, { timeoutMs: 10000 });
  const tabs = await httpJSON(`${base}/json/list`, { timeoutMs: 10000 });
  const list = Array.isArray(tabs.body) ? tabs.body : [];
  const page = list.find((tab) => tab.type === "page" && tab.webSocketDebuggerUrl)
    || list.find((tab) => tab.webSocketDebuggerUrl);
  if (!page?.webSocketDebuggerUrl) throw new Error("No CDP page target found");
  const client = await CdpClient.connect(page.webSocketDebuggerUrl);
  await client.send("Page.enable").catch(() => null);
  await client.send("Runtime.enable").catch(() => null);
  await client.send("Network.enable").catch(() => null);
  return {
    client,
    cdpVersion: {
      ok: version.ok,
      browser: version.body?.Browser || null,
      protocolVersion: version.body?.["Protocol-Version"] || null,
    },
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

async function navigate(client, url, waitMs = 3000, timeoutMs = 30000) {
  const before = client.networkEvents.length;
  const startedAt = Date.now();
  await client.send("Page.navigate", { url }, timeoutMs);
  const ready = await waitForReadyState(client, 25000);
  if (waitMs > 0) await sleep(waitMs);
  const page = await evaluateValue(client, `(() => {
    const text = document.body ? document.body.innerText || "" : "";
    const html = document.documentElement ? document.documentElement.outerHTML || "" : "";
    return {
      url: location.href,
      title: document.title,
      readyState: document.readyState,
      textLength: text.length,
      ipCandidates: Array.from(new Set(text.match(/\\b(?:\\d{1,3}\\.){3}\\d{1,3}\\b|\\b(?:[a-f0-9]{0,4}:){2,}[a-f0-9]{0,4}\\b/gi) || [])).slice(0, 30),
      textSampleRedacted: text.slice(0, 1000),
      textHashInput: text.slice(0, 50000),
      htmlLength: html.length
    };
  })()`, 20000).catch((error) => ({ error: normalizeError(error) }));
  const networkEvents = client.networkEvents.slice(before).map((event) => ({
    method: event.method,
    urlHash: event.params?.request?.url ? shortHash(event.params.request.url) : null,
    status: event.params?.response?.status || null,
    protocol: event.params?.response?.protocol || null,
    errorText: redact(event.params?.errorText || ""),
  }));
  return {
    ok: ready && !String(page?.url || "").startsWith("chrome-error:") && !page?.error,
    latencyMs: Date.now() - startedAt,
    url: redact(page?.url || ""),
    title: redact(page?.title || ""),
    textLength: page?.textLength || 0,
    textHash: shortHash(page?.textHashInput || ""),
    textSampleRedacted: redact(page?.textSampleRedacted || ""),
    ipHashes: Array.isArray(page?.ipCandidates) ? page.ipCandidates.map(hashId) : [],
    htmlLength: page?.htmlLength || 0,
    networkEvents,
  };
}

async function runJsonPageProbe(client, url, waitMs) {
  const nav = await navigate(client, url, waitMs, 30000);
  const value = await evaluateValue(client, `(() => {
    const text = document.body ? document.body.innerText.trim() : "";
    try { return JSON.parse(text); }
    catch (error) { return { parseError: String(error && error.message || error), text: text.slice(0, 500) }; }
  })()`).catch((error) => ({ error: normalizeError(error) }));
  return { nav, value };
}

async function runFingerprintProbe(client) {
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
    function canvasHash() {
      const canvas = document.createElement("canvas");
      canvas.width = 320;
      canvas.height = 96;
      const ctx = canvas.getContext("2d");
      ctx.textBaseline = "top";
      ctx.font = "16px Arial";
      ctx.fillStyle = "#f60";
      ctx.fillRect(10, 10, 120, 35);
      ctx.fillStyle = "#069";
      ctx.fillText("Missing probe 字形 123", 15, 18);
      return hash(canvas.toDataURL());
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
          extensionsHash: hash((gl.getSupportedExtensions() || []).join("|"))
        };
      } catch (error) {
        return { error: String(error && error.message || error) };
      }
    }
    const canvasSamples = Array.from({ length: 5 }, () => canvasHash());
    return {
      url: location.href,
      userAgent: navigator.userAgent,
      webdriver: navigator.webdriver,
      language: navigator.language,
      languages: Array.from(navigator.languages || []),
      platform: navigator.platform,
      hardwareConcurrency: navigator.hardwareConcurrency,
      deviceMemory: navigator.deviceMemory || null,
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
      screen: {
        width: screen.width,
        height: screen.height,
        availWidth: screen.availWidth,
        availHeight: screen.availHeight,
        devicePixelRatio,
      },
      pluginsLength: navigator.plugins ? navigator.plugins.length : null,
      mimeTypesLength: navigator.mimeTypes ? navigator.mimeTypes.length : null,
      webgpuAvailable: Boolean(navigator.gpu),
      webgl: webglInfo(),
      canvasSamples,
      canvasStableInSession: new Set(canvasSamples).size === 1,
      clientRectsHash: hash(JSON.stringify(document.body ? document.body.getBoundingClientRect() : {})),
    };
  })()`, 20000);
  return {
    ...value,
    fingerprintHash: shortHash(JSON.stringify(value || {})),
  };
}

async function runIpProbe(client) {
  const { nav, value } = await runJsonPageProbe(client, urls.ip, 2000);
  const rawIp = value?.ip || "";
  if (value && typeof value === "object") delete value.ip;
  return {
    ok: Boolean(value?.success !== false && value?.country_code),
    nav,
    exitIpHash: rawIp ? hashId(rawIp) : null,
    data: value || null,
  };
}

async function runTransportProbe(client, fingerprint) {
  const { nav, value } = await runJsonPageProbe(client, urls.tls, 1000);
  const tls = value?.tls || {};
  const http2 = value?.http2 || {};
  const uaMajor = String(fingerprint?.userAgent || "").match(/Chrome\/(\d+)/)?.[1] || null;
  const browserMajor = String(fingerprint?.browser || "").match(/Chrome\/(\d+)/)?.[1] || null;
  return {
    nav,
    observed: {
      ok: Boolean(tls.ja3_hash || tls.ja4 || value?.http_version || http2.akamai_fingerprint),
      httpVersion: value?.http_version || value?.httpVersion || null,
      ja3Hash: tls.ja3_hash || null,
      ja4: tls.ja4 || null,
      tlsVersion: tls.tls_version_record || tls.version || null,
      http2AkamaiHash: http2.akamai_fingerprint ? shortHash(http2.akamai_fingerprint) : null,
      sentFramesHash: http2.sent_frames ? shortHash(JSON.stringify(http2.sent_frames)) : null,
      uaMajor,
      browserMajor,
      uaBrowserMajorMatch: uaMajor && browserMajor ? uaMajor === browserMajor : null,
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
  }))`, 12000).catch((error) => ({ available: false, candidates: [], error: normalizeError(error) }));
  const candidates = Array.isArray(value?.candidates) ? value.candidates : [];
  return {
    available: Boolean(value?.available),
    error: redact(value?.error || ""),
    candidateCount: candidates.length,
    candidateHashes: candidates.map(shortHash),
    addressHashes: candidates.flatMap(extractIps).map(hashId),
  };
}

function extractIps(text) {
  return String(text || "").match(/\b(?:\d{1,3}\.){3}\d{1,3}\b|\b(?:[a-f0-9]{0,4}:){2,}[a-f0-9]{0,4}\b/gi) || [];
}

async function runTextProbe(client, id, url, waitMs) {
  const nav = await navigate(client, url, waitMs, Math.max(30000, waitMs + 20000)).catch((error) => ({
    ok: false,
    error: normalizeError(error),
    url,
    textLength: 0,
    textHash: null,
    textSampleRedacted: "",
    networkEvents: [],
  }));
  return {
    id,
    ok: Boolean(nav.ok),
    url,
    finalUrl: nav.url || "",
    error: nav.error || "",
    textLength: nav.textLength || 0,
    textHash: nav.textHash || null,
    textSampleRedacted: redact(nav.textSampleRedacted || ""),
    ipHashes: nav.ipHashes || [],
    parsed: id === "creepjs" ? parseCreepText(nav.textSampleRedacted || "") : {},
    network: {
      requestCount: nav.networkEvents?.filter((event) => event.method === "Network.requestWillBeSent").length || 0,
      responseCount: nav.networkEvents?.filter((event) => event.method === "Network.responseReceived").length || 0,
      failureCount: nav.networkEvents?.filter((event) => event.method === "Network.loadingFailed").length || 0,
    },
  };
}

function parseCreepText(text) {
  const normalized = String(text || "").replace(/\s+/g, " ");
  const trust = normalized.match(/trust[^0-9]{0,30}(\d{1,3})\s*%?/i)?.[1] || null;
  const lies = normalized.match(/lies?[^0-9]{0,30}(\d{1,4})/i)?.[1] || null;
  return {
    trustScoreCandidate: trust ? Number(trust) : null,
    liesCandidate: lies ? Number(lies) : null,
    parsedFromSample: Boolean(trust || lies),
  };
}

function loadPreviousFingerprint(product, cell) {
  const base = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "deep-matrix", "deep-1783482661915", "raw");
  const file = `${productLabels[product]}_${cell.submatrix}_${cell.region.key}`;
  const match = readdirSyncSafe(base).find((name) => name.includes(file) && name.endsWith(".json"));
  if (!match) return null;
  try {
    const json = JSON.parse(spawnSync("node", ["-e", `process.stdout.write(require('fs').readFileSync(${JSON.stringify(path.join(base, match))}, 'utf8'))`], { encoding: "utf8" }).stdout);
    return json.deep?.finalFingerprint || json.deep?.initialFingerprint || null;
  } catch {
    return null;
  }
}

function readdirSyncSafe(dir) {
  try {
    return spawnSync("powershell", ["-NoProfile", "-Command", `Get-ChildItem -LiteralPath ${JSON.stringify(dir)} -File | ForEach-Object { $_.Name }`], { encoding: "utf8" })
      .stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  } catch {
    return [];
  }
}

function compareFingerprint(previous, current) {
  if (!previous || !current) return { compared: false };
  return {
    compared: true,
    userAgentSame: previous.userAgent === current.userAgent,
    languageSame: previous.language === current.language,
    timezoneSame: previous.timezone === current.timezone,
    canvasSame: previous.canvasHash === current.canvasSamples?.[0],
    webglRendererSame: previous.webgl?.renderer === current.webgl?.renderer,
    clientRectsSame: previous.clientRectsHash === current.clientRectsHash,
    previousHash: previous.fingerprintHash || null,
    currentHash: current.fingerprintHash || null,
  };
}

async function runProbe(item, clashSecret) {
  const { product, profileId, profileName, cell } = item;
  const launchKey = `${productLabels[product]}_${cell.submatrix}_${cell.region.key}`;
  const result = {
    schema: "two_browser_missing_probe_v1",
    generatedAt: new Date().toISOString(),
    product,
    profileName,
    profileIdHash: shortHash(profileId),
    submatrix: cell.submatrix,
    proxyClass: cell.proxyClass,
    region: cell.region.key,
    proxyMeta: cell.proxyMeta,
    status: "pending",
  };
  let client = null;
  try {
    if (cell.submatrix === "clash") {
      await setClashMode(clashSecret, "global");
      await setClashGlobal(clashSecret, cell.clashNodeName);
      result.clashNodeHash = shortHash(cell.clashNodeName);
    }
    const opened = product === "personalPilot" ? await openPersonalPilot(profileId) : await openBitBrowser(profileId);
    result.open = {
      ok: opened.ok,
      latencyMs: opened.latencyMs,
      debugPortPresent: Boolean(opened.debugPort),
      error: opened.error,
    };
    if (!opened.ok || !opened.debugPort) {
      result.status = "open_failed";
      return result;
    }
    const connected = await connectFirstPage(opened.debugPort);
    client = connected.client;
    result.cdpVersion = connected.cdpVersion;
    result.ip = await runIpProbe(client);
    result.fingerprint = await runFingerprintProbe(client);
    result.fingerprint.browser = connected.cdpVersion.browser;
    result.fingerprintComparison = compareFingerprint(loadPreviousFingerprint(product, cell), result.fingerprint);
    result.transport = await runTransportProbe(client, result.fingerprint);
    result.webrtc = await runWebRtcProbe(client);
    result.creepjs = await runTextProbe(client, "creepjs", urls.creepjs, 15000);
    await sleep(rateLimitMs);
    result.browserLeaksDns = await runTextProbe(client, "browserleaks-dns", urls.browserLeaksDns, 12000);
    await sleep(rateLimitMs);
    result.browserLeaksWebrtc = await runTextProbe(client, "browserleaks-webrtc", urls.browserLeaksWebrtc, 9000);
    result.geoTriangle = {
      expectedCountryCode: cell.region.expectedCountryCode,
      ipCountryCode: result.ip?.data?.country_code || null,
      ipTimezone: result.ip?.data?.timezone?.id || null,
      jsTimezone: result.fingerprint?.timezone || null,
      jsLanguage: result.fingerprint?.language || null,
      countryMatches: result.ip?.data?.country_code === cell.region.expectedCountryCode,
      timezoneMatches: result.ip?.data?.timezone?.id === result.fingerprint?.timezone,
    };
    result.status = "ok";
  } catch (error) {
    result.status = "error";
    result.error = normalizeError(error);
  } finally {
    if (client) client.close();
    try {
      result.close = product === "personalPilot" ? await closePersonalPilot(profileId) : await closeBitBrowser(profileId);
    } catch (error) {
      result.close = { ok: false, error: normalizeError(error) };
    }
  }
  await writeJSON(path.join(baseOutputDir, "raw", `${safeFileName(launchKey)}_${result.status}.json`), result);
  return result;
}

function summarize(results, config, blockers) {
  const byProduct = {};
  const missingStill = new Set([
    "AdsPower API raw artifact",
    "Germany same-class proxy node",
    "authoritative DNS-token proof",
    "10-run cross-session fingerprint drift",
    "structured CreepJS trust/lies if sample parser cannot extract it",
    "Firefox/Camoufox dual-engine benchmark",
    "OS SendInput Unicode proof",
  ]);
  for (const result of results) {
    byProduct[result.product] ||= {
      total: 0,
      ok: 0,
      openFailed: 0,
      countryMatches: 0,
      timezoneMatches: 0,
      webrtcNoCandidate: 0,
      tlsObserved: 0,
      uaBrowserMajorMatch: 0,
      canvasStableInSession: 0,
      creepParsed: 0,
      browserLeaksDnsObserved: 0,
    };
    const item = byProduct[result.product];
    item.total += 1;
    if (result.status === "ok") item.ok += 1;
    if (result.status === "open_failed") item.openFailed += 1;
    if (result.geoTriangle?.countryMatches) item.countryMatches += 1;
    if (result.geoTriangle?.timezoneMatches) item.timezoneMatches += 1;
    if (result.webrtc?.candidateCount === 0) item.webrtcNoCandidate += 1;
    if (result.transport?.observed?.ok) item.tlsObserved += 1;
    if (result.transport?.observed?.uaBrowserMajorMatch) item.uaBrowserMajorMatch += 1;
    if (result.fingerprint?.canvasStableInSession) item.canvasStableInSession += 1;
    if (result.creepjs?.parsed?.parsedFromSample) item.creepParsed += 1;
    if (result.browserLeaksDns?.ok && result.browserLeaksDns?.textLength > 0) item.browserLeaksDnsObserved += 1;
  }
  return {
    schema: "two_browser_missing_probe_summary_v1",
    generatedAt: new Date().toISOString(),
    outputDir: path.relative(projectRoot, baseOutputDir),
    config,
    blockers,
    completed: results.length,
    byProduct,
    missingStill: Array.from(missingStill),
  };
}

async function writeJSON(file, value) {
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, JSON.stringify(value, null, 2), "utf8");
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

async function main() {
  if (!Number.isFinite(maxBitBrowserOpens) || maxBitBrowserOpens < 0) throw new Error("--max-bitbrowser-opens must be >= 0");
  await mkdir(path.join(baseOutputDir, "raw"), { recursive: true });
  const blockers = [];
  const ports = {
    personalPilot: products.includes("personalPilot") ? await tcpCheck("127.0.0.1", 19876) : null,
    bitBrowser: products.includes("bitBrowser") ? await tcpCheck("127.0.0.1", 54345) : null,
    clashMixed: cellFilters.some((cell) => cell.startsWith("clash:")) ? await tcpCheck("127.0.0.1", 7897) : null,
    udealBridgeHttp: cellFilters.includes("udeal:la") ? await tcpCheck("127.0.0.1", 18082) : null,
  };
  if (products.includes("personalPilot") && !personalPilotKey) blockers.push("PERSONAL_PILOT_API_KEY is required for PersonaPilot.");
  for (const [name, status] of Object.entries(ports)) {
    if (status && !status.ok) blockers.push(`${name} port is not reachable: ${status.error}`);
  }
  const preflight = await loadLatestProxyPreflight();
  const cells = buildCells(preflight);
  let clashSecret = "";
  let clashOriginal = null;
  if (cellFilters.some((cell) => cell.startsWith("clash:"))) {
    clashSecret = readClashSecret();
    if (!clashSecret) blockers.push("Clash controller secret was not found.");
    else clashOriginal = await getClashState(clashSecret).catch((error) => {
      blockers.push(`Clash controller state read failed: ${normalizeError(error)}`);
      return null;
    });
  }
  const profiles = await resolveProfiles(cells);
  for (const profile of profiles) {
    if (!profile.found) blockers.push(`Missing existing profile: ${profile.product} ${profile.profileName}`);
  }
  const bitBrowserPlannedOpens = profiles.filter((profile) => profile.product === "bitBrowser" && profile.found).length;
  if (bitBrowserPlannedOpens > maxBitBrowserOpens) {
    blockers.push(`BitBrowser planned opens ${bitBrowserPlannedOpens} exceeds --max-bitbrowser-opens=${maxBitBrowserOpens}`);
  }
  const config = {
    products,
    cellFilters,
    maxBitBrowserOpens,
    bitBrowserPlannedOpens,
    rateLimitMs,
    ports,
    cells: cells.map((cell) => ({
      submatrix: cell.submatrix,
      region: cell.region.key,
      proxyClass: cell.proxyClass,
      proxyMeta: cell.proxyMeta,
      clashNodeHash: cell.clashNodeName ? shortHash(cell.clashNodeName) : null,
    })),
    clashController: clashOriginal ? {
      endpoint: "127.0.0.1:9097",
      secretPresent: Boolean(clashSecret),
      originalMode: clashOriginal.mode,
      originalGlobalHash: clashOriginal.globalNow ? shortHash(clashOriginal.globalNow) : null,
    } : null,
    probes: {
      creepjs: "text sample parser only; trust/lies may remain unparsed",
      dns: "browserleaks DNS page, not authoritative DNS-token proof",
      webrtc: "CDP RTCPeerConnection candidate probe plus BrowserLeaks text hash",
      tls: "tls.peet.ws JA3/JA4/H2 summary",
      drift: "compares against previous deep finalFingerprint when available; not a 10-run drift gate",
    },
  };
  await writeJSON(path.join(baseOutputDir, "config.redacted.json"), config);
  await writeJSON(path.join(baseOutputDir, "profiles.redacted.json"), profiles.map((profile) => ({
    product: profile.product,
    profileName: profile.profileName,
    profileIdHash: profile.profileId ? shortHash(profile.profileId) : null,
    found: profile.found,
    created: profile.created,
    submatrix: profile.cell.submatrix,
    region: profile.cell.region.key,
    proxyClass: profile.cell.proxyClass,
  })));
  const results = [];
  if (!blockers.length) {
    try {
      for (const profile of profiles) {
        const result = await runProbe(profile, clashSecret);
        results.push(result);
        console.log(JSON.stringify({
          product: result.product,
          cell: `${result.submatrix}:${result.region}`,
          status: result.status,
          country: result.geoTriangle?.countryMatches,
          timezone: result.geoTriangle?.timezoneMatches,
          tls: result.transport?.observed?.ok,
          uaBrowserMajorMatch: result.transport?.observed?.uaBrowserMajorMatch,
          webrtcCandidates: result.webrtc?.candidateCount,
          canvasStable: result.fingerprint?.canvasStableInSession,
          creepParsed: result.creepjs?.parsed?.parsedFromSample,
          dnsObserved: Boolean(result.browserLeaksDns?.ok && result.browserLeaksDns?.textLength > 0),
        }));
        await sleep(rateLimitMs);
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
  }
  const summary = summarize(results, config, blockers);
  await writeJSON(path.join(baseOutputDir, "summary.json"), summary);
  console.log(JSON.stringify({ ok: blockers.length === 0, outputDir: baseOutputDir, blockers, byProduct: summary.byProduct }, null, 2));
  if (blockers.length) process.exitCode = 1;
}

main().catch(async (error) => {
  console.error(normalizeError(error));
  try {
    await mkdir(baseOutputDir, { recursive: true });
    await writeFile(path.join(baseOutputDir, "fatal-error.txt"), normalizeError(error), "utf8");
  } catch {}
  process.exit(1);
});
