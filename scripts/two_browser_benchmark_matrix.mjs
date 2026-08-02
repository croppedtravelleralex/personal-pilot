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
const targetUrl = "https://browserleaks.com/ip";
const ipProbeUrl = "https://ipwho.is/?fields=success,message,ip,country,country_code,region,city,timezone,connection";
const baseOutputDir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "matrix", `matrix-${stamp}`);
const products = parseListArg("products", ["personalPilot", "bitBrowser"]).map(normalizeProduct).filter(Boolean);
const submatrices = parseListArg("submatrices", ["clash", "udeal"]).map((value) => value.toLowerCase());
const launchesPerCell = Number.parseInt(argValue("launches", "10"), 10);
const smoke = args.includes("--smoke");
const writeRaw = !args.includes("--no-raw");

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
    label: "US",
    locale: "en-US",
    acceptLanguage: "en-US,en;q=0.9",
    timezone: "America/Los_Angeles",
    expectedCountryCode: "US",
  },
  jp: {
    key: "jp",
    label: "JP",
    locale: "ja-JP",
    acceptLanguage: "ja-JP,ja;q=0.9,en;q=0.8",
    timezone: "Asia/Tokyo",
    expectedCountryCode: "JP",
  },
  la: {
    key: "la",
    label: "LA",
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
    .replace(/("?(?:password|token|secret|api[_-]?key|authorization|cookie)"?\s*[:=]\s*)["']?[^"',\s}]+/gi, "$1***");
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
  const response = await fetch(url, {
    ...fetchOptions,
    signal: controller.signal,
    headers: {
      Accept: "application/json",
      ...(fetchOptions.body ? { "Content-Type": "application/json" } : {}),
      ...(fetchOptions.headers || {}),
    },
  }).finally(() => clearTimeout(timer));
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

function uniqueStrings(values) {
  return [...new Set(values.filter(Boolean))];
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
    clashFile: clashFile ? path.join("data", "reports", "three-browser-benchmark", "proxy-preflight", clashFile) : null,
    udealFile: udealFile ? path.join("data", "reports", "three-browser-benchmark", "proxy-preflight", udealFile) : null,
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

async function ensurePersonalPilotProfile(cell) {
  const profileName = matrixProfileName("personalPilot", cell);
  const existing = await findPersonalPilotProfile(profileName);
  const payload = {
    profile: buildPersonalPilotProfile(profileName, cell),
    launchCode: "",
    autoLaunch: false,
  };
  if (existing?.profileId) {
    const updated = await httpJSON(`${endpoints.personalPilot}/api/profiles/${existing.profileId}`, {
      method: "PUT",
      headers: localHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 30000,
    });
    if (!updated.body?.ok) throw new Error(`PersonalPilot profile update failed: ${redact(JSON.stringify(updated.body))}`);
    return {
      product: "personalPilot",
      profileName,
      profileId: existing.profileId,
      reused: true,
    };
  }
  const created = await httpJSON(`${endpoints.personalPilot}/api/profiles`, {
    method: "POST",
    headers: localHeaders(),
    body: JSON.stringify(payload),
    timeoutMs: 30000,
  });
  if (!created.body?.ok || !created.body?.profileId) {
    throw new Error(`PersonalPilot profile create failed: ${redact(JSON.stringify(created.body))}`);
  }
  return {
    product: "personalPilot",
    profileName,
    profileId: created.body.profileId,
    reused: false,
  };
}

async function findPersonalPilotProfile(name) {
  const profiles = await httpJSON(`${endpoints.personalPilot}/api/profiles`, {
    headers: localHeaders(),
    timeoutMs: 15000,
  });
  const items = Array.isArray(profiles.body?.items) ? profiles.body.items : [];
  return items.find((item) => item.profileName === name) || null;
}

function matrixProfileName(product, cell) {
  return `benchmark-matrix-${productLabels[product]}-${cell.proxyClass}-${cell.region.key}`;
}

function buildPersonalPilotProfile(profileName, cell) {
  const region = cell.region;
  const proxyConfig = cell.proxyServer;
  return {
    profileName,
    coreId: "core-fingerprint-chromium-139-0-7258-154",
    proxyConfig,
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
    tags: ["three-browser-benchmark", "matrix", `proxy:${cell.proxyClass}`, `region:${region.key}`],
  };
}

async function ensureBitBrowserProfile(cell) {
  const profileName = matrixProfileName("bitBrowser", cell);
  const existing = await findBitBrowserProfile(profileName);
  const payload = buildBitBrowserProfile(profileName, cell, existing?.id);
  const updated = await httpJSON(`${endpoints.bitBrowser}/browser/update`, {
    method: "POST",
    body: JSON.stringify(payload),
    timeoutMs: 30000,
  });
  if (!updated.body?.success) {
    throw new Error(`BitBrowser profile update failed: ${redact(JSON.stringify(updated.body))}`);
  }
  const profileId = updated.body?.data?.id || updated.body?.data?.browserId || updated.body?.id || existing?.id;
  if (!profileId) throw new Error("BitBrowser profile update returned no id");
  return {
    product: "bitBrowser",
    profileName,
    profileId,
    reused: Boolean(existing?.id),
  };
}

async function findBitBrowserProfile(name) {
  const list = await httpJSON(`${endpoints.bitBrowser}/browser/list`, {
    method: "POST",
    body: JSON.stringify({ page: 0, pageSize: 200 }),
    timeoutMs: 15000,
  });
  const items = Array.isArray(list.body?.data?.list) ? list.body.data.list : [];
  return items.find((item) => item.name === name) || null;
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
    remark: `three-browser-benchmark matrix ${cell.proxyClass} ${region.key}`,
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
    body: JSON.stringify({
      profileId,
      startUrls: [targetUrl],
      skipDefaultStartUrls: true,
    }),
    timeoutMs: 90000,
  });
  const latencyMs = Date.now() - startedAt;
  return {
    ok: Boolean(opened.body?.ok),
    status: opened.status,
    latencyMs,
    debugPort: opened.body?.debugPort || null,
    pid: opened.body?.pid || null,
    browserVersion: null,
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
  const latencyMs = Date.now() - startedAt;
  const httpDebug = opened.body?.data?.http || "";
  const debugPort = Number.parseInt(String(httpDebug).split(":").at(-1), 10) || null;
  return {
    ok: Boolean(opened.body?.success),
    status: opened.status,
    latencyMs,
    debugPort,
    pid: opened.body?.data?.pid || null,
    browserVersion: opened.body?.data?.coreVersion || null,
    wsPresent: Boolean(opened.body?.data?.ws),
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

async function cdpSnapshot(debugPort) {
  const base = `http://127.0.0.1:${debugPort}`;
  const out = {};
  try {
    const version = await httpJSON(`${base}/json/version`, { timeoutMs: 10000 });
    out.versionOk = version.ok;
    out.browser = version.body?.Browser || null;
    out.protocolVersion = version.body?.["Protocol-Version"] || null;
  } catch (error) {
    out.versionError = normalizeError(error);
  }
  try {
    const tabs = await httpJSON(`${base}/json/list`, { timeoutMs: 10000 });
    const list = Array.isArray(tabs.body) ? tabs.body : [];
    out.tabs = list.slice(0, 5).map((tab) => ({
      type: tab.type || null,
      title: redact(tab.title || ""),
      url: redact(tab.url || ""),
    }));
    const page = list.find((tab) => tab.type === "page" && tab.url === targetUrl && tab.webSocketDebuggerUrl)
      || list.find((tab) => tab.type === "page" && String(tab.url || "").includes("browserleaks.com/ip") && tab.webSocketDebuggerUrl)
      || list.find((tab) => tab.type === "page" && tab.webSocketDebuggerUrl)
      || list.find((tab) => tab.webSocketDebuggerUrl);
    if (page?.webSocketDebuggerUrl) {
      out.page = await runCdpPageProbe(page.webSocketDebuggerUrl);
    }
  } catch (error) {
    out.tabsError = normalizeError(error);
  }
  return out;
}

async function runCdpPageProbe(wsUrl) {
  const client = await CdpClient.connect(wsUrl);
  try {
    await client.send("Page.enable").catch(() => null);
    await client.send("Runtime.enable").catch(() => null);
    await sleep(2500);
    const initialProbe = await evaluateProbe(client);
    let ipApi = null;
    try {
      await client.send("Page.navigate", { url: ipProbeUrl });
      await waitForReadyState(client, 12000);
      ipApi = await evaluateIpApiPage(client);
    } catch (error) {
      ipApi = { ok: false, error: normalizeError(error) };
    }
    const finalProbe = await evaluateProbe(client).catch((error) => ({ error: normalizeError(error) }));
    return {
      initialProbe,
      ipApi,
      finalProbe,
      fingerprintHash: shortHash(JSON.stringify(finalProbe?.fingerprint || initialProbe?.fingerprint || {})),
    };
  } finally {
    client.close();
  }
}

async function waitForReadyState(client, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await client.send("Runtime.evaluate", {
      expression: "document.readyState",
      returnByValue: true,
    }).catch(() => null);
    if (result?.result?.value === "complete" || result?.result?.value === "interactive") return;
    await sleep(500);
  }
}

async function evaluateIpApiPage(client) {
  const result = await client.send("Runtime.evaluate", {
    expression: `(() => {
      const text = document.body ? document.body.innerText.trim() : "";
      try { return { ok: true, data: JSON.parse(text) }; }
      catch (error) { return { ok: false, text: text.slice(0, 500), error: String(error && error.message || error) }; }
    })()`,
    returnByValue: true,
  });
  const value = result?.result?.value || {};
  if (!value.ok || !value.data) return value;
  const data = value.data;
  const rawIp = data.ip || data.query || "";
  delete data.ip;
  delete data.query;
  return {
    ok: Boolean(data.success !== false && (data.country_code || data.countryCode)),
    exitIpHash: rawIp ? hashId(rawIp) : null,
    data,
  };
}

async function evaluateProbe(client) {
  const expression = `(() => {
    function shortHash(input) {
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
        };
      } catch (error) {
        return { error: String(error && error.message || error) };
      }
    }
    function canvasHash() {
      try {
        const canvas = document.createElement("canvas");
        canvas.width = 280;
        canvas.height = 80;
        const ctx = canvas.getContext("2d");
        ctx.textBaseline = "top";
        ctx.font = "16px Arial";
        ctx.fillStyle = "#f60";
        ctx.fillRect(10, 10, 120, 35);
        ctx.fillStyle = "#069";
        ctx.fillText("PersonaPilot benchmark 123", 15, 18);
        return shortHash(canvas.toDataURL());
      } catch (error) {
        return "error:" + String(error && error.message || error);
      }
    }
    return {
      url: location.href,
      title: document.title,
      readyState: document.readyState,
      fingerprint: {
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
          colorDepth: screen.colorDepth,
          pixelDepth: screen.pixelDepth,
          devicePixelRatio,
        },
        pluginsLength: navigator.plugins ? navigator.plugins.length : null,
        mimeTypesLength: navigator.mimeTypes ? navigator.mimeTypes.length : null,
        webgl: webglInfo(),
        canvasHash: canvasHash(),
      },
    };
  })()`;
  const result = await client.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  return result?.result?.value || null;
}

class CdpClient {
  constructor(ws) {
    this.ws = ws;
    this.nextId = 1;
    this.pending = new Map();
    ws.addEventListener("message", (event) => {
      let msg = null;
      try {
        msg = JSON.parse(event.data);
      } catch {
        return;
      }
      if (!msg.id || !this.pending.has(msg.id)) return;
      const { resolve, reject, timer } = this.pending.get(msg.id);
      clearTimeout(timer);
      this.pending.delete(msg.id);
      if (msg.error) reject(new Error(msg.error.message || JSON.stringify(msg.error)));
      else resolve(msg.result || {});
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
  if (submatrices.includes("udeal")) {
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
  const result = {
    schema: "two_browser_benchmark_launch_v1",
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
      browserVersion: opened.browserVersion || null,
      error: redact(opened.rawError || ""),
    };
    if (!opened.ok || !opened.debugPort) {
      result.status = "open_failed";
      return result;
    }
    result.cdp = await cdpSnapshot(opened.debugPort);
    const ipCountry = result.cdp?.page?.ipApi?.data?.countryCode || result.cdp?.page?.ipApi?.data?.country_code || null;
    result.countryMatchesExpected = ipCountry ? ipCountry === cell.region.expectedCountryCode : null;
    result.status = result.cdp?.versionOk ? "ok" : "cdp_failed";
  } catch (error) {
    result.status = "error";
    result.error = normalizeError(error);
  } finally {
    try {
      const closed = profile.product === "personalPilot"
        ? await closePersonalPilot(profile.profileId)
        : await closeBitBrowser(profile.profileId);
      result.close = closed;
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
    byProduct[product] ||= { total: 0, ok: 0, failed: 0, countriesMatched: 0, countriesObserved: 0, openLatencyMs: [] };
    byCell[cellKey] ||= { total: 0, ok: 0, failed: 0, products: {} };
    const p = byProduct[product];
    const c = byCell[cellKey];
    p.total += 1;
    c.total += 1;
    c.products[product] ||= { total: 0, ok: 0, failed: 0 };
    c.products[product].total += 1;
    if (result.status === "ok") {
      p.ok += 1;
      c.ok += 1;
      c.products[product].ok += 1;
    } else {
      p.failed += 1;
      c.failed += 1;
      c.products[product].failed += 1;
    }
    if (typeof result.countryMatchesExpected === "boolean") {
      p.countriesObserved += 1;
      if (result.countryMatchesExpected) p.countriesMatched += 1;
    }
    if (Number.isFinite(result.open?.latencyMs)) p.openLatencyMs.push(result.open.latencyMs);
  }
  for (const item of Object.values(byProduct)) {
    item.openLatencyP50Ms = percentile(item.openLatencyMs, 50);
    item.openLatencyP95Ms = percentile(item.openLatencyMs, 95);
    delete item.openLatencyMs;
  }
  return {
    schema: "two_browser_benchmark_matrix_summary_v1",
    generatedAt: new Date().toISOString(),
    outputDir: path.relative(projectRoot, baseOutputDir),
    config,
    plannedLaunches: prepared.length * config.launchesPerCell,
    completedLaunches: results.length,
    blockers,
    byProduct,
    byCell,
    fingerprintHashes: summarizeFingerprintHashes(results),
  };
}

function summarizeFingerprintHashes(results) {
  const out = {};
  for (const result of results) {
    const key = `${result.product}:${result.submatrix}:${result.region}`;
    out[key] ||= { total: 0, hashes: {} };
    const hash = result.cdp?.page?.fingerprintHash || "missing";
    out[key].total += 1;
    out[key].hashes[hash] = (out[key].hashes[hash] || 0) + 1;
  }
  return out;
}

function percentile(values, pct) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.min(sorted.length - 1, Math.ceil((pct / 100) * sorted.length) - 1);
  return sorted[idx];
}

async function writeScorecard(summary) {
  const lines = [
    "# PersonaPilot / BitBrowser Matrix Scorecard",
    "",
    `Generated: ${summary.generatedAt}`,
    "",
    `Planned launches: ${summary.plannedLaunches}`,
    `Completed launches: ${summary.completedLaunches}`,
    "",
    "## Products",
    "",
    "| Product | Total | OK | Failed | Country matched | P50 open ms | P95 open ms |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: |",
  ];
  for (const [product, item] of Object.entries(summary.byProduct)) {
    lines.push(`| ${product} | ${item.total} | ${item.ok} | ${item.failed} | ${item.countriesMatched}/${item.countriesObserved} | ${item.openLatencyP50Ms ?? ""} | ${item.openLatencyP95Ms ?? ""} |`);
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

async function main() {
  if (!Number.isFinite(launchesPerCell) || launchesPerCell < 1) throw new Error("--launches must be >= 1");
  await mkdir(path.join(baseOutputDir, "raw"), { recursive: true });
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

  const config = {
    products,
    submatrices,
    launchesPerCell: smoke ? 1 : launchesPerCell,
    targetUrl,
    ipProbe: "ipwho.is via browser page; raw ip removed, SHA256 hash retained",
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
  };
  await writeFile(path.join(baseOutputDir, "config.redacted.json"), JSON.stringify(config, null, 2), "utf8");

  let results = [];
  if (blockers.length === 0) {
    const prepared = await prepareProfiles(cells);
    await writeFile(path.join(baseOutputDir, "profiles.redacted.json"), JSON.stringify(prepared.map(({ cell, profile }) => ({
      product: profile.product,
      profileName: profile.profileName,
      profileIdHash: shortHash(profile.profileId),
      reused: profile.reused,
      submatrix: cell.submatrix,
      region: cell.region.key,
      proxyClass: cell.proxyClass,
    })), null, 2), "utf8");

    const launchCount = smoke ? 1 : launchesPerCell;
    try {
      for (let attempt = 1; attempt <= launchCount; attempt += 1) {
        for (const item of prepared) {
          const result = await runLaunch(item, attempt, clashSecret);
          results.push(result);
          if (writeRaw) {
            const rawName = [
              String(attempt).padStart(2, "0"),
              productLabels[result.product],
              result.submatrix,
              result.region,
              result.status,
            ].map(safeFileName).join("_");
            await writeFile(path.join(baseOutputDir, "raw", `${rawName}.json`), JSON.stringify(result, null, 2), "utf8");
          }
          console.log(JSON.stringify({
            attempt,
            product: result.product,
            submatrix: result.submatrix,
            region: result.region,
            status: result.status,
            countryMatchesExpected: result.countryMatchesExpected,
            openLatencyMs: result.open?.latencyMs ?? null,
          }));
          await sleep(800);
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
    await writeFile(path.join(baseOutputDir, "summary.json"), JSON.stringify(summary, null, 2), "utf8");
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
  await writeFile(path.join(baseOutputDir, "summary.json"), JSON.stringify(summary, null, 2), "utf8");
  await writeScorecard(summary);
  console.log(JSON.stringify({
    ok: false,
    outputDir: baseOutputDir,
    completedLaunches: 0,
    blockers,
  }, null, 2));
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
