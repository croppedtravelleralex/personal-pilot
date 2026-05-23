import type { DesktopValidationBrowserSignal } from "../../types/desktop";

type ProbeStatus = DesktopValidationBrowserSignal["status"];

function elapsed(started: number) {
  return Math.max(0, Math.round(performance.now() - started));
}

function signal(
  id: string,
  category: string,
  status: ProbeStatus,
  label: string,
  summary: string,
  detail: string | null,
  durationMs: number,
): DesktopValidationBrowserSignal {
  return {
    id,
    category,
    layer: "observed",
    status,
    label,
    summary,
    detail,
    collectorScope: "desktop-webview",
    runtimeAdapter: "desktop_webview",
    targetProfileBrowser: false,
    failureReason: status === "failed" ? summary : null,
    durationMs,
  };
}

async function collectWebRtcSignal(): Promise<DesktopValidationBrowserSignal> {
  const started = performance.now();
  const peerConnection = window.RTCPeerConnection;
  if (!peerConnection) {
    return signal(
      "webrtc-desktop-webview-api",
      "webrtc",
      "warning",
      "WebRTC desktop WebView API probe",
      "Desktop WebView does not expose RTCPeerConnection in this runtime.",
      "scope=desktop-webview; target-profile-browser=false",
      elapsed(started),
    );
  }

  const connection = new peerConnection({ iceServers: [] });
  const candidates = new Set<string>();

  try {
    connection.createDataChannel("validation-probe");
    connection.onicecandidate = (event) => {
      if (event.candidate?.candidate) {
        candidates.add(event.candidate.candidate);
      }
    };
    const offer = await connection.createOffer();
    await connection.setLocalDescription(offer);
    await new Promise<void>((resolve) => window.setTimeout(resolve, 1200));
  } catch (error) {
    return signal(
      "webrtc-desktop-webview-api",
      "webrtc",
      "failed",
      "WebRTC desktop WebView API probe",
      `WebRTC probe failed: ${error instanceof Error ? error.message : String(error)}`,
      "scope=desktop-webview; target-profile-browser=false",
      elapsed(started),
    );
  } finally {
    connection.close();
  }

  return signal(
    "webrtc-desktop-webview-api",
    "webrtc",
    candidates.size > 0 ? "succeeded" : "warning",
    "WebRTC desktop WebView API probe",
    candidates.size > 0
      ? `Desktop WebView gathered ${candidates.size} ICE candidate(s).`
      : "Desktop WebView WebRTC API is present, but no ICE candidates were gathered.",
    `scope=desktop-webview; target-profile-browser=false; candidates=${Array.from(candidates).join(" | ")}`,
    elapsed(started),
  );
}

function collectCanvasSignal(): DesktopValidationBrowserSignal {
  const started = performance.now();
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 240;
    canvas.height = 80;
    const context = canvas.getContext("2d");
    if (!context) {
      return signal(
        "canvas-desktop-webview-render",
        "canvas",
        "failed",
        "Canvas desktop WebView render probe",
        "Desktop WebView could not create a 2D canvas context.",
        "scope=desktop-webview; target-profile-browser=false",
        elapsed(started),
      );
    }

    context.fillStyle = "#153a5b";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.fillStyle = "#f4c542";
    context.font = "18px Arial";
    context.fillText("PersonaPilot validation", 12, 36);
    context.strokeStyle = "#ffffff";
    context.strokeRect(8, 8, 224, 64);
    const dataUrl = canvas.toDataURL("image/png");

    return signal(
      "canvas-desktop-webview-render",
      "canvas",
      dataUrl.length > 100 ? "succeeded" : "warning",
      "Canvas desktop WebView render probe",
      `Desktop WebView canvas rendered a ${dataUrl.length} byte data URL sample.`,
      `scope=desktop-webview; target-profile-browser=false; size=${canvas.width}x${canvas.height}`,
      elapsed(started),
    );
  } catch (error) {
    return signal(
      "canvas-desktop-webview-render",
      "canvas",
      "failed",
      "Canvas desktop WebView render probe",
      `Canvas probe failed: ${error instanceof Error ? error.message : String(error)}`,
      "scope=desktop-webview; target-profile-browser=false",
      elapsed(started),
    );
  }
}

async function collectAudioSignal(): Promise<DesktopValidationBrowserSignal> {
  const started = performance.now();
  const AudioContextCtor = window.AudioContext ?? window.webkitAudioContext;
  if (!AudioContextCtor) {
    return signal(
      "audio-desktop-webview-context",
      "audio",
      "warning",
      "AudioContext desktop WebView probe",
      "Desktop WebView does not expose AudioContext in this runtime.",
      "scope=desktop-webview; target-profile-browser=false",
      elapsed(started),
    );
  }

  let context: AudioContext | null = null;
  try {
    context = new AudioContextCtor();
    const detail = [
      `scope=desktop-webview`,
      `target-profile-browser=false`,
      `sampleRate=${context.sampleRate}`,
      `state=${context.state}`,
    ].join("; ");
    return signal(
      "audio-desktop-webview-context",
      "audio",
      context.sampleRate > 0 ? "succeeded" : "warning",
      "AudioContext desktop WebView probe",
      `Desktop WebView AudioContext opened with sampleRate=${context.sampleRate}.`,
      detail,
      elapsed(started),
    );
  } catch (error) {
    return signal(
      "audio-desktop-webview-context",
      "audio",
      "failed",
      "AudioContext desktop WebView probe",
      `AudioContext probe failed: ${error instanceof Error ? error.message : String(error)}`,
      "scope=desktop-webview; target-profile-browser=false",
      elapsed(started),
    );
  } finally {
    await context?.close().catch(() => undefined);
  }
}

function collectLeakSignal(): DesktopValidationBrowserSignal {
  const started = performance.now();
  const storage = {
    cookieEnabled: navigator.cookieEnabled,
    localStorageAvailable: false,
    sessionStorageAvailable: false,
  };

  try {
    const key = "persona-pilot-validation-local";
    window.localStorage.setItem(key, "1");
    storage.localStorageAvailable = window.localStorage.getItem(key) === "1";
    window.localStorage.removeItem(key);
  } catch {
    storage.localStorageAvailable = false;
  }

  try {
    const key = "persona-pilot-validation-session";
    window.sessionStorage.setItem(key, "1");
    storage.sessionStorageAvailable = window.sessionStorage.getItem(key) === "1";
    window.sessionStorage.removeItem(key);
  } catch {
    storage.sessionStorageAvailable = false;
  }

  return signal(
    "leak-desktop-webview-storage-scope",
    "leak",
    storage.localStorageAvailable || storage.sessionStorageAvailable ? "succeeded" : "warning",
    "Desktop WebView storage scope probe",
    "Desktop WebView storage APIs were sampled for leak-check capability evidence.",
    `scope=desktop-webview; target-profile-browser=false; cookieEnabled=${storage.cookieEnabled}; localStorage=${storage.localStorageAvailable}; sessionStorage=${storage.sessionStorageAvailable}`,
    elapsed(started),
  );
}

export async function collectBrowserValidationSignals(): Promise<DesktopValidationBrowserSignal[]> {
  const [webrtc, audio] = await Promise.all([collectWebRtcSignal(), collectAudioSignal()]);
  return [webrtc, collectCanvasSignal(), audio, collectLeakSignal()];
}

declare global {
  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}
