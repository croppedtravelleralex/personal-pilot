import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";

const projectRoot = process.cwd();
const stamp = Date.now();
const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const productsArg = args.find((arg) => arg.startsWith("--products="))?.slice("--products=".length) || "";
const outputDir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "readiness");
const personalPilotKey = process.env.PERSONAL_PILOT_API_KEY || "";
const adsPowerKey = process.env.ADSPOWER_API_KEY || "";

const productAliases = {
  "personal-pilot": "personalPilot",
  personalpilot: "personalPilot",
  pp: "personalPilot",
  bitbrowser: "bitBrowser",
  "bit-browser": "bitBrowser",
  bit: "bitBrowser",
  adspower: "adsPower",
  "ads-power": "adsPower",
  ads: "adsPower",
};
const allProducts = ["personalPilot", "bitBrowser", "adsPower"];
const defaultProducts = ["personalPilot", "bitBrowser"];
const selectedProducts = productsArg
  ? [...new Set(productsArg.split(",")
      .map((value) => productAliases[value.trim().toLowerCase()] || value.trim())
      .filter(Boolean))]
  : defaultProducts;
const selectedProductSet = new Set(selectedProducts);

function productSelected(product) {
  return selectedProductSet.has(product);
}

const endpoints = {
  personalPilot: "http://127.0.0.1:19876",
  bitBrowser: "http://127.0.0.1:54345",
  adsPower: "http://127.0.0.1:50325",
};

function redact(value) {
  if (typeof value !== "string") return value;
  return value
    .replace(/(https?|socks5?|ssh):\/\/([^:@/\s]+):([^@/\s]+)@/gi, "$1://***:***@")
    .replace(/Bearer\s+[A-Za-z0-9._-]+/g, "Bearer ***")
    .replace(/("?(?:password|token|secret|api[_-]?key|authorization)"?\s*[:=]\s*)["']?[^"',\s}]+/gi, "$1***");
}

function hashId(value) {
  return createHash("sha256").update(String(value)).digest("hex").slice(0, 16);
}

function normalizeError(error) {
  if (!error) return "";
  return redact(error.message || String(error));
}

async function httpJSON(url, options = {}) {
  const timeoutMs = options.timeoutMs ?? 15000;
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
    body = redact(text.slice(0, 500));
  }
  return { ok: response.ok, status: response.status, body };
}

function localHeaders() {
  return personalPilotKey ? { "X-Personal-Pilot-Api-Key": personalPilotKey } : {};
}

function adsPowerHeaders() {
  return adsPowerKey ? { Authorization: `Bearer ${adsPowerKey}` } : {};
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

async function captureCdpSnapshot(debugBase, out) {
  try {
    const version = await httpJSON(`${debugBase}/json/version`);
    out.cdpVersionOk = version.ok;
    out.browser = version.body?.Browser || null;
    out.protocolVersion = version.body?.["Protocol-Version"] || null;
  } catch (error) {
    out.cdpVersionError = normalizeError(error);
  }
  try {
    const tabs = await httpJSON(`${debugBase}/json/list`);
    const list = Array.isArray(tabs.body) ? tabs.body : [];
    out.cdpTabs = list.slice(0, 5).map((tab) => ({
      type: tab.type || null,
      title: redact(tab.title || ""),
      url: redact(tab.url || ""),
    }));
    out.targetPageObserved = out.cdpTabs.some((tab) => String(tab.url).includes("browserleaks.com/ip"));
  } catch (error) {
    out.cdpTabsError = normalizeError(error);
  }
}

async function latestProxyPreflightSummary() {
  const dir = path.join(projectRoot, "data", "reports", "three-browser-benchmark", "proxy-preflight");
  try {
    const files = (await readdir(dir))
      .filter((name) => /^proxy-preflight-summary-.*\.json$/.test(name))
      .sort();
    if (files.length === 0) return { found: false };
    const file = files.at(-1);
    const raw = await readFile(path.join(dir, file), "utf8");
    const parsed = JSON.parse(raw);
    return {
      found: true,
      file: path.join("data", "reports", "three-browser-benchmark", "proxy-preflight", file),
      status: parsed.status || parsed.summary?.status || "unknown",
      proxyClasses: parsed.proxyClasses || parsed.summary?.proxyClasses || parsed.submatrices || null,
    };
  } catch (error) {
    return { found: false, error: normalizeError(error) };
  }
}

async function checkPersonalPilot() {
  const result = {
    endpoint: endpoints.personalPilot,
    tcp: await tcpCheck("127.0.0.1", 19876),
    health: null,
    profileCount: null,
    dryRun: null,
  };
  try {
    result.health = await httpJSON(`${endpoints.personalPilot}/api/health`, { headers: localHeaders() });
  } catch (error) {
    result.health = { ok: false, error: normalizeError(error) };
  }
  try {
    const profiles = await httpJSON(`${endpoints.personalPilot}/api/profiles`, { headers: localHeaders() });
    result.profileCount = profiles.body?.count ?? profiles.body?.items?.length ?? null;
  } catch (error) {
    result.profileCountError = normalizeError(error);
  }
  if (dryRun && result.health?.body?.ok) {
    result.dryRun = await runPersonalPilotDryRun();
  }
  return result;
}

async function runPersonalPilotDryRun() {
  const startedAt = Date.now();
  const profileName = `benchmark-dryrun-personal-pilot-udeal-la-${stamp}`;
  const payload = {
    profile: {
      profileName,
      coreId: "core-fingerprint-chromium-139-0-7258-154",
      proxyConfig: "http://127.0.0.1:18082",
      fingerprintArgs: [
        "--fingerprint-brand=Chrome",
        "--fingerprint-platform=windows",
        "--fingerprint-platform-version=10.0.0",
        "--lang=en-US",
        "--accept-lang=en-US,en;q=0.9",
        "--timezone=America/Los_Angeles",
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
        "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1",
      ],
      tags: ["three-browser-benchmark", "dry-run", "proxy:udeal-la"],
    },
    autoLaunch: true,
    start: {
      startUrls: ["https://browserleaks.com/ip"],
      skipDefaultStartUrls: true,
    },
  };
  const out = { profileName, proxyHash: hashId(payload.profile.proxyConfig) };
  try {
    const created = await httpJSON(`${endpoints.personalPilot}/api/profiles`, {
      method: "POST",
      headers: localHeaders(),
      body: JSON.stringify(payload),
    });
    out.createStatus = created.status;
    out.ok = Boolean(created.body?.ok);
    out.profileId = created.body?.profileId || null;
    out.launched = Boolean(created.body?.launched);
    out.debugPort = created.body?.debugPort || created.body?.profile?.debugPort || null;
    out.debugReady = Boolean(created.body?.debugReady || created.body?.profile?.debugReady);
    if (out.debugPort) {
      await captureCdpSnapshot(`http://127.0.0.1:${out.debugPort}`, out);
    }
    if (out.profileId) {
      try {
        const stopped = await httpJSON(`${endpoints.personalPilot}/api/instances/stop`, {
          method: "POST",
          headers: localHeaders(),
          body: JSON.stringify({ profileId: out.profileId }),
        });
        out.stopOk = Boolean(stopped.body?.ok);
        out.stopStatus = stopped.status;
      } catch (error) {
        out.stopError = normalizeError(error);
      }
    }
  } catch (error) {
    out.ok = false;
    out.error = normalizeError(error);
  }
  out.durationMs = Date.now() - startedAt;
  return out;
}

async function checkBitBrowser() {
  const result = {
    endpoint: endpoints.bitBrowser,
    tcp: await tcpCheck("127.0.0.1", 54345),
    profileCount: null,
    dryRun: null,
  };
  try {
    const list = await httpJSON(`${endpoints.bitBrowser}/browser/list`, {
      method: "POST",
      body: JSON.stringify({ page: 0, pageSize: 20 }),
    });
    result.listOk = Boolean(list.body?.success);
    result.profileCount = list.body?.data?.totalNum ?? null;
  } catch (error) {
    result.listError = normalizeError(error);
  }
  if (dryRun && result.listOk) {
    result.dryRun = await runBitBrowserDryRun();
  }
  return result;
}

async function runBitBrowserDryRun() {
  const startedAt = Date.now();
  const profileName = `benchmark-dryrun-bitbrowser-udeal-la-${stamp}`;
  const payload = {
    platform: "https://browserleaks.com",
    platformIcon: "browserleaks.com",
    url: "https://browserleaks.com/ip",
    name: profileName,
    remark: "three-browser-benchmark dry-run udeal-la",
    userName: "",
    password: "",
    proxyMethod: 2,
    proxyType: "http",
    host: "127.0.0.1",
    port: 18082,
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
    },
  };
  const out = { profileName, proxyHash: hashId(`${payload.proxyType}://${payload.host}:${payload.port}`) };
  try {
    const created = await httpJSON(`${endpoints.bitBrowser}/browser/update`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
    out.createSuccess = Boolean(created.body?.success);
    out.createStatus = created.status;
    out.profileId = created.body?.data?.id || created.body?.data?.browserId || created.body?.id || null;
    out.seq = created.body?.data?.seq || null;
    if (out.profileId) {
      const opened = await httpJSON(`${endpoints.bitBrowser}/browser/open`, {
        method: "POST",
        body: JSON.stringify({ id: out.profileId, queue: true }),
      });
      out.openSuccess = Boolean(opened.body?.success);
      out.httpDebug = opened.body?.data?.http || null;
      out.wsPresent = Boolean(opened.body?.data?.ws);
      out.coreVersion = opened.body?.data?.coreVersion || null;
      out.pid = opened.body?.data?.pid || null;
      if (out.httpDebug) {
        try {
          const debugBase = out.httpDebug.startsWith("http") ? out.httpDebug : `http://${out.httpDebug}`;
          await captureCdpSnapshot(debugBase, out);
        } catch (error) {
          out.cdpVersionError = normalizeError(error);
        }
      }
      try {
        const closed = await httpJSON(`${endpoints.bitBrowser}/browser/close`, {
          method: "POST",
          body: JSON.stringify({ id: out.profileId }),
        });
        out.closeSuccess = Boolean(closed.body?.success);
      } catch (error) {
        out.closeError = normalizeError(error);
      }
    } else {
      out.createMessage = redact(created.body?.msg || created.body?.message || "missing profile id");
    }
  } catch (error) {
    out.createSuccess = false;
    out.error = normalizeError(error);
  }
  out.durationMs = Date.now() - startedAt;
  return out;
}

async function checkAdsPower() {
  const result = {
    endpoint: endpoints.adsPower,
    tcp: await tcpCheck("127.0.0.1", 50325),
    status: null,
    profileList: null,
    apiKeyProvided: Boolean(adsPowerKey),
    dryRun: null,
  };
  try {
    result.status = await httpJSON(`${endpoints.adsPower}/status`);
  } catch (error) {
    result.status = { ok: false, error: normalizeError(error) };
  }
  try {
    const list = await httpJSON(`${endpoints.adsPower}/api/v1/user/list?page=1&page_size=10`, {
      headers: adsPowerHeaders(),
    });
    result.profileList = {
      httpOk: list.ok,
      code: list.body?.code,
      msg: redact(list.body?.msg || ""),
      count: list.body?.data?.list?.length ?? list.body?.data?.page_list?.length ?? null,
    };
  } catch (error) {
    result.profileList = { httpOk: false, error: normalizeError(error) };
  }
  if (dryRun && adsPowerKey && result.profileList?.code === 0) {
    result.dryRun = await runAdsPowerDryRun();
  }
  return result;
}

async function runAdsPowerDryRun() {
  const startedAt = Date.now();
  const profileName = `benchmark-dryrun-adspower-udeal-la-${stamp}`;
  const payload = {
    name: profileName,
    group_id: "0",
    remark: "three-browser-benchmark dry-run udeal-la",
    platform: "browserleaks.com",
    tabs: ["https://browserleaks.com/ip"],
    user_proxy_config: {
      proxy_soft: "other",
      proxy_type: "http",
      proxy_host: "127.0.0.1",
      proxy_port: "18082",
    },
    fingerprint_config: {
      automatic_timezone: "1",
      language_switch: "0",
      language: ["en-US", "en"],
      page_language_switch: "0",
      page_language: "en-US",
      webrtc: "disabled",
      flash: "block",
      canvas: "1",
      webgl_image: "1",
      webgl: "3",
      audio: "1",
      hardware_concurrency: "8",
      device_memory: "8",
      screen_resolution: "1280_720",
      browser_kernel_config: {
        type: "chrome",
        version: "ua_auto",
      },
    },
  };
  const out = { profileName, proxyHash: hashId("http://127.0.0.1:18082") };
  try {
    const created = await httpJSON(`${endpoints.adsPower}/api/v2/browser-profile/create`, {
      method: "POST",
      headers: adsPowerHeaders(),
      body: JSON.stringify(payload),
      timeoutMs: 30000,
    });
    out.createCode = created.body?.code ?? null;
    out.createStatus = created.status;
    out.createMsg = redact(created.body?.msg || "");
    out.profileId = created.body?.data?.profile_id || created.body?.data?.user_id || null;
    out.profileNo = created.body?.data?.profile_no || null;
    if (out.profileId) {
      const opened = await httpJSON(`${endpoints.adsPower}/api/v2/browser-profile/start`, {
        method: "POST",
        headers: adsPowerHeaders(),
        body: JSON.stringify({
          profile_id: out.profileId,
          last_opened_tabs: "0",
          proxy_detection: "0",
          cdp_mask: "1",
          launch_args: ["--window-size=1280,720"],
        }),
        timeoutMs: 60000,
      });
      out.openCode = opened.body?.code ?? null;
      out.openStatus = opened.status;
      out.openMsg = redact(opened.body?.msg || "");
      out.debugPort = opened.body?.data?.debug_port || null;
      out.seleniumDebug = opened.body?.data?.ws?.selenium || null;
      out.puppeteerWsPresent = Boolean(opened.body?.data?.ws?.puppeteer);
      out.webdriverPresent = Boolean(opened.body?.data?.webdriver);
      const cdpBase = out.debugPort
        ? `http://127.0.0.1:${out.debugPort}`
        : out.seleniumDebug
          ? `http://${out.seleniumDebug}`
          : "";
      if (cdpBase) {
        await captureCdpSnapshot(cdpBase, out);
      }
      try {
        const stopped = await httpJSON(`${endpoints.adsPower}/api/v2/browser-profile/stop`, {
          method: "POST",
          headers: adsPowerHeaders(),
          body: JSON.stringify({ profile_id: out.profileId }),
          timeoutMs: 30000,
        });
        out.closeCode = stopped.body?.code ?? null;
        out.closeStatus = stopped.status;
        out.closeMsg = redact(stopped.body?.msg || "");
      } catch (error) {
        out.closeError = normalizeError(error);
      }
    }
  } catch (error) {
    out.error = normalizeError(error);
  }
  out.durationMs = Date.now() - startedAt;
  return out;
}

async function main() {
  await mkdir(outputDir, { recursive: true });
  const report = {
    schema: "three_browser_benchmark_readiness_v1",
    generatedAt: new Date().toISOString(),
    dryRun,
    defaultProducts,
    selectedProducts,
    skippedProducts: allProducts.filter((product) => !productSelected(product)),
    proxyPreflight: await latestProxyPreflightSummary(),
    ports: {
      ...(productSelected("personalPilot") ? { personalPilot: await tcpCheck("127.0.0.1", 19876) } : {}),
      ...(productSelected("bitBrowser") ? { bitBrowser: await tcpCheck("127.0.0.1", 54345) } : {}),
      ...(productSelected("adsPower") ? { adsPower: await tcpCheck("127.0.0.1", 50325) } : {}),
      clashMixed: await tcpCheck("127.0.0.1", 7897),
      udealBridgeHttp: await tcpCheck("127.0.0.1", 18082),
      udealBridgeSocks: await tcpCheck("127.0.0.1", 18090),
    },
    products: {
      ...(productSelected("personalPilot") ? { personalPilot: await checkPersonalPilot() } : {}),
      ...(productSelected("bitBrowser") ? { bitBrowser: await checkBitBrowser() } : {}),
      ...(productSelected("adsPower") ? { adsPower: await checkAdsPower() } : {}),
    },
    blockers: [],
    nextActions: [],
  };

  if (productSelected("adsPower") && report.products.adsPower?.profileList?.code === -1) {
    report.blockers.push("AdsPower Local API requires api-key; current Free plan may not expose API & MCP without paid/trial access.");
  }
  if (productSelected("adsPower") && dryRun && adsPowerKey && !report.products.adsPower?.dryRun?.cdpVersionOk) {
    report.blockers.push("AdsPower dry-run did not reach CDP version endpoint.");
  }
  if (productSelected("bitBrowser") && (report.products.bitBrowser?.profileCount ?? 0) === 0 && !report.products.bitBrowser?.dryRun?.profileId) {
    report.blockers.push("BitBrowser has no benchmark profile yet.");
  }
  if (productSelected("personalPilot") && !report.products.personalPilot?.dryRun?.cdpVersionOk && dryRun) {
    report.blockers.push("PersonalPilot dry-run did not reach CDP version endpoint.");
  }
  if (productSelected("adsPower") && report.products.adsPower?.profileList?.code === -1) {
    report.nextActions.push("Use paid/trial AdsPower API access and set ADSPOWER_API_KEY, or keep AdsPower as API-paywalled/manual-only in this benchmark.");
  }
  report.nextActions.push("After all selected products have profile IDs and debug endpoints, run detector matrix launch loops.");

  const jsonPath = path.join(outputDir, `readiness-${stamp}.json`);
  await writeFile(jsonPath, JSON.stringify(report, null, 2), "utf8");
  console.log(JSON.stringify({
    ok: report.blockers.length === 0,
    reportPath: jsonPath,
    selectedProducts: report.selectedProducts,
    skippedProducts: report.skippedProducts,
    blockers: report.blockers,
    personalPilotDryRun: report.products.personalPilot?.dryRun || null,
    bitBrowserDryRun: report.products.bitBrowser?.dryRun || null,
    adsPowerDryRun: report.products.adsPower?.dryRun || null,
    adsPower: report.products.adsPower?.profileList || null,
  }, null, 2));
}

main().catch((error) => {
  console.error(normalizeError(error));
  process.exit(1);
});
