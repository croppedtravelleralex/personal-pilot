use std::{collections::VecDeque, fs, io, net::TcpListener, path::Path, process::Stdio};

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    task::JoinHandle,
    time::{sleep, timeout, Duration, Instant},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    domain::run::{RUN_STATUS_FAILED, RUN_STATUS_SUCCEEDED, RUN_STATUS_TIMED_OUT},
    runner::{
        RunnerCancelResult, RunnerCapabilities, RunnerExecutionResult, RunnerOutcomeStatus,
        RunnerTask, TaskRunner,
    },
};

const CAMOUFOX_RUNNER_MODE: &str = "camoufox_minimal_cdp_v1";
const ENABLED_ENV: &str = "PERSONA_PILOT_CAMOUFOX_ENABLED";
const CONFIG_ENV: &str = "PERSONA_PILOT_CAMOUFOX_CONFIG";
const TEST_WS_ENV: &str = "PERSONA_PILOT_CAMOUFOX_TEST_WS";
const HTML_PREVIEW_LIMIT: usize = 4000;
const TEXT_PREVIEW_LIMIT: usize = 4000;
const STDOUT_PREVIEW_LIMIT: usize = 4000;
const STDERR_PREVIEW_LIMIT: usize = 2000;

type CdpSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Default)]
pub struct CamoufoxRunner;

#[derive(Debug, Clone, Deserialize)]
struct CamoufoxConfig {
    #[serde(default, alias = "binary", alias = "executable")]
    binary_path: String,
    #[serde(default, alias = "profile", alias = "profile_path")]
    profile_dir: Option<String>,
    #[serde(default)]
    extra_args: Vec<String>,
    #[serde(default)]
    proxy_server: Option<String>,
}

#[derive(Debug)]
struct RunnerFailure {
    error_kind: &'static str,
    message: String,
    stage_hint: Option<&'static str>,
    stderr_hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct BrowserActionResult {
    title: Option<String>,
    final_url: Option<String>,
    html: Option<String>,
    text: Option<String>,
    validation_signals: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct BrowserReadinessSnapshot {
    #[serde(default, rename = "readyState")]
    ready_state: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "href")]
    final_url: String,
    #[serde(default, rename = "hasHtml")]
    has_html: bool,
    #[serde(default, rename = "hasBody")]
    has_body: bool,
}

#[derive(Debug, Deserialize)]
struct BrowserHtmlSnapshot {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "href")]
    final_url: String,
    #[serde(default)]
    html: String,
}

#[derive(Debug, Deserialize)]
struct BrowserTextSnapshot {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "href")]
    final_url: String,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct BrowserVersionResponse {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

struct SpawnedCamoufox {
    child: Child,
    pid: u32,
    stdout_handle: JoinHandle<String>,
    stderr_handle: JoinHandle<String>,
}

struct CdpClient {
    socket: CdpSocket,
    next_id: u64,
    backlog: VecDeque<Value>,
}

impl RunnerFailure {
    fn new(
        error_kind: &'static str,
        message: impl Into<String>,
        stage_hint: Option<&'static str>,
        stderr_hint: Option<String>,
    ) -> Self {
        Self {
            error_kind,
            message: message.into(),
            stage_hint,
            stderr_hint,
        }
    }
}

impl BrowserActionResult {
    fn from_readiness(snapshot: BrowserReadinessSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: None,
            text: None,
            validation_signals: Vec::new(),
        }
    }

    fn from_html(snapshot: BrowserHtmlSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: Some(snapshot.html),
            text: None,
            validation_signals: Vec::new(),
        }
    }

    fn from_text(snapshot: BrowserTextSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: None,
            text: Some(snapshot.text),
            validation_signals: Vec::new(),
        }
    }
}

impl CdpClient {
    async fn connect(ws_endpoint: &str) -> Result<Self, RunnerFailure> {
        let (socket, _) = connect_async(ws_endpoint).await.map_err(|err| {
            RunnerFailure::new(
                "cdp_connect_failed",
                format!("camoufox CDP websocket connect failed: {err}"),
                Some("launch"),
                Some(err.to_string()),
            )
        })?;
        Ok(Self {
            socket,
            next_id: 0,
            backlog: VecDeque::new(),
        })
    }

    async fn create_target(&mut self) -> Result<String, RunnerFailure> {
        let response = self
            .send_command("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await?;
        response
            .pointer("/result/targetId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    "camoufox did not return targetId for Target.createTarget",
                    Some("launch"),
                    None,
                )
            })
    }

    async fn attach_to_target(&mut self, target_id: &str) -> Result<String, RunnerFailure> {
        let response = self
            .send_command(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        response
            .pointer("/result/sessionId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    "camoufox did not return sessionId for Target.attachToTarget",
                    Some("launch"),
                    None,
                )
            })
    }

    async fn enable_page_runtime(&mut self, session_id: &str) -> Result<(), RunnerFailure> {
        self.send_command("Page.enable", json!({}), Some(session_id))
            .await?;
        self.send_command("Runtime.enable", json!({}), Some(session_id))
            .await?;
        Ok(())
    }

    async fn navigate(&mut self, session_id: &str, url: &str) -> Result<(), RunnerFailure> {
        let response = self
            .send_command("Page.navigate", json!({ "url": url }), Some(session_id))
            .await?;
        if let Some(error_text) = response
            .pointer("/result/errorText")
            .and_then(Value::as_str)
        {
            return Err(RunnerFailure::new(
                "browser_navigation_failed",
                format!("camoufox navigation failed: {error_text}"),
                Some("navigate"),
                Some(error_text.to_string()),
            ));
        }
        Ok(())
    }

    async fn evaluate_json(
        &mut self,
        session_id: &str,
        expression: &str,
    ) -> Result<Value, RunnerFailure> {
        let response = self
            .send_command(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
                Some(session_id),
            )
            .await?;
        if let Some(description) = response
            .pointer("/result/exceptionDetails/text")
            .and_then(Value::as_str)
        {
            return Err(RunnerFailure::new(
                "cdp_evaluate_failed",
                format!("camoufox evaluation failed: {description}"),
                Some("action"),
                Some(description.to_string()),
            ));
        }
        response
            .pointer("/result/result/value")
            .cloned()
            .ok_or_else(|| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    "camoufox evaluation did not return JSON value",
                    Some("action"),
                    None,
                )
            })
    }

    async fn read_readiness(
        &mut self,
        session_id: &str,
    ) -> Result<BrowserReadinessSnapshot, RunnerFailure> {
        serde_json::from_value(
            self.evaluate_json(session_id, readiness_expression())
                .await?,
        )
        .map_err(|err| {
            RunnerFailure::new(
                "cdp_protocol_error",
                format!("failed to decode camoufox readiness snapshot: {err}"),
                Some("action"),
                Some(err.to_string()),
            )
        })
    }

    async fn read_html(&mut self, session_id: &str) -> Result<BrowserHtmlSnapshot, RunnerFailure> {
        serde_json::from_value(self.evaluate_json(session_id, html_expression()).await?).map_err(
            |err| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    format!("failed to decode camoufox html snapshot: {err}"),
                    Some("output_wait"),
                    Some(err.to_string()),
                )
            },
        )
    }

    async fn read_text(&mut self, session_id: &str) -> Result<BrowserTextSnapshot, RunnerFailure> {
        serde_json::from_value(self.evaluate_json(session_id, text_expression()).await?).map_err(
            |err| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    format!("failed to decode camoufox text snapshot: {err}"),
                    Some("output_wait"),
                    Some(err.to_string()),
                )
            },
        )
    }

    async fn read_validation_signals(
        &mut self,
        session_id: &str,
    ) -> Result<Vec<Value>, RunnerFailure> {
        self.evaluate_json(session_id, validation_probe_expression())
            .await?
            .as_array()
            .cloned()
            .ok_or_else(|| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    "failed to decode camoufox validation probe signals",
                    Some("action"),
                    None,
                )
            })
    }

    async fn send_command(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, RunnerFailure> {
        self.next_id += 1;
        let command_id = self.next_id;
        let mut payload = json!({ "id": command_id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            payload["sessionId"] = Value::String(session_id.to_string());
        }
        self.socket
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|err| {
                RunnerFailure::new(
                    "cdp_send_failed",
                    format!("failed to send camoufox CDP command {method}: {err}"),
                    Some("action"),
                    Some(err.to_string()),
                )
            })?;
        loop {
            if let Some(index) = self
                .backlog
                .iter()
                .position(|message| message.get("id").and_then(Value::as_u64) == Some(command_id))
            {
                let response = self.backlog.remove(index).expect("backlog response");
                return Self::decode_response(method, response);
            }
            let message = self.read_next_json_message().await?;
            if message.get("id").and_then(Value::as_u64) == Some(command_id) {
                return Self::decode_response(method, message);
            }
            self.backlog.push_back(message);
        }
    }

    fn decode_response(method: &str, response: Value) -> Result<Value, RunnerFailure> {
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown CDP error");
            return Err(RunnerFailure::new(
                "cdp_protocol_error",
                format!("CDP command {method} failed: {message}"),
                Some("action"),
                Some(message.to_string()),
            ));
        }
        Ok(response)
    }

    async fn read_next_json_message(&mut self) -> Result<Value, RunnerFailure> {
        loop {
            let next = self.socket.next().await.ok_or_else(|| {
                RunnerFailure::new(
                    "runner_connection_closed",
                    "camoufox websocket stream ended unexpectedly",
                    Some("action"),
                    Some("websocket stream ended".to_string()),
                )
            })?;
            let message = next.map_err(|err| {
                RunnerFailure::new(
                    "runner_connection_closed",
                    format!("camoufox websocket read failed: {err}"),
                    Some("action"),
                    Some(err.to_string()),
                )
            })?;
            match message {
                Message::Text(text) => {
                    return serde_json::from_str::<Value>(&text).map_err(|err| {
                        RunnerFailure::new(
                            "cdp_protocol_error",
                            format!("failed to decode camoufox CDP text message: {err}"),
                            Some("action"),
                            Some(err.to_string()),
                        )
                    });
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice::<Value>(&bytes).map_err(|err| {
                        RunnerFailure::new(
                            "cdp_protocol_error",
                            format!("failed to decode camoufox CDP binary message: {err}"),
                            Some("action"),
                            Some(err.to_string()),
                        )
                    });
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
                Message::Close(frame) => {
                    return Err(RunnerFailure::new(
                        "runner_connection_closed",
                        format!("camoufox websocket closed: {frame:?}"),
                        Some("action"),
                        Some("websocket closed".to_string()),
                    ));
                }
            }
        }
    }
}

fn env_enabled() -> bool {
    std::env::var(ENABLED_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn config_path() -> Option<String> {
    std::env::var(CONFIG_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn load_config(path: &str) -> Result<CamoufoxConfig, RunnerFailure> {
    let raw = fs::read_to_string(path).map_err(|err| {
        RunnerFailure::new(
            "runner_config_read_failed",
            format!("failed to read camoufox config {path}: {err}"),
            Some("configuration"),
            Some(err.to_string()),
        )
    })?;
    let mut config: CamoufoxConfig = serde_json::from_str(&raw).map_err(|err| {
        RunnerFailure::new(
            "runner_config_invalid",
            format!("failed to parse camoufox config {path}: {err}"),
            Some("configuration"),
            Some(err.to_string()),
        )
    })?;
    config.binary_path = config.binary_path.trim().to_string();
    Ok(config)
}

fn requested_action(task: &RunnerTask) -> String {
    task.payload
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or(task.kind.as_str())
        .to_string()
}

fn normalize_action(action: &str) -> Option<&'static str> {
    match action.trim() {
        "fetch" | "open_page" => Some("open_page"),
        "get_html" => Some("get_html"),
        "get_title" => Some("get_title"),
        "get_final_url" => Some("get_final_url"),
        "extract_text" => Some("extract_text"),
        "validation_probe" => Some("validation_probe"),
        _ => None,
    }
}

fn supported_actions() -> Vec<&'static str> {
    vec![
        "open_page",
        "fetch",
        "get_html",
        "get_title",
        "get_final_url",
        "extract_text",
        "validation_probe",
    ]
}

fn extract_url(payload: &Value) -> Option<String> {
    payload
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn looks_like_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn allocate_loopback_port() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn readiness_expression() -> &'static str {
    r#"(() => {
  const doc = document;
  return { readyState: doc ? (doc.readyState || '') : '', title: doc ? (doc.title || '') : '', href: window.location ? (window.location.href || '') : '', hasHtml: !!(doc && doc.documentElement), hasBody: !!(doc && doc.body) };
})()"#
}

fn html_expression() -> &'static str {
    r#"(() => ({ title: document.title || '', href: window.location.href || '', html: document.documentElement ? document.documentElement.outerHTML : '' }))()"#
}

fn text_expression() -> &'static str {
    r#"(() => ({ title: document.title || '', href: window.location.href || '', text: document.body ? document.body.innerText : '' }))()"#
}

fn validation_probe_expression() -> &'static str {
    r#"(async function(){
  function elapsed(start){ return Math.max(0, Math.round(performance.now() - start)); }
  function signal(id, category, status, label, summary, detail, durationMs){ return { id, category, layer: 'observed', status, label, summary, detail, durationMs }; }
  const signals = [];
  const runtimeDetail = 'scope=camoufox-profile-browser; collector=cdp-runtime-evaluate';
  const navStarted = performance.now();
  signals.push(signal('camoufox-profile-browser-navigator', 'detector', navigator.userAgent ? 'succeeded' : 'warning', 'Camoufox navigator runtime probe', 'Camoufox navigator sampled.', `${runtimeDetail}; userAgent=${navigator.userAgent}; platform=${navigator.platform}; webdriver=${navigator.webdriver}; languages=${Array.from(navigator.languages || []).join(',')}`, elapsed(navStarted)));
  try {
    const started = performance.now();
    const canvas = document.createElement('canvas'); canvas.width = 120; canvas.height = 40; const ctx = canvas.getContext('2d'); ctx.fillText('PersonaPilot', 4, 20); const data = canvas.toDataURL('image/png');
    signals.push(signal('camoufox-profile-browser-canvas', 'canvas', data.length > 100 ? 'succeeded' : 'warning', 'Camoufox canvas runtime probe', 'Camoufox canvas sampled.', `${runtimeDetail}; dataUrlLength=${data.length}; size=${canvas.width}x${canvas.height}`, elapsed(started)));
  } catch (error) { signals.push(signal('camoufox-profile-browser-canvas', 'canvas', 'failed', 'Camoufox canvas runtime probe', 'Camoufox canvas probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    const dtf = Intl.DateTimeFormat().resolvedOptions();
    signals.push(signal('camoufox-profile-browser-timezone', 'timezone', dtf.timeZone ? 'succeeded' : 'warning', 'Camoufox timezone runtime probe', 'Camoufox timezone sampled.', `${runtimeDetail}; timezone=${dtf.timeZone}; locale=${dtf.locale}`, elapsed(started)));
  } catch (error) { signals.push(signal('camoufox-profile-browser-timezone', 'timezone', 'failed', 'Camoufox timezone runtime probe', 'Camoufox timezone probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    if (!gl) {
      signals.push(signal('camoufox-profile-browser-webgl', 'webgl', 'warning', 'Camoufox WebGL runtime probe', 'Camoufox runtime does not expose WebGL.', runtimeDetail, elapsed(started)));
    } else {
      const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
      const vendor = debugInfo ? gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR);
      const renderer = debugInfo ? gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
      signals.push(signal('camoufox-profile-browser-webgl', 'webgl', vendor || renderer ? 'succeeded' : 'warning', 'Camoufox WebGL runtime probe', 'Camoufox WebGL surface sampled.', `${runtimeDetail}; vendor=${vendor}; renderer=${renderer}; extensions=${(gl.getSupportedExtensions() || []).length}`, elapsed(started)));
    }
  } catch (error) { signals.push(signal('camoufox-profile-browser-webgl', 'webgl', 'failed', 'Camoufox WebGL runtime probe', 'Camoufox WebGL probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    const AudioContextCtor = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextCtor) {
      signals.push(signal('camoufox-profile-browser-audio', 'audio', 'warning', 'Camoufox AudioContext runtime probe', 'Camoufox runtime does not expose AudioContext.', runtimeDetail, elapsed(started)));
    } else {
      const context = new AudioContextCtor();
      signals.push(signal('camoufox-profile-browser-audio', 'audio', context.sampleRate > 0 ? 'succeeded' : 'warning', 'Camoufox AudioContext runtime probe', 'Camoufox AudioContext sampled.', `${runtimeDetail}; sampleRate=${context.sampleRate}; state=${context.state}`, elapsed(started)));
      if (context.close) await context.close().catch(() => undefined);
    }
  } catch (error) { signals.push(signal('camoufox-profile-browser-audio', 'audio', 'failed', 'Camoufox AudioContext runtime probe', 'Camoufox audio probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    const fonts = document.fonts && document.fonts.check ? ['Arial', 'Segoe UI', 'Times New Roman'].filter((font) => document.fonts.check(`12px "${font}"`)) : [];
    signals.push(signal('camoufox-profile-browser-fonts', 'fingerprint', document.fonts ? 'succeeded' : 'warning', 'Camoufox font runtime probe', 'Camoufox font surface sampled.', `${runtimeDetail}; documentFonts=${!!document.fonts}; matched=${fonts.join(',')}`, elapsed(started)));
  } catch (error) { signals.push(signal('camoufox-profile-browser-fonts', 'fingerprint', 'failed', 'Camoufox font runtime probe', 'Camoufox font probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    const devices = navigator.mediaDevices && navigator.mediaDevices.enumerateDevices ? await navigator.mediaDevices.enumerateDevices().catch(() => []) : [];
    signals.push(signal('camoufox-profile-browser-media-devices', 'fingerprint', navigator.mediaDevices ? 'succeeded' : 'warning', 'Camoufox media devices runtime probe', 'Camoufox media devices surface sampled.', `${runtimeDetail}; mediaDevices=${!!navigator.mediaDevices}; count=${devices.length}; kinds=${devices.map((d) => d.kind).join(',')}`, elapsed(started)));
  } catch (error) { signals.push(signal('camoufox-profile-browser-media-devices', 'fingerprint', 'failed', 'Camoufox media devices runtime probe', 'Camoufox media devices probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  try {
    const started = performance.now();
    if (!window.RTCPeerConnection) {
      signals.push(signal('camoufox-profile-browser-webrtc', 'webrtc', 'warning', 'Camoufox WebRTC runtime probe', 'Camoufox runtime does not expose RTCPeerConnection.', runtimeDetail, elapsed(started)));
    } else {
      const pc = new RTCPeerConnection({ iceServers: [] });
      const candidates = [];
      pc.createDataChannel('validation-probe');
      pc.onicecandidate = (event) => { if (event.candidate && event.candidate.candidate) candidates.push(event.candidate.candidate); };
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      await new Promise((resolve) => setTimeout(resolve, 800));
      pc.close();
      signals.push(signal('camoufox-profile-browser-webrtc', 'webrtc', candidates.length > 0 ? 'succeeded' : 'warning', 'Camoufox WebRTC runtime probe', candidates.length > 0 ? `Camoufox gathered ${candidates.length} ICE candidate(s).` : 'Camoufox WebRTC API is present, but no ICE candidates were gathered.', `${runtimeDetail}; candidates=${candidates.join(' | ')}`, elapsed(started)));
    }
  } catch (error) { signals.push(signal('camoufox-profile-browser-webrtc', 'webrtc', 'failed', 'Camoufox WebRTC runtime probe', 'Camoufox WebRTC probe failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0)); }
  const storageStarted = performance.now();
  try {
    if (!window.__personalPilotPointerOverlay) {
      const dot = document.createElement('div');
      dot.style.cssText = 'position:fixed;width:16px;height:16px;border-radius:999px;border:2px solid #fff;background:rgba(20,120,255,.85);z-index:2147483647;pointer-events:none;transform:translate(-9999px,-9999px)';
      document.documentElement.appendChild(dot);
      const move = (e) => { dot.style.transform = 'translate(' + (e.clientX - 8) + 'px,' + (e.clientY - 8) + 'px)'; };
      window.addEventListener('mousemove', move, true);
      window.__personalPilotPointerOverlay = { destroy: () => { window.removeEventListener('mousemove', move, true); dot.remove(); delete window.__personalPilotPointerOverlay; } };
    }
    signals.push(signal('camoufox-profile-browser-pointer', 'fingerprint', 'succeeded', 'Camoufox pointer overlay probe', 'Pointer overlay installed for automation visibility.', runtimeDetail, 0));
  } catch (error) {
    signals.push(signal('camoufox-profile-browser-pointer', 'fingerprint', 'warning', 'Camoufox pointer overlay probe', 'Pointer overlay install failed.', `${runtimeDetail}; error=${error && error.message ? error.message : String(error)}`, 0));
  }
  let localStorageAvailable = false;
  let sessionStorageAvailable = false;
  try { localStorage.setItem('persona-pilot-camoufox-local', '1'); localStorageAvailable = localStorage.getItem('persona-pilot-camoufox-local') === '1'; localStorage.removeItem('persona-pilot-camoufox-local'); } catch (_) {}
  try { sessionStorage.setItem('persona-pilot-camoufox-session', '1'); sessionStorageAvailable = sessionStorage.getItem('persona-pilot-camoufox-session') === '1'; sessionStorage.removeItem('persona-pilot-camoufox-session'); } catch (_) {}
  signals.push(signal('camoufox-profile-browser-storage', 'leak', navigator.cookieEnabled || localStorageAvailable || sessionStorageAvailable ? 'succeeded' : 'warning', 'Camoufox storage scope runtime probe', 'Camoufox storage surface sampled.', `${runtimeDetail}; cookieEnabled=${navigator.cookieEnabled}; localStorage=${localStorageAvailable}; sessionStorage=${sessionStorageAvailable}`, elapsed(storageStarted)));
  return signals;
})()"#
}

fn snapshot_is_readable(action: &str, snapshot: &BrowserReadinessSnapshot) -> bool {
    let ready = matches!(snapshot.ready_state.as_str(), "interactive" | "complete");
    match action {
        "get_html" => ready && snapshot.has_html,
        "extract_text" => ready && snapshot.has_body,
        "open_page" | "get_title" | "get_final_url" => {
            ready && !snapshot.final_url.trim().is_empty()
        }
        _ => ready,
    }
}

async fn perform_browser_action(
    ws_endpoint: &str,
    task: &RunnerTask,
    action: &str,
    url: &str,
) -> Result<BrowserActionResult, RunnerFailure> {
    let mut client = CdpClient::connect(ws_endpoint).await?;
    let target_id = client.create_target().await?;
    let session_id = client.attach_to_target(&target_id).await?;
    client.enable_page_runtime(&session_id).await?;
    client.navigate(&session_id, url).await?;

    let deadline = Instant::now()
        + Duration::from_secs(task.timeout_seconds.unwrap_or(10).clamp(1, 120) as u64);
    loop {
        match client.read_readiness(&session_id).await {
            Ok(snapshot) if snapshot_is_readable(action, &snapshot) => {
                let mut result = match action {
                    "get_html" => client
                        .read_html(&session_id)
                        .await
                        .map(BrowserActionResult::from_html),
                    "extract_text" => client
                        .read_text(&session_id)
                        .await
                        .map(BrowserActionResult::from_text),
                    _ => Ok(BrowserActionResult::from_readiness(snapshot)),
                }?;
                if action == "validation_probe" {
                    result.validation_signals = client
                        .read_validation_signals(&session_id)
                        .await
                        .unwrap_or_default();
                }
                return Ok(result);
            }
            Ok(_) => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(RunnerFailure::new(
                "timeout",
                "camoufox action timed out while waiting for readable page",
                Some("output_wait"),
                None,
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn spawn_camoufox(
    config: &CamoufoxConfig,
    port: u16,
    timeout_seconds: u64,
) -> Result<SpawnedCamoufox, io::Error> {
    let mut cmd = Command::new(&config.binary_path);
    cmd.arg(format!("--remote-debugging-port={port}"));
    if let Some(profile_dir) = config
        .profile_dir
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        fs::create_dir_all(profile_dir)?;
        cmd.arg("--profile").arg(profile_dir);
    }
    if let Some(proxy_server) = config
        .proxy_server
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.arg(format!("--proxy-server={proxy_server}"));
    }
    for arg in &config.extra_args {
        let trimmed = arg.trim();
        if !trimmed.is_empty() {
            cmd.arg(trimmed);
        }
    }
    cmd.arg("about:blank");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        let _ = timeout_seconds;
    }
    #[cfg(unix)]
    {
        let _ = timeout_seconds;
    }
    let mut child = cmd.spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "camoufox child did not expose pid"))?;
    let stdout_handle = tokio::spawn(read_stream_to_string(child.stdout.take()));
    let stderr_handle = tokio::spawn(read_stream_to_string(child.stderr.take()));
    Ok(SpawnedCamoufox {
        child,
        pid,
        stdout_handle,
        stderr_handle,
    })
}

async fn wait_for_ws_endpoint(child: &mut Child, port: u16) -> Result<String, RunnerFailure> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .map_err(|err| {
            RunnerFailure::new(
                "spawn_failed",
                format!("failed to build camoufox readiness client: {err}"),
                Some("launch"),
                Some(err.to_string()),
            )
        })?;
    let version_url = format!("http://127.0.0.1:{port}/json/version");
    loop {
        if let Some(status) = child.try_wait().map_err(|err| {
            RunnerFailure::new(
                "process_wait_failed",
                format!("failed to inspect camoufox process: {err}"),
                Some("launch"),
                Some(err.to_string()),
            )
        })? {
            return Err(RunnerFailure::new(
                "runner_process_exit",
                format!(
                    "camoufox exited before CDP endpoint became ready (exit_code={:?})",
                    status.code()
                ),
                Some("launch"),
                None,
            ));
        }
        match client.get(&version_url).send().await {
            Ok(response) if response.status().is_success() => {
                let version = response
                    .json::<BrowserVersionResponse>()
                    .await
                    .map_err(|err| {
                        RunnerFailure::new(
                            "cdp_protocol_error",
                            format!("failed to parse camoufox /json/version: {err}"),
                            Some("launch"),
                            Some(err.to_string()),
                        )
                    })?;
                if !version.web_socket_debugger_url.trim().is_empty() {
                    return Ok(version.web_socket_debugger_url);
                }
            }
            _ => {}
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn shutdown_spawned_process(spawned: &mut SpawnedCamoufox) -> Result<Option<i32>, String> {
    if let Some(status) = spawned
        .child
        .try_wait()
        .map_err(|err| format!("failed to inspect camoufox before cleanup: {err}"))?
    {
        return Ok(status.code());
    }
    spawned
        .child
        .kill()
        .await
        .map_err(|err| format!("failed to terminate camoufox process: {err}"))?;
    timeout(Duration::from_secs(2), spawned.child.wait())
        .await
        .map_err(|_| "camoufox child did not exit after kill".to_string())?
        .map(|status| status.code())
        .map_err(|err| format!("failed to wait for camoufox child: {err}"))
}

async fn read_stream_to_string<R>(reader: Option<R>) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return String::new();
    };
    let mut buf = Vec::new();
    match reader.read_to_end(&mut buf).await {
        Ok(_) => String::from_utf8_lossy(&buf).to_string(),
        Err(err) => format!("<failed to read stream: {err}>"),
    }
}

async fn collect_joined_output(handle: JoinHandle<String>, label: &str) -> String {
    handle
        .await
        .unwrap_or_else(|err| format!("<{label} join error: {err}>"))
}

fn truncate_output(value: &str, limit: usize) -> String {
    let mut out = String::new();
    for ch in value.chars().take(limit) {
        out.push(ch);
    }
    if value.chars().count() > limit {
        out.push_str("...<truncated>");
    }
    out
}

fn preview_if_non_empty(value: String, limit: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(truncate_output(trimmed, limit))
    }
}

fn content_preview_metadata(
    raw: Option<&str>,
    limit: usize,
) -> (Option<String>, Option<usize>, Option<bool>) {
    raw.map(|value| {
        let length = value.chars().count();
        (
            Some(truncate_output(value, limit)),
            Some(length),
            Some(length > limit),
        )
    })
    .unwrap_or((None, None, None))
}

fn build_result(
    outcome: RunnerOutcomeStatus,
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    execution_stage: Option<&str>,
    requested_action: &str,
    action: &str,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    config_path: Option<&str>,
    binary_path: Option<&str>,
    pid: Option<u32>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    browser_result: Option<&BrowserActionResult>,
    message: impl Into<String>,
) -> RunnerExecutionResult {
    let message = message.into();
    let title = browser_result.and_then(|result| result.title.clone());
    let final_url = browser_result.and_then(|result| result.final_url.clone());
    let html_raw = browser_result.and_then(|result| result.html.as_deref());
    let text_raw = browser_result.and_then(|result| result.text.as_deref());
    let (html_preview, html_length, html_truncated) = if action == "get_html" {
        content_preview_metadata(html_raw, HTML_PREVIEW_LIMIT)
    } else {
        (None, None, None)
    };
    let (text_preview, text_length, text_truncated) = if action == "extract_text" {
        content_preview_metadata(text_raw, TEXT_PREVIEW_LIMIT)
    } else {
        (None, None, None)
    };
    let validation_signals = browser_result
        .map(|result| result.validation_signals.clone())
        .unwrap_or_default();
    let is_error = matches!(
        outcome,
        RunnerOutcomeStatus::Failed
            | RunnerOutcomeStatus::TimedOut
            | RunnerOutcomeStatus::Cancelled
    );
    let failure_scope = match outcome {
        RunnerOutcomeStatus::Succeeded => None,
        RunnerOutcomeStatus::TimedOut => Some("runner_timeout"),
        RunnerOutcomeStatus::Cancelled => Some("runner_cancelled"),
        RunnerOutcomeStatus::Failed => Some("browser_execution"),
    };
    let content_preview = if action == "get_html" {
        html_preview.clone()
    } else if action == "extract_text" {
        text_preview.clone()
    } else {
        None
    };
    let content_length = if action == "get_html" {
        html_length
    } else if action == "extract_text" {
        text_length
    } else {
        None
    };
    let content_truncated = if action == "get_html" {
        html_truncated
    } else if action == "extract_text" {
        text_truncated
    } else {
        None
    };
    let content_encoding = if action == "get_html" {
        Some("html")
    } else if action == "extract_text" {
        Some("plain")
    } else {
        None
    };
    let content_source_action = if action == "get_html" || action == "extract_text" {
        Some(action)
    } else {
        None
    };
    let content_ready = if action == "get_html" {
        html_raw.map(|value| !value.is_empty())
    } else if action == "extract_text" {
        text_raw.map(|value| !value.is_empty())
    } else {
        None
    };
    let content_kind = if action == "get_html" {
        Some("text/html")
    } else if action == "extract_text" {
        Some("text/plain")
    } else {
        None
    };

    let mut payload = Map::new();
    payload.insert("runner".to_string(), json!("camoufox"));
    payload.insert("runner_mode".to_string(), json!(CAMOUFOX_RUNNER_MODE));
    payload.insert("is_fake".to_string(), json!(false));
    payload.insert("real_browser_execution".to_string(), json!(ok));
    payload.insert(
        "browser_launch_attempted".to_string(),
        json!(binary_path.is_some()),
    );
    payload.insert("requested_action".to_string(), json!(requested_action));
    payload.insert("action".to_string(), json!(action));
    payload.insert("supported_actions".to_string(), json!(supported_actions()));
    payload.insert(
        "capability_stage".to_string(),
        json!("minimal_real_execution_v1"),
    );
    payload.insert("ok".to_string(), json!(ok));
    payload.insert("status".to_string(), json!(status));
    payload.insert("error_kind".to_string(), json!(error_kind));
    payload.insert("failure_scope".to_string(), json!(failure_scope));
    payload.insert("browser_failure_signal".to_string(), Value::Null);
    payload.insert("execution_stage".to_string(), json!(execution_stage));
    payload.insert("task_id".to_string(), json!(task.task_id));
    payload.insert("attempt".to_string(), json!(task.attempt));
    payload.insert("kind".to_string(), json!(task.kind));
    payload.insert("payload".to_string(), task.payload.clone());
    payload.insert("url".to_string(), json!(url));
    payload.insert("timeout_seconds".to_string(), json!(timeout_seconds));
    payload.insert("config_env".to_string(), json!(CONFIG_ENV));
    payload.insert("config_path".to_string(), json!(config_path));
    payload.insert("enabled_env".to_string(), json!(ENABLED_ENV));
    payload.insert("bin".to_string(), json!(binary_path));
    payload.insert("pid".to_string(), json!(pid));
    payload.insert("exit_code".to_string(), json!(exit_code));
    payload.insert("stdout_preview".to_string(), json!(stdout_preview));
    payload.insert("stderr_preview".to_string(), json!(stderr_preview));
    payload.insert("title".to_string(), json!(title));
    payload.insert("final_url".to_string(), json!(final_url));
    payload.insert("html_preview".to_string(), json!(html_preview));
    payload.insert("html_length".to_string(), json!(html_length));
    payload.insert("html_truncated".to_string(), json!(html_truncated));
    payload.insert("text_preview".to_string(), json!(text_preview));
    payload.insert("text_length".to_string(), json!(text_length));
    payload.insert("text_truncated".to_string(), json!(text_truncated));
    payload.insert("content_preview".to_string(), json!(content_preview));
    payload.insert("content_length".to_string(), json!(content_length));
    payload.insert("content_truncated".to_string(), json!(content_truncated));
    payload.insert("content_encoding".to_string(), json!(content_encoding));
    payload.insert(
        "content_source_action".to_string(),
        json!(content_source_action),
    );
    payload.insert("content_ready".to_string(), json!(content_ready));
    payload.insert("content_kind".to_string(), json!(content_kind));
    payload.insert("validation_signals".to_string(), json!(validation_signals));
    payload.insert("message".to_string(), json!(message));
    RunnerExecutionResult {
        status: outcome,
        result_json: Some(Value::Object(payload)),
        error_message: is_error.then_some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: format!("{}.execution", task.kind),
            source: "runner.camoufox".to_string(),
            severity: if is_error {
                crate::runner::types::SummaryArtifactSeverity::Error
            } else {
                crate::runner::types::SummaryArtifactSeverity::Info
            },
            title: format!("{} camoufox runner summary", task.kind),
            summary: format!(
                "camoufox minimal CDP runner action={action} status={status} message={message}"
            ),
        }],
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
    }
}

fn configuration_failure(
    task: &RunnerTask,
    requested_action: &str,
    error_kind: &'static str,
    message: impl Into<String>,
    config_path: Option<&str>,
) -> RunnerExecutionResult {
    build_result(
        RunnerOutcomeStatus::Failed,
        false,
        RUN_STATUS_FAILED,
        Some(error_kind),
        Some("configuration"),
        requested_action,
        normalize_action(requested_action).unwrap_or(requested_action),
        task,
        extract_url(&task.payload).as_deref(),
        task.timeout_seconds
            .and_then(|value| u64::try_from(value).ok()),
        config_path,
        None,
        None,
        None,
        None,
        None,
        None,
        message,
    )
}

#[async_trait]
impl TaskRunner for CamoufoxRunner {
    fn name(&self) -> &'static str {
        "camoufox"
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_timeout: true,
            supports_cancel_running: false,
            supports_artifacts: true,
        }
    }

    async fn cancel_running(&self, task_id: &str) -> RunnerCancelResult {
        RunnerCancelResult { accepted: false, message: format!("camoufox minimal runner does not keep a persistent task registry; task_id={task_id}") }
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        let requested_action = requested_action(&task);
        let action = match normalize_action(&requested_action) {
            Some(action) => action,
            None => return configuration_failure(&task, &requested_action, "invalid_action", "camoufox runner supports open_page/fetch/get_html/get_title/get_final_url/extract_text/validation_probe", config_path().as_deref()),
        };
        let url = match extract_url(&task.payload) {
            Some(url) => url,
            None => {
                return configuration_failure(
                    &task,
                    &requested_action,
                    "invalid_input",
                    "camoufox runner requires a non-empty url in task payload",
                    config_path().as_deref(),
                )
            }
        };
        if !looks_like_url(&url) {
            return configuration_failure(
                &task,
                &requested_action,
                "invalid_input",
                "camoufox runner only accepts http:// or https:// urls",
                config_path().as_deref(),
            );
        }
        let config_path = config_path();
        if !env_enabled() {
            return configuration_failure(&task, &requested_action, "runner_disabled", format!("camoufox runner is disabled; set {ENABLED_ENV}=true after configuring {CONFIG_ENV}"), config_path.as_deref());
        }
        let Some(config_path_value) = config_path.as_deref() else {
            return configuration_failure(
                &task,
                &requested_action,
                "runner_config_missing",
                format!("camoufox runner requires non-empty {CONFIG_ENV} before execution"),
                None,
            );
        };
        let config = match load_config(config_path_value) {
            Ok(config) => config,
            Err(failure) => {
                return configuration_failure(
                    &task,
                    &requested_action,
                    failure.error_kind,
                    failure.message,
                    Some(config_path_value),
                )
            }
        };
        let timeout_seconds = task.timeout_seconds.unwrap_or(20).clamp(1, 180) as u64;
        let test_ws_endpoint = std::env::var(TEST_WS_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());
        if let Some(ws_endpoint) = test_ws_endpoint {
            let execution = timeout(
                Duration::from_secs(timeout_seconds),
                perform_browser_action(&ws_endpoint, &task, action, &url),
            )
            .await;
            return match execution {
                Ok(Ok(browser_result)) => build_result(
                    RunnerOutcomeStatus::Succeeded,
                    true,
                    RUN_STATUS_SUCCEEDED,
                    None,
                    Some(if matches!(action, "get_html" | "extract_text") {
                        "output_wait"
                    } else {
                        "action"
                    }),
                    &requested_action,
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(config_path_value),
                    Some(&config.binary_path),
                    None,
                    None,
                    None,
                    None,
                    Some(&browser_result),
                    format!("camoufox test websocket completed {action} successfully"),
                ),
                Ok(Err(failure)) => build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    RUN_STATUS_FAILED,
                    Some(failure.error_kind),
                    failure.stage_hint,
                    &requested_action,
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(config_path_value),
                    Some(&config.binary_path),
                    None,
                    None,
                    None,
                    failure.stderr_hint,
                    None,
                    failure.message,
                ),
                Err(_) => build_result(
                    RunnerOutcomeStatus::TimedOut,
                    false,
                    RUN_STATUS_TIMED_OUT,
                    Some("timeout"),
                    Some("navigate"),
                    &requested_action,
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(config_path_value),
                    Some(&config.binary_path),
                    None,
                    None,
                    None,
                    Some(format!(
                        "camoufox test websocket endpoint did not finish within {timeout_seconds}s"
                    )),
                    None,
                    format!("camoufox test websocket timed out after {timeout_seconds}s"),
                ),
            };
        }

        if config.binary_path.is_empty() {
            return configuration_failure(
                &task,
                &requested_action,
                "runner_config_invalid",
                "camoufox config requires binary_path before spawning a browser",
                Some(config_path_value),
            );
        }
        if !Path::new(&config.binary_path).exists() {
            return configuration_failure(
                &task,
                &requested_action,
                "binary_not_found",
                format!("camoufox binary not found: {}", config.binary_path),
                Some(config_path_value),
            );
        }

        let port = match allocate_loopback_port() {
            Ok(port) => port,
            Err(err) => {
                return configuration_failure(
                    &task,
                    &requested_action,
                    "port_allocation_failed",
                    format!("failed to allocate camoufox CDP port: {err}"),
                    Some(config_path_value),
                )
            }
        };
        let (mut spawned, binary_path) = match spawn_camoufox(&config, port, timeout_seconds) {
            Ok(spawned) => (spawned, config.binary_path.clone()),
            Err(err) => {
                return build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    RUN_STATUS_FAILED,
                    Some("spawn_failed"),
                    Some("launch"),
                    &requested_action,
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(config_path_value),
                    Some(&config.binary_path),
                    None,
                    None,
                    None,
                    Some(err.to_string()),
                    None,
                    format!("failed to spawn camoufox binary: {err}"),
                )
            }
        };
        let pid = spawned.pid;
        let execution = timeout(Duration::from_secs(timeout_seconds), async {
            let ws_endpoint = wait_for_ws_endpoint(&mut spawned.child, port).await?;
            perform_browser_action(&ws_endpoint, &task, action, &url).await
        })
        .await;
        let shutdown_result = shutdown_spawned_process(&mut spawned).await;
        let stdout = collect_joined_output(spawned.stdout_handle, "stdout").await;
        let stderr = collect_joined_output(spawned.stderr_handle, "stderr").await;
        let stdout_preview = preview_if_non_empty(stdout, STDOUT_PREVIEW_LIMIT);
        let stderr_preview = preview_if_non_empty(stderr, STDERR_PREVIEW_LIMIT);
        let exit_code = shutdown_result.as_ref().ok().copied().flatten();

        match execution {
            Ok(Ok(browser_result)) => build_result(
                RunnerOutcomeStatus::Succeeded,
                true,
                RUN_STATUS_SUCCEEDED,
                None,
                Some(if matches!(action, "get_html" | "extract_text") {
                    "output_wait"
                } else {
                    "action"
                }),
                &requested_action,
                action,
                &task,
                Some(&url),
                Some(timeout_seconds),
                Some(config_path_value),
                Some(&binary_path),
                Some(pid),
                exit_code,
                stdout_preview,
                stderr_preview,
                Some(&browser_result),
                format!("camoufox launched and completed {action} successfully"),
            ),
            Ok(Err(failure)) => build_result(
                RunnerOutcomeStatus::Failed,
                false,
                RUN_STATUS_FAILED,
                Some(failure.error_kind),
                failure.stage_hint,
                &requested_action,
                action,
                &task,
                Some(&url),
                Some(timeout_seconds),
                Some(config_path_value),
                Some(&binary_path),
                Some(pid),
                exit_code,
                stdout_preview,
                stderr_preview.or(failure.stderr_hint),
                None,
                failure.message,
            ),
            Err(_) => build_result(
                RunnerOutcomeStatus::TimedOut,
                false,
                RUN_STATUS_TIMED_OUT,
                Some("timeout"),
                Some("navigate"),
                &requested_action,
                action,
                &task,
                Some(&url),
                Some(timeout_seconds),
                Some(config_path_value),
                Some(&binary_path),
                Some(pid),
                exit_code,
                stdout_preview,
                stderr_preview,
                None,
                format!("camoufox timed out after {timeout_seconds}s"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task() -> RunnerTask {
        RunnerTask {
            task_id: "task-camoufox".to_string(),
            attempt: 1,
            kind: "open_page".to_string(),
            payload: json!({"url": "https://example.com"}),
            timeout_seconds: Some(5),
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        }
    }

    #[tokio::test]
    async fn execute_returns_disabled_configuration_failure_without_launching_browser() {
        std::env::remove_var(ENABLED_ENV);
        std::env::remove_var(CONFIG_ENV);
        let result = CamoufoxRunner.execute(task()).await;
        let json = result.result_json.expect("result json");
        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        assert_eq!(json.get("runner").and_then(Value::as_str), Some("camoufox"));
        assert_eq!(
            json.get("runner_mode").and_then(Value::as_str),
            Some(CAMOUFOX_RUNNER_MODE)
        );
        assert_eq!(
            json.get("error_kind").and_then(Value::as_str),
            Some("runner_disabled")
        );
        assert_eq!(
            json.get("browser_launch_attempted")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn normalize_action_accepts_fetch_alias() {
        assert_eq!(normalize_action("fetch"), Some("open_page"));
        assert_eq!(
            normalize_action("validation_probe"),
            Some("validation_probe")
        );
        assert_eq!(normalize_action("unknown"), None);
    }

    #[test]
    fn load_config_accepts_binary_path() {
        let path =
            std::env::temp_dir().join(format!("camoufox-config-{}.json", std::process::id()));
        fs::write(
            &path,
            json!({"binary_path":"C:/Tools/camoufox/camoufox.exe","extra_args":["--headless"]})
                .to_string(),
        )
        .expect("write config");
        let loaded = load_config(path.to_str().expect("path")).expect("load config");
        assert_eq!(loaded.binary_path, "C:/Tools/camoufox/camoufox.exe");
        assert_eq!(loaded.extra_args, vec!["--headless"]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_config_allows_empty_binary_for_test_websocket_mode() {
        let path = std::env::temp_dir().join(format!(
            "camoufox-test-ws-config-{}.json",
            std::process::id()
        ));
        fs::write(&path, json!({"binary_path":""}).to_string()).expect("write config");
        let loaded = load_config(path.to_str().expect("path")).expect("load config");
        assert_eq!(loaded.binary_path, "");
        let _ = fs::remove_file(path);
    }
}
