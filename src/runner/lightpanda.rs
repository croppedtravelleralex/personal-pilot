use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io,
    net::TcpListener,
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    task::JoinHandle,
    time::{sleep, timeout, Duration, Instant},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    behavior::{
        form::{
            build_form_action_summary_json, FORM_ACTION_STATUS_BLOCKED, FORM_ACTION_STATUS_FAILED,
            FORM_ACTION_STATUS_SHADOW_ONLY, FORM_ACTION_STATUS_SUCCEEDED,
        },
        BehaviorBudget, BehaviorRuntimeExplain, BehaviorTraceSummary,
    },
    domain::run::RUN_STATUS_TIMED_OUT,
    network_identity::fingerprint_consumption::{
        build_lightpanda_runtime_projection, FingerprintConsumptionSnapshot,
        FINGERPRINT_CONSUMPTION_SOURCE_RUNTIME,
    },
    runner::{
        RunnerBehaviorPlan, RunnerCancelResult, RunnerCapabilities, RunnerExecutionResult,
        RunnerFormActionPlan, RunnerFormErrorSignals, RunnerFormFieldPlan, RunnerOutcomeStatus,
        RunnerTask, TaskRunner,
    },
};

const HTML_PREVIEW_LIMIT: usize = 4000;
const TEXT_PREVIEW_LIMIT: usize = 4000;
const STDOUT_PREVIEW_LIMIT: usize = 4000;
const STDERR_PREVIEW_LIMIT: usize = 2000;
const LIGHTPANDA_WAIT_POLL_MS: u64 = 200;

type CdpSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Clone, Default)]
pub struct LightpandaRunner {
    running_tasks: Arc<Mutex<HashMap<String, u32>>>,
}

#[derive(Debug, Clone)]
struct LightpandaFingerprintRuntime {
    envs: Vec<(String, String)>,
    applied_fields: Vec<String>,
    ignored_fields: Vec<String>,
    consumption: FingerprintConsumptionSnapshot,
}

#[derive(Debug, Clone, Default)]
struct BrowserActionResult {
    title: Option<String>,
    final_url: Option<String>,
    html: Option<String>,
    text: Option<String>,
    cookies: Vec<Value>,
    local_storage: Option<Value>,
    session_storage: Option<Value>,
    behavior_runtime_explain: Option<BehaviorRuntimeExplain>,
    behavior_trace_summary: Option<BehaviorTraceSummary>,
    behavior_trace_lines: Vec<String>,
    form_action_status: Option<String>,
    form_action_mode: Option<String>,
    form_action_retry_count: i64,
    form_action_summary_json: Option<Value>,
    form_action_error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct BehaviorStepExecutionRecord {
    primitive: String,
    outcome: String,
    added_latency_ms: i64,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct BehaviorExecutionRuntime {
    runtime_explain: BehaviorRuntimeExplain,
    trace_summary: BehaviorTraceSummary,
    trace_lines: Vec<String>,
}

#[derive(Debug, Clone)]
struct FormActionRuntime {
    status: String,
    mode: String,
    retry_count: i64,
    failure_signal: Option<String>,
    summary_json: Value,
    trace_lines: Vec<String>,
    session_persisted: bool,
    post_login_actions_executed: bool,
    success_ready_selector_seen: bool,
    behavior_runtime_explain: BehaviorRuntimeExplain,
    behavior_trace_summary: BehaviorTraceSummary,
    error_message: Option<String>,
}

#[derive(Debug)]
enum FormActionExecutionError {
    Fatal(RunnerFailure),
    Retryable { signal: String, message: String },
    Terminal { signal: String, message: String },
}

#[derive(Debug)]
struct RunnerFailure {
    error_kind: &'static str,
    message: String,
    stage_hint: Option<&'static str>,
    stderr_hint: Option<String>,
}

#[derive(Debug)]
struct CdpNavigateResponse {
    error_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LightpandaVersionResponse {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
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

struct SpawnedLightpanda {
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

impl BrowserActionResult {
    fn from_readiness(snapshot: BrowserReadinessSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: None,
            text: None,
            cookies: Vec::new(),
            local_storage: None,
            session_storage: None,
            behavior_runtime_explain: None,
            behavior_trace_summary: None,
            behavior_trace_lines: Vec::new(),
            form_action_status: None,
            form_action_mode: None,
            form_action_retry_count: 0,
            form_action_summary_json: None,
            form_action_error_message: None,
        }
    }

    fn from_html(snapshot: BrowserHtmlSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: Some(snapshot.html),
            text: None,
            cookies: Vec::new(),
            local_storage: None,
            session_storage: None,
            behavior_runtime_explain: None,
            behavior_trace_summary: None,
            behavior_trace_lines: Vec::new(),
            form_action_status: None,
            form_action_mode: None,
            form_action_retry_count: 0,
            form_action_summary_json: None,
            form_action_error_message: None,
        }
    }

    fn from_text(snapshot: BrowserTextSnapshot) -> Self {
        Self {
            title: non_empty_trimmed(snapshot.title),
            final_url: non_empty_trimmed(snapshot.final_url),
            html: None,
            text: Some(snapshot.text),
            cookies: Vec::new(),
            local_storage: None,
            session_storage: None,
            behavior_runtime_explain: None,
            behavior_trace_summary: None,
            behavior_trace_lines: Vec::new(),
            form_action_status: None,
            form_action_mode: None,
            form_action_retry_count: 0,
            form_action_summary_json: None,
            form_action_error_message: None,
        }
    }
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn json_value_literal(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn string_from_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(item) => Some(item.clone()),
        Value::Bool(item) => Some(item.to_string()),
        Value::Number(item) => Some(item.to_string()),
        other => Some(other.to_string()),
    }
}

fn bool_from_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(item) => Some(*item),
        Value::Number(item) => item.as_i64().map(|raw| raw != 0),
        Value::String(item) => {
            let normalized = item.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" | "checked" => Some(true),
                "false" | "0" | "no" | "off" | "unchecked" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

fn apply_non_active_form_action_result(result: &mut BrowserActionResult, task: &RunnerTask) {
    let Some(plan) = task.form_action_plan.as_ref() else {
        return;
    };
    result.form_action_mode = Some(plan.mode.clone());
    result.form_action_retry_count = 0;
    if plan.execution_mode != "active" {
        result.form_action_status = Some(FORM_ACTION_STATUS_SHADOW_ONLY.to_string());
        let mut summary = build_form_action_summary_json(
            plan,
            FORM_ACTION_STATUS_SHADOW_ONLY,
            0,
            plan.blocked_reason.as_deref(),
            None,
        );
        set_form_action_summary_flags(&mut summary, false, false, false);
        result.form_action_summary_json = Some(summary);
        return;
    }

    if plan.blocked_reason.is_some() {
        let mut summary = build_form_action_summary_json(
            plan,
            FORM_ACTION_STATUS_BLOCKED,
            0,
            plan.blocked_reason.as_deref(),
            None,
        );
        set_form_action_summary_flags(&mut summary, false, false, false);
        result.form_action_status = Some(FORM_ACTION_STATUS_BLOCKED.to_string());
        result.form_action_summary_json = Some(summary);
    }
}

fn apply_form_action_runtime(result: &mut BrowserActionResult, runtime: FormActionRuntime) {
    let FormActionRuntime {
        status,
        mode,
        retry_count,
        failure_signal,
        mut summary_json,
        trace_lines,
        session_persisted,
        post_login_actions_executed,
        success_ready_selector_seen,
        behavior_runtime_explain,
        mut behavior_trace_summary,
        error_message,
    } = runtime;
    set_form_action_summary_flags(
        &mut summary_json,
        success_ready_selector_seen,
        post_login_actions_executed,
        session_persisted,
    );
    behavior_trace_summary.session_persisted = session_persisted;
    result.form_action_status = Some(status);
    result.form_action_mode = Some(mode);
    result.form_action_retry_count = retry_count;
    result.form_action_summary_json = Some(summary_json);
    result.form_action_error_message = error_message.or_else(|| {
        failure_signal.map(|signal| format!("lightpanda active form action failed: {signal}"))
    });
    result.behavior_runtime_explain = Some(behavior_runtime_explain);
    result.behavior_trace_summary = Some(behavior_trace_summary);
    result.behavior_trace_lines = trace_lines;
}

fn set_form_action_summary_flags(
    summary_json: &mut Value,
    success_ready_selector_seen: bool,
    post_login_actions_executed: bool,
    session_persisted: bool,
) {
    if let Some(obj) = summary_json.as_object_mut() {
        obj.insert(
            "success_ready_selector_seen".to_string(),
            json!(success_ready_selector_seen),
        );
        obj.insert(
            "post_login_actions_executed".to_string(),
            json!(post_login_actions_executed),
        );
        obj.insert("session_persisted".to_string(), json!(session_persisted));
    }
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

impl CdpClient {
    async fn connect(ws_endpoint: &str) -> Result<Self, RunnerFailure> {
        let (socket, _) = connect_async(ws_endpoint).await.map_err(|err| {
            RunnerFailure::new(
                "cdp_connect_failed",
                format!("failed to connect to lightpanda websocket endpoint: {err}"),
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
                    "lightpanda createTarget response did not include targetId",
                    Some("launch"),
                    None,
                )
            })
    }

    async fn attach_to_target(&mut self, target_id: &str) -> Result<String, RunnerFailure> {
        let response = self
            .send_command(
                "Target.attachToTarget",
                json!({
                    "targetId": target_id,
                    "flatten": true
                }),
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
                    "lightpanda attachToTarget response did not include sessionId",
                    Some("launch"),
                    None,
                )
            })
    }

    async fn enable_page_and_runtime(&mut self, session_id: &str) -> Result<(), RunnerFailure> {
        self.send_command("Page.enable", json!({}), Some(session_id))
            .await?;
        self.send_command("Runtime.enable", json!({}), Some(session_id))
            .await?;
        self.send_command("Network.enable", json!({}), Some(session_id))
            .await?;
        Ok(())
    }

    async fn set_cookies(
        &mut self,
        session_id: &str,
        cookies: &[Value],
    ) -> Result<(), RunnerFailure> {
        if cookies.is_empty() {
            return Ok(());
        }

        self.send_command(
            "Network.setCookies",
            json!({
                "cookies": cookies,
            }),
            Some(session_id),
        )
        .await?;
        Ok(())
    }

    async fn get_cookies(
        &mut self,
        session_id: &str,
        url: &str,
    ) -> Result<Vec<Value>, RunnerFailure> {
        let response = self
            .send_command(
                "Network.getCookies",
                json!({
                    "urls": [url],
                }),
                Some(session_id),
            )
            .await?;
        Ok(response
            .pointer("/result/cookies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn set_storage(
        &mut self,
        session_id: &str,
        local_storage: Option<&Value>,
        session_storage: Option<&Value>,
    ) -> Result<(), RunnerFailure> {
        if local_storage.is_none() && session_storage.is_none() {
            return Ok(());
        }

        self.evaluate_json(
            session_id,
            &storage_restore_expression(local_storage, session_storage),
        )
        .await?;
        Ok(())
    }

    async fn get_storage(
        &mut self,
        session_id: &str,
    ) -> Result<(Option<Value>, Option<Value>), RunnerFailure> {
        let value = self
            .evaluate_json(session_id, storage_snapshot_expression())
            .await?;
        Ok((
            value.get("localStorage").cloned().filter(Value::is_object),
            value
                .get("sessionStorage")
                .cloned()
                .filter(Value::is_object),
        ))
    }

    async fn navigate(
        &mut self,
        session_id: &str,
        url: &str,
    ) -> Result<CdpNavigateResponse, RunnerFailure> {
        let response = self
            .send_command("Page.navigate", json!({ "url": url }), Some(session_id))
            .await?;

        Ok(CdpNavigateResponse {
            error_text: response
                .pointer("/result/errorText")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
    }

    async fn evaluate_json(
        &mut self,
        session_id: &str,
        expression: &str,
    ) -> Result<Value, RunnerFailure> {
        let response = self
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
                Some(session_id),
            )
            .await?;

        if let Some(description) = response
            .pointer("/result/result/description")
            .and_then(Value::as_str)
        {
            if response.pointer("/result/exceptionDetails").is_some() {
                return Err(RunnerFailure::new(
                    "cdp_evaluate_failed",
                    format!("lightpanda evaluation failed: {description}"),
                    Some("action"),
                    Some(description.to_string()),
                ));
            }
        }

        response
            .pointer("/result/result/value")
            .cloned()
            .ok_or_else(|| {
                RunnerFailure::new(
                    "cdp_protocol_error",
                    "lightpanda evaluation did not return a JSON value",
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
                format!("failed to decode readiness snapshot: {err}"),
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
                    format!("failed to decode html snapshot: {err}"),
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
                    format!("failed to decode text snapshot: {err}"),
                    Some("output_wait"),
                    Some(err.to_string()),
                )
            },
        )
    }

    async fn send_command(
        &mut self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, RunnerFailure> {
        self.next_id += 1;
        let command_id = self.next_id;

        let mut payload = json!({
            "id": command_id,
            "method": method,
            "params": params,
        });
        if let Some(session_id) = session_id {
            payload["sessionId"] = Value::String(session_id.to_string());
        }

        self.socket
            .send(Message::Text(payload.to_string().into()))
            .await
            .map_err(|err| {
                RunnerFailure::new(
                    "cdp_send_failed",
                    format!("failed to send CDP command {method}: {err}"),
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
                .unwrap_or_else(|| error.as_str().unwrap_or("unknown CDP error"));
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
            let next = self.socket.next().await;
            let Some(next) = next else {
                return Err(RunnerFailure::new(
                    "runner_connection_closed",
                    "lightpanda websocket stream ended unexpectedly",
                    Some("action"),
                    Some("websocket stream ended".to_string()),
                ));
            };

            let message = next.map_err(|err| {
                RunnerFailure::new(
                    "runner_connection_closed",
                    format!("lightpanda websocket read failed: {err}"),
                    Some("action"),
                    Some(err.to_string()),
                )
            })?;

            match message {
                Message::Text(text) => {
                    return serde_json::from_str::<Value>(&text).map_err(|err| {
                        RunnerFailure::new(
                            "cdp_protocol_error",
                            format!("failed to decode CDP text message: {err}"),
                            Some("action"),
                            Some(err.to_string()),
                        )
                    });
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice::<Value>(&bytes).map_err(|err| {
                        RunnerFailure::new(
                            "cdp_protocol_error",
                            format!("failed to decode CDP binary message: {err}"),
                            Some("action"),
                            Some(err.to_string()),
                        )
                    });
                }
                Message::Ping(payload) => {
                    self.socket
                        .send(Message::Pong(payload))
                        .await
                        .map_err(|err| {
                            RunnerFailure::new(
                                "runner_connection_closed",
                                format!("failed to respond to websocket ping: {err}"),
                                Some("action"),
                                Some(err.to_string()),
                            )
                        })?;
                }
                Message::Pong(_) => {}
                Message::Close(frame) => {
                    return Err(RunnerFailure::new(
                        "runner_connection_closed",
                        format!("lightpanda websocket closed: {frame:?}"),
                        Some("action"),
                        Some("websocket closed".to_string()),
                    ));
                }
                Message::Frame(_) => {}
            }
        }
    }
}

fn profile_value_as_env_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => {
            let trimmed = v.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Bool(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}

fn build_lightpanda_fingerprint_runtime(task: &RunnerTask) -> Option<LightpandaFingerprintRuntime> {
    let profile = task.fingerprint_profile.as_ref()?;
    let projection =
        build_lightpanda_runtime_projection(&profile.id, profile.version, &profile.profile_json);

    Some(LightpandaFingerprintRuntime {
        envs: projection.envs,
        applied_fields: projection.consumption.applied_fields.clone(),
        ignored_fields: projection.consumption.ignored_fields.clone(),
        consumption: projection.consumption,
    })
}

fn result_payload(
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    failure_scope: Option<&str>,
    browser_failure_signal: Option<&str>,
    execution_stage: Option<&str>,
    requested_action: &str,
    action: &str,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    browser_result: Option<&BrowserActionResult>,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
    message: &str,
) -> Value {
    let fingerprint_profile = task.fingerprint_profile.as_ref().map(|profile| {
        json!({
            "id": profile.id,
            "version": profile.version,
            "profile": profile.profile_json,
        })
    });
    let proxy_json = task.proxy.as_ref().map(|proxy| {
        json!({
            "id": proxy.id,
            "scheme": proxy.scheme,
            "host": proxy.host,
            "port": proxy.port,
            "region": proxy.region,
            "country": proxy.country,
            "provider": proxy.provider,
            "score": proxy.score,
            "resolution_status": proxy.resolution_status,
        })
    });

    let title = browser_result.and_then(|result| result.title.clone());
    let final_url = browser_result.and_then(|result| result.final_url.clone());
    let html_raw = browser_result.and_then(|result| result.html.as_deref());
    let text_raw = browser_result.and_then(|result| result.text.as_deref());
    let behavior_runtime_explain =
        browser_result.and_then(|result| result.behavior_runtime_explain.clone());
    let behavior_trace_summary =
        browser_result.and_then(|result| result.behavior_trace_summary.clone());
    let behavior_trace_lines = browser_result
        .map(|result| result.behavior_trace_lines.clone())
        .unwrap_or_default();
    let form_action_status = browser_result.and_then(|result| result.form_action_status.clone());
    let form_action_mode = browser_result.and_then(|result| result.form_action_mode.clone());
    let form_action_retry_count = browser_result.map(|result| result.form_action_retry_count);
    let form_action_summary_json =
        browser_result.and_then(|result| result.form_action_summary_json.clone());
    let cookies = browser_result.map(|result| result.cookies.clone());
    let local_storage = browser_result.and_then(|result| result.local_storage.clone());
    let session_storage = browser_result.and_then(|result| result.session_storage.clone());

    let (html_preview, html_length, html_truncated) = match action {
        "get_html" => content_preview_metadata(html_raw, HTML_PREVIEW_LIMIT),
        _ => (None, None, None),
    };
    let (text_preview, text_length, text_truncated) = match action {
        "extract_text" => content_preview_metadata(text_raw, TEXT_PREVIEW_LIMIT),
        _ => (None, None, None),
    };

    let (content_kind, content_preview, content_length, content_truncated, content_encoding) =
        match action {
            "get_html" => (
                Some("text/html"),
                html_preview.clone(),
                html_length,
                html_truncated,
                Some("html"),
            ),
            "extract_text" => (
                Some("text/plain"),
                text_preview.clone(),
                text_length,
                text_truncated,
                Some("plain"),
            ),
            _ => (None, None, None, None, None),
        };

    let content_source_action = match action {
        "get_html" | "extract_text" => Some(action),
        _ => None,
    };

    let content_ready = match action {
        "get_html" => Some(html_raw.map(|raw| !raw.is_empty()).unwrap_or(false)),
        "extract_text" => Some(text_raw.map(|raw| !raw.is_empty()).unwrap_or(false)),
        _ => None,
    };

    let fingerprint_runtime_json = fingerprint_runtime.map(|runtime| {
        let supported_field_count = runtime.consumption.applied_count();
        let unsupported_field_count = runtime.consumption.ignored_count();
        let consumption_status = fingerprint_consumption_status(runtime);

        json!({
            "env_keys": runtime.envs.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>(),
            "applied_fields": runtime.applied_fields,
            "ignored_fields": runtime.ignored_fields,
            "applied_count": runtime.applied_fields.len(),
            "ignored_count": runtime.ignored_fields.len(),
            "supported_field_count": supported_field_count,
            "unsupported_field_count": unsupported_field_count,
            "consumption_status": consumption_status,
            "consumption_source_of_truth": FINGERPRINT_CONSUMPTION_SOURCE_RUNTIME,
            "consumption_version": runtime.consumption.consumption_version,
            "warning": runtime.consumption.partial_support_warning,
            "consumption_explain": {
                "declared_fields": runtime.consumption.declared_fields,
                "resolved_fields": runtime.consumption.resolved_fields,
                "applied_fields": runtime.consumption.applied_fields,
                "ignored_fields": runtime.consumption.ignored_fields,
                "declared_count": runtime.consumption.declared_count(),
                "resolved_count": runtime.consumption.resolved_count(),
                "applied_count": runtime.consumption.applied_count(),
                "ignored_count": runtime.consumption.ignored_count(),
                "consumption_status": consumption_status,
                "consumption_version": runtime.consumption.consumption_version,
                "partial_support_warning": runtime.consumption.partial_support_warning
            }
        })
    });

    let mut payload = json!({
        "runner": "lightpanda",
        "requested_action": requested_action,
        "action": action,
        "supported_actions": supported_actions(),
        "capability_stage": "minimal_real_execution_v1",
        "ok": ok,
        "status": status,
        "error_kind": error_kind,
        "failure_scope": failure_scope,
        "browser_failure_signal": browser_failure_signal,
        "execution_stage": execution_stage,
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": timeout_seconds,
        "fingerprint_profile": fingerprint_profile,
        "proxy": proxy_json,
        "fingerprint_runtime": fingerprint_runtime_json,
        "bin": bin,
        "exit_code": exit_code,
        "stdout_preview": stdout_preview,
        "stderr_preview": stderr_preview,
        "title": title,
        "final_url": final_url,
        "html_preview": html_preview,
        "html_length": html_length,
        "html_truncated": html_truncated,
        "text_preview": text_preview,
        "text_length": text_length,
        "text_truncated": text_truncated,
        "content_preview": content_preview,
        "content_length": content_length,
        "content_truncated": content_truncated,
        "content_encoding": content_encoding,
        "content_source_action": content_source_action,
        "content_ready": content_ready,
        "content_kind": content_kind,
        "message": message,
    });

    if let Value::Object(ref mut obj) = payload {
        obj.insert("form_action_status".to_string(), json!(form_action_status));
        obj.insert("form_action_mode".to_string(), json!(form_action_mode));
        obj.insert(
            "form_action_retry_count".to_string(),
            json!(form_action_retry_count),
        );
        obj.insert(
            "form_action_summary_json".to_string(),
            json!(form_action_summary_json),
        );
        obj.insert(
            "behavior_runtime_explain".to_string(),
            serde_json::to_value(behavior_runtime_explain).unwrap_or(Value::Null),
        );
        obj.insert(
            "behavior_trace_summary".to_string(),
            serde_json::to_value(behavior_trace_summary).unwrap_or(Value::Null),
        );
        obj.insert(
            "behavior_trace_lines".to_string(),
            json!(behavior_trace_lines),
        );
        obj.insert("cookies".to_string(), json!(cookies));
        obj.insert("local_storage".to_string(), json!(local_storage));
        obj.insert("session_storage".to_string(), json!(session_storage));
    }

    payload
}

fn build_result(
    outcome: RunnerOutcomeStatus,
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    browser_failure_signal_override: Option<&str>,
    execution_stage_override: Option<&str>,
    requested_action: &str,
    action: &str,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    browser_result: Option<&BrowserActionResult>,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
    message: impl Into<String>,
) -> RunnerExecutionResult {
    let message = message.into();
    let is_error = matches!(
        outcome,
        RunnerOutcomeStatus::Failed
            | RunnerOutcomeStatus::TimedOut
            | RunnerOutcomeStatus::Cancelled
    );
    let browser_failure_signal = browser_failure_signal_override.or_else(|| {
        detect_browser_failure_signal(stderr_preview.as_deref(), stdout_preview.as_deref())
    });
    let failure_scope = match outcome {
        RunnerOutcomeStatus::Cancelled => Some("runner_cancelled"),
        _ => is_error.then_some(runner_failure_scope(
            error_kind,
            browser_failure_signal,
            matches!(outcome, RunnerOutcomeStatus::TimedOut),
        )),
    };
    let execution_stage = execution_stage_override.or_else(|| {
        classify_execution_stage(
            outcome,
            error_kind,
            browser_failure_signal,
            action,
            browser_result,
            stdout_preview.as_deref(),
            stderr_preview.as_deref(),
        )
    });

    let (html_length, text_length) = match browser_result {
        Some(result) => (
            result.html.as_ref().map(|value| value.chars().count()),
            result.text.as_ref().map(|value| value.chars().count()),
        ),
        None => (None, None),
    };

    RunnerExecutionResult {
        status: outcome,
        result_json: Some(result_payload(
            ok,
            status,
            error_kind,
            failure_scope,
            browser_failure_signal,
            execution_stage,
            requested_action,
            action,
            task,
            url,
            timeout_seconds,
            bin,
            exit_code,
            stdout_preview,
            stderr_preview,
            browser_result,
            fingerprint_runtime,
            &message,
        )),
        error_message: matches!(outcome, RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut)
            .then_some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: format!("{}.execution", task.kind),
            source: "runner.lightpanda".to_string(),
            severity: match outcome {
                RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut => {
                    crate::runner::types::SummaryArtifactSeverity::Error
                }
                RunnerOutcomeStatus::Cancelled => {
                    crate::runner::types::SummaryArtifactSeverity::Warning
                }
                RunnerOutcomeStatus::Succeeded => crate::runner::types::SummaryArtifactSeverity::Info,
            },
            title: execution_summary_title(action, status, error_kind),
            summary: format!(
                "{} failure_scope={} browser_failure_signal={} execution_stage={} content_kind={} html_length={} text_length={}",
                execution_summary_text(action, task, status, error_kind, exit_code, timeout_seconds, &message),
                failure_scope.unwrap_or("none"),
                browser_failure_signal.unwrap_or("none"),
                execution_stage.unwrap_or("none"),
                match action {
                    "get_html" => "text/html",
                    "extract_text" => "text/plain",
                    _ => "none",
                },
                html_length
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                text_length
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            ),
        }],
        session_cookies: browser_result
            .map(|result| result.cookies.clone())
            .filter(|cookies| !cookies.is_empty()),
        session_local_storage: browser_result
            .and_then(|result| result.local_storage.clone())
            .filter(Value::is_object),
        session_session_storage: browser_result
            .and_then(|result| result.session_storage.clone())
            .filter(Value::is_object),
    }
}

fn invalid_input(
    task: &RunnerTask,
    requested_action: &str,
    action: &str,
    message: &str,
    url: Option<&str>,
) -> RunnerExecutionResult {
    build_result(
        RunnerOutcomeStatus::Failed,
        false,
        "failed",
        Some("invalid_input"),
        None,
        Some("launch"),
        requested_action,
        action,
        task,
        url,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        message,
    )
}

fn extract_action(task: &RunnerTask) -> String {
    if let Some(action) = task
        .payload
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return action.to_string();
    }

    match task.kind.as_str() {
        "open_page" | "fetch" | "get_html" | "get_title" | "get_final_url" | "extract_text" => {
            task.kind.clone()
        }
        _ => "open_page".to_string(),
    }
}

fn normalize_action(action: &str) -> Option<&'static str> {
    match action {
        "open_page" | "fetch" => Some("open_page"),
        "get_html" => Some("get_html"),
        "get_title" => Some("get_title"),
        "get_final_url" => Some("get_final_url"),
        "extract_text" => Some("extract_text"),
        _ => None,
    }
}

fn supported_actions() -> &'static [&'static str] {
    &[
        "open_page",
        "fetch",
        "get_html",
        "get_title",
        "get_final_url",
        "extract_text",
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
    url.starts_with("http://") || url.starts_with("https://")
}

fn lightpanda_bin() -> String {
    std::env::var("LIGHTPANDA_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "lightpanda".to_string())
}

fn lightpanda_test_ws_endpoint() -> Option<String> {
    std::env::var("LIGHTPANDA_TEST_WS_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn truncate_output(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    let total_chars = trimmed.chars().count();
    let mut preview = trimmed.chars().take(max_chars).collect::<String>();
    if total_chars > max_chars {
        preview.push_str("...[truncated]");
    }
    preview
}

fn preview_if_non_empty(raw: String, max_chars: usize) -> Option<String> {
    let preview = truncate_output(&raw, max_chars);
    (!preview.is_empty()).then_some(preview)
}

fn content_preview_metadata(
    raw: Option<&str>,
    max_chars: usize,
) -> (Option<String>, Option<usize>, Option<bool>) {
    match raw {
        Some(raw) => {
            let length = raw.chars().count();
            let preview = truncate_output(raw, max_chars);
            let preview_value = (!preview.is_empty()).then_some(preview);
            (preview_value, Some(length), Some(length > max_chars))
        }
        None => (None, Some(0), Some(false)),
    }
}

fn classify_spawn_error(err: &io::Error) -> &'static str {
    match err.kind() {
        io::ErrorKind::NotFound => "binary_not_found",
        io::ErrorKind::PermissionDenied => "spawn_permission_denied",
        _ => "spawn_failed",
    }
}

fn classify_exit_code(exit_code: Option<i32>) -> &'static str {
    match exit_code {
        Some(126) => "runner_invocation_not_executable",
        Some(127) => "runner_command_not_found",
        Some(143) => "runner_cancelled",
        Some(code) if code >= 128 => "runner_terminated_by_signal",
        Some(_) => "runner_non_zero_exit",
        None => "runner_non_zero_exit",
    }
}

fn execution_summary_title(action: &str, status: &str, error_kind: Option<&str>) -> String {
    match (status, error_kind) {
        ("succeeded", _) => format!("{action} execution succeeded"),
        ("timed_out", Some("timeout")) => format!("{action} execution timed out"),
        ("failed", Some(kind)) => format!("{action} execution failed ({kind})"),
        ("failed", None) => format!("{action} execution failed"),
        _ => format!("{action} execution {status}"),
    }
}

fn execution_summary_text(
    action: &str,
    task: &RunnerTask,
    status: &str,
    error_kind: Option<&str>,
    exit_code: Option<i32>,
    timeout_seconds: Option<u64>,
    message: &str,
) -> String {
    format!(
        "kind={} action={} status={} error_kind={} exit_code={:?} timeout_seconds={:?} message={}",
        task.kind,
        action,
        status,
        error_kind.unwrap_or("none"),
        exit_code,
        timeout_seconds,
        message,
    )
}

fn detect_browser_failure_signal(
    stderr_preview: Option<&str>,
    stdout_preview: Option<&str>,
) -> Option<&'static str> {
    let stderr = stderr_preview.unwrap_or("").to_ascii_lowercase();
    let stdout = stdout_preview.unwrap_or("").to_ascii_lowercase();
    let combined = format!("{stderr}\n{stdout}");

    if combined.contains("timeout") || combined.contains("timed out") {
        Some("browser_timeout_signal")
    } else if combined.contains("navigation") && combined.contains("fail") {
        Some("browser_navigation_failure_signal")
    } else if combined.contains("dns") || combined.contains("name not resolved") {
        Some("browser_dns_failure_signal")
    } else if combined.contains("certificate")
        || combined.contains("tls")
        || combined.contains("ssl")
    {
        Some("browser_tls_failure_signal")
    } else {
        None
    }
}

fn runner_failure_scope(
    error_kind: Option<&str>,
    browser_failure_signal: Option<&str>,
    timed_out: bool,
) -> &'static str {
    if timed_out {
        "runner_timeout"
    } else if error_kind == Some("runner_cancelled") {
        "runner_cancelled"
    } else if browser_failure_signal.is_some() {
        "browser_execution"
    } else {
        match error_kind {
            Some("binary_not_found") | Some("spawn_permission_denied") | Some("spawn_failed") => {
                "runner_invocation"
            }
            Some("process_wait_failed") => "runner_process_wait",
            Some(
                "runner_command_not_found"
                | "runner_invocation_not_executable"
                | "runner_terminated_by_signal"
                | "runner_non_zero_exit",
            ) => "runner_process_exit",
            _ => "runner_execution",
        }
    }
}

fn classify_execution_stage(
    outcome: RunnerOutcomeStatus,
    error_kind: Option<&str>,
    browser_failure_signal: Option<&str>,
    action: &str,
    browser_result: Option<&BrowserActionResult>,
    stdout_preview: Option<&str>,
    stderr_preview: Option<&str>,
) -> Option<&'static str> {
    let has_result = browser_result.is_some();
    let has_output_preview = stdout_preview.is_some_and(|value| !value.trim().is_empty());
    let is_content_action = matches!(action, "get_html" | "extract_text");

    match outcome {
        RunnerOutcomeStatus::Succeeded => Some(if is_content_action {
            "output_wait"
        } else if has_result {
            "action"
        } else {
            "navigate"
        }),
        RunnerOutcomeStatus::Cancelled => Some("action"),
        RunnerOutcomeStatus::TimedOut => Some(if is_content_action && has_result {
            "output_wait"
        } else {
            "navigate"
        }),
        RunnerOutcomeStatus::Failed => {
            if matches!(
                error_kind,
                Some(
                    "binary_not_found"
                        | "spawn_permission_denied"
                        | "spawn_failed"
                        | "runner_command_not_found"
                        | "runner_invocation_not_executable"
                        | "cdp_connect_failed"
                )
            ) {
                Some("launch")
            } else if matches!(
                browser_failure_signal,
                Some(
                    "browser_navigation_failure_signal"
                        | "browser_dns_failure_signal"
                        | "browser_tls_failure_signal"
                )
            ) {
                Some("navigate")
            } else if browser_failure_signal == Some("browser_timeout_signal")
                || (is_content_action && has_result)
            {
                Some("output_wait")
            } else if error_kind == Some("process_wait_failed")
                || stderr_preview.is_some_and(|value| !value.trim().is_empty())
                || has_output_preview
            {
                Some("action")
            } else {
                Some("action")
            }
        }
    }
}

fn fingerprint_consumption_status(runtime: &LightpandaFingerprintRuntime) -> &'static str {
    let supported_field_count = runtime
        .applied_fields
        .iter()
        .filter(|field| *field != "profile_id" && *field != "profile_version")
        .count();
    let unsupported_field_count = runtime.ignored_fields.len();
    if supported_field_count == 0 && unsupported_field_count == 0 {
        "metadata_only"
    } else if supported_field_count == 0 {
        "ignored_only"
    } else if unsupported_field_count == 0 {
        "fully_consumed"
    } else {
        "partially_consumed"
    }
}

fn readiness_expression() -> &'static str {
    r#"(() => {
  const doc = document;
  return {
    readyState: doc ? (doc.readyState || "") : "",
    title: doc ? (doc.title || "") : "",
    href: typeof window !== "undefined" && window.location ? (window.location.href || "") : "",
    hasHtml: !!(doc && doc.documentElement),
    hasBody: !!(doc && doc.body)
  };
})()"#
}

fn html_expression() -> &'static str {
    r#"(() => {
  const doc = document;
  return {
    title: doc ? (doc.title || "") : "",
    href: typeof window !== "undefined" && window.location ? (window.location.href || "") : "",
    html: doc && doc.documentElement ? doc.documentElement.outerHTML : ""
  };
})()"#
}

fn text_expression() -> &'static str {
    r#"(() => {
  const doc = document;
  return {
    title: doc ? (doc.title || "") : "",
    href: typeof window !== "undefined" && window.location ? (window.location.href || "") : "",
    text: doc && doc.body ? doc.body.innerText : ""
  };
})()"#
}

fn storage_restore_expression(
    local_storage: Option<&Value>,
    session_storage: Option<&Value>,
) -> String {
    let payload = json!({
        "localStorage": local_storage.cloned().unwrap_or(Value::Null),
        "sessionStorage": session_storage.cloned().unwrap_or(Value::Null),
    });
    format!(
        r#"(() => {{
  const __PP_STORAGE_RESTORE__ = true;
  const payload = {payload};
  const normalize = (value) => {{
    if (!value || typeof value !== "object" || Array.isArray(value)) {{
      return [];
    }}
    return Object.entries(value);
  }};
  const writeStore = (store, entries) => {{
    let count = 0;
    for (const [key, value] of entries) {{
      store.setItem(String(key), typeof value === "string" ? value : JSON.stringify(value));
      count += 1;
    }}
    return count;
  }};
  return {{
    localStorageRestoreCount: writeStore(window.localStorage, normalize(payload.localStorage)),
    sessionStorageRestoreCount: writeStore(window.sessionStorage, normalize(payload.sessionStorage)),
  }};
}})()"#,
        payload = payload,
    )
}

fn storage_snapshot_expression() -> &'static str {
    r#"(() => {
  const __PP_STORAGE_SNAPSHOT__ = true;
  const readStore = (store) => {
    const items = {};
    for (let i = 0; i < store.length; i += 1) {
      const key = store.key(i);
      if (key !== null) {
        items[key] = store.getItem(key);
      }
    }
    return items;
  };
  return {
    localStorage: readStore(window.localStorage),
    sessionStorage: readStore(window.sessionStorage),
  };
})()"#
}

fn content_metrics_expression() -> &'static str {
    r#"(() => {
  const doc = document;
  const root = doc ? (doc.scrollingElement || doc.documentElement || doc.body) : null;
  const body = doc ? doc.body : null;
  return {
    readyState: doc ? (doc.readyState || "") : "",
    textLength: body ? ((body.innerText || "").length) : 0,
    htmlLength: doc && doc.documentElement ? (doc.documentElement.outerHTML || "").length : 0,
    scrollHeight: root ? (root.scrollHeight || 0) : 0,
    scrollTop: root ? (root.scrollTop || 0) : 0
  };
})()"#
}

fn behavior_mode_from_task(task: &RunnerTask) -> Option<&str> {
    task.payload
        .get("behavior_policy_json")
        .and_then(|value| value.get("mode"))
        .and_then(Value::as_str)
}

fn supported_active_behavior_primitive(primitive: &str) -> bool {
    matches!(
        primitive,
        "idle"
            | "wait_for_readiness"
            | "wait_for_content_stable"
            | "scroll_progressive"
            | "scroll_to_ratio"
            | "pause_on_content"
            | "focus_element"
            | "blur_element"
            | "hover_candidate"
            | "type_with_rhythm"
            | "clear_with_corrections"
            | "persist_session_state"
            | "soft_abort_if_budget_exceeded"
    )
}

fn behavior_budget_from_plan(plan: Option<&RunnerBehaviorPlan>) -> Option<BehaviorBudget> {
    plan.and_then(|item| item.budget_json.clone())
        .and_then(|value| serde_json::from_value::<BehaviorBudget>(value).ok())
}

fn behavior_step_i64(step: &Value, field: &str) -> Option<i64> {
    step.get(field).and_then(Value::as_i64)
}

fn behavior_step_string(step: &Value, field: &str) -> Option<String> {
    step.get(field).and_then(Value::as_str).map(str::to_string)
}

fn form_seed_value(task: &RunnerTask) -> i64 {
    task.behavior_plan
        .as_ref()
        .map(|plan| {
            plan.seed
                .bytes()
                .fold(0_i64, |acc, item| acc.wrapping_add(i64::from(item)))
        })
        .unwrap_or(17)
}

#[allow(dead_code)]
fn selector_presence_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_SELECTOR_PRESENCE__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  return {{
    present: Boolean(element),
    tagName: element ? String(element.tagName || "").toLowerCase() : null
  }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn selector_visible_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_SELECTOR_VISIBLE__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  const rect = element ? element.getBoundingClientRect() : null;
  const visible = Boolean(
    element
      && rect
      && rect.width > 0
      && rect.height > 0
      && (window.getComputedStyle(element).visibility || "visible") !== "hidden"
      && (window.getComputedStyle(element).display || "block") !== "none"
  );
  return {{
    visible,
    present: Boolean(element)
  }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn field_state_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_FIELD_STATE__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  const value = element && "value" in element ? String(element.value ?? "") : "";
  return {{
    present: Boolean(element),
    value,
    checked: Boolean(element && "checked" in element ? element.checked : false),
    tagName: element ? String(element.tagName || "").toLowerCase() : null,
    inputType: element && "type" in element ? String(element.type || "").toLowerCase() : null
  }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn focus_element_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_FOCUS_ELEMENT__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  if (!element) {{
    return {{ outcome: "skipped", note: "selector_not_found" }};
  }}
  if (typeof element.focus === "function") {{
    element.focus();
  }}
  return {{ outcome: "executed", note: "focused" }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn blur_element_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_BLUR_ELEMENT__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  if (!element) {{
    return {{ outcome: "skipped", note: "selector_not_found" }};
  }}
  if (typeof element.blur === "function") {{
    element.blur();
  }}
  return {{ outcome: "executed", note: "blurred" }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn hover_candidate_expression(selector_or_heuristic: Option<&str>) -> String {
    let selector = selector_or_heuristic
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"(() => {{
  const __PP_HOVER_CANDIDATE__ = true;
  const requestedSelector = {selector};
  const isVisible = (element) => {{
    if (!element) {{
      return false;
    }}
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }};
  let element = requestedSelector ? document.querySelector(requestedSelector) : null;
  if (!element) {{
    const candidates = Array.from(document.querySelectorAll('a,button,[role="button"],nav a,[data-testid],[aria-label],input,textarea,select'));
    element = candidates.find((item) => isVisible(item)) || null;
  }}
  if (!element) {{
    return {{ outcome: "skipped", note: "no_hover_candidate" }};
  }}
  const event = new MouseEvent("mouseover", {{ bubbles: true, cancelable: true, view: window }});
  element.dispatchEvent(event);
  return {{
    outcome: "executed",
    note: requestedSelector ? "selector" : "heuristic"
  }};
}})()"#,
        selector = selector,
    )
}

fn clear_with_corrections_expression(selector: &str, seed: i64) -> String {
    format!(
        r#"(() => {{
  const __PP_CLEAR_WITH_CORRECTIONS__ = true;
  const selector = {selector};
  const seed = {seed};
  const element = document.querySelector(selector);
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const setValue = (target, value) => {{
    const proto = target instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
    const descriptor = Object.getOwnPropertyDescriptor(proto, "value");
    if (descriptor && descriptor.set) {{
      descriptor.set.call(target, value);
    }} else {{
      target.value = value;
    }}
  }};
  return (async () => {{
    if (!element) {{
      return {{ outcome: "skipped", note: "selector_not_found" }};
    }}
    const current = "value" in element ? String(element.value ?? "") : "";
    if (!current) {{
      return {{ outcome: "skipped", note: "already_empty" }};
    }}
    for (let index = current.length; index >= 0; index -= 1) {{
      const nextValue = current.slice(0, index);
      setValue(element, nextValue);
      element.dispatchEvent(new InputEvent("input", {{ bubbles: true, inputType: "deleteContentBackward", data: null }}));
      await wait(8 + ((seed + index) % 13));
    }}
    element.dispatchEvent(new Event("change", {{ bubbles: true }}));
    return {{ outcome: "executed", note: "cleared_len=" + current.length }};
  }})();
}})()"#,
        selector = json_string_literal(selector),
        seed = seed,
    )
}

fn type_with_rhythm_expression(selector: &str, text: &str, seed: i64, role: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_TYPE_WITH_RHYTHM__ = true;
  const selector = {selector};
  const text = {text};
  const seed = {seed};
  const role = {role};
  const element = document.querySelector(selector);
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const setValue = (target, value) => {{
    const proto = target instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
    const descriptor = Object.getOwnPropertyDescriptor(proto, "value");
    if (descriptor && descriptor.set) {{
      descriptor.set.call(target, value);
    }} else {{
      target.value = value;
    }}
  }};
  return (async () => {{
    if (!element) {{
      return {{ outcome: "skipped", note: "selector_not_found" }};
    }}
    let current = "value" in element ? String(element.value ?? "") : "";
    for (let index = 0; index < text.length; index += 1) {{
      current += text[index];
      setValue(element, current);
      element.dispatchEvent(new InputEvent("input", {{ bubbles: true, inputType: "insertText", data: text[index] }}));
      await wait(22 + ((seed + index + role.length) % 29));
    }}
    element.dispatchEvent(new Event("change", {{ bubbles: true }}));
    return {{ outcome: "executed", note: "typed_len=" + text.length }};
  }})();
}})()"#,
        selector = json_string_literal(selector),
        text = json_string_literal(text),
        seed = seed,
        role = json_string_literal(role),
    )
}

fn click_selector_expression(selector: &str) -> String {
    format!(
        r#"(() => {{
  const __PP_CLICK_SELECTOR__ = true;
  const selector = {selector};
  const element = document.querySelector(selector);
  if (!element) {{
    return {{ outcome: "skipped", note: "selector_not_found" }};
  }}
  const overEvent = new MouseEvent("mouseover", {{ bubbles: true, cancelable: true, view: window }});
  const downEvent = new MouseEvent("mousedown", {{ bubbles: true, cancelable: true, view: window }});
  const upEvent = new MouseEvent("mouseup", {{ bubbles: true, cancelable: true, view: window }});
  element.dispatchEvent(overEvent);
  element.dispatchEvent(downEvent);
  if (typeof element.click === "function") {{
    element.click();
  }}
  element.dispatchEvent(upEvent);
  return {{ outcome: "executed", note: "clicked" }};
}})()"#,
        selector = json_string_literal(selector),
    )
}

fn checkbox_toggle_expression(selector: &str, desired_checked: bool) -> String {
    format!(
        r#"(() => {{
  const __PP_CHECKBOX_TOGGLE__ = true;
  const selector = {selector};
  const desiredChecked = {desired_checked};
  const element = document.querySelector(selector);
  if (!element) {{
    return {{ outcome: "skipped", note: "selector_not_found" }};
  }}
  const current = Boolean("checked" in element ? element.checked : false);
  if (current === desiredChecked) {{
    return {{ outcome: "skipped", note: "already_aligned" }};
  }}
  if ("checked" in element) {{
    element.checked = desiredChecked;
  }}
  element.dispatchEvent(new Event("input", {{ bubbles: true }}));
  element.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return {{ outcome: "executed", note: "checked=" + String(desiredChecked) }};
}})()"#,
        selector = json_string_literal(selector),
        desired_checked = desired_checked,
    )
}

fn success_probe_expression(
    ready_selector: &str,
    url_patterns: &[String],
    title_contains: &[String],
) -> String {
    format!(
        r#"(() => {{
  const __PP_SUCCESS_PROBE__ = true;
  const readySelector = {ready_selector};
  const urlPatterns = {url_patterns};
  const titleContains = {title_contains};
  const readyElement = readySelector ? document.querySelector(readySelector) : null;
  const href = String(window.location.href || "");
  const title = String(document.title || "");
  return {{
    readySelectorSeen: Boolean(readyElement),
    urlMatched: urlPatterns.some((item) => href.includes(item)),
    titleMatched: titleContains.some((item) => title.includes(item)),
    href,
    title
  }};
}})()"#,
        ready_selector = json_string_literal(ready_selector),
        url_patterns = serde_json::to_string(url_patterns).unwrap_or_else(|_| "[]".to_string()),
        title_contains = serde_json::to_string(title_contains).unwrap_or_else(|_| "[]".to_string()),
    )
}

fn error_signal_probe_expression(error_signals: &RunnerFormErrorSignals) -> String {
    format!(
        r#"(() => {{
  const __PP_ERROR_SIGNAL_PROBE__ = true;
  const groups = {groups};
  const isVisible = (element) => {{
    if (!element) {{
      return false;
    }}
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }};
  for (const [signal, selectors] of Object.entries(groups)) {{
    for (const selector of selectors) {{
      const element = document.querySelector(selector);
      if (element && isVisible(element)) {{
        return {{
          matched: true,
          failureSignal: signal,
          selector
        }};
      }}
    }}
  }}
  return {{
    matched: false,
    failureSignal: null,
    selector: null
  }};
}})()"#,
        groups = json_value_literal(&json!({
            "login_error": error_signals.login_error,
            "field_error": error_signals.field_error,
            "account_locked": error_signals.account_locked,
        })),
    )
}

fn submit_no_effect_probe_expression(
    primary_form_selector: Option<&str>,
    submit_selector: &str,
) -> String {
    let form_selector = primary_form_selector
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_string());
    format!(
        r#"(() => {{
  const __PP_SUBMIT_NO_EFFECT_PROBE__ = true;
  const primaryFormSelector = {primary_form_selector};
  const submitSelector = {submit_selector};
  const isVisible = (element) => {{
    if (!element) {{
      return false;
    }}
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }};
  const formElement = primaryFormSelector
    ? document.querySelector(primaryFormSelector)
    : document.querySelector("form");
  const submitElement = document.querySelector(submitSelector);
  return {{
    stillOnForm: Boolean(formElement),
    submitVisible: isVisible(submitElement),
    submitEnabled: Boolean(submitElement && !submitElement.disabled)
  }};
}})()"#,
        primary_form_selector = form_selector,
        submit_selector = json_string_literal(submit_selector),
    )
}

fn scroll_progressive_expression(step: &Value) -> String {
    format!(
        r#"(() => {{
  const step = {step};
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  return (async () => {{
    const root = document.scrollingElement || document.documentElement || document.body;
    if (!root) {{
      return {{ outcome: "skipped", note: "no_scrolling_element" }};
    }}
    const scrollMax = Math.max(0, (root.scrollHeight || 0) - (window.innerHeight || 0));
    if (!scrollMax) {{
      return {{ outcome: "skipped", note: "not_scrollable" }};
    }}
    const segments = Math.max(1, Number(step.segments || 1));
    for (let index = 1; index <= segments; index += 1) {{
      const targetTop = Math.round(scrollMax * (index / segments));
      window.scrollTo(0, targetTop);
      await wait(120);
    }}
    return {{ outcome: "executed", note: "segments=" + segments }};
  }})();
}})()"#,
        step = step
    )
}

fn scroll_to_ratio_expression(step: &Value) -> String {
    format!(
        r#"(() => {{
  const step = {step};
  return (async () => {{
    const root = document.scrollingElement || document.documentElement || document.body;
    if (!root) {{
      return {{ outcome: "skipped", note: "no_scrolling_element" }};
    }}
    const scrollMax = Math.max(0, (root.scrollHeight || 0) - (window.innerHeight || 0));
    if (!scrollMax) {{
      return {{ outcome: "skipped", note: "not_scrollable" }};
    }}
    const rawRatio = Number(step.ratio || 0);
    const ratio = Math.max(0, Math.min(100, rawRatio));
    const targetTop = Math.round(scrollMax * (ratio / 100));
    window.scrollTo(0, targetTop);
    return {{ outcome: "executed", note: "ratio=" + ratio }};
  }})();
}})()"#,
        step = step
    )
}

async fn wait_for_behavior_readiness(
    client: &mut CdpClient,
    session_id: &str,
    action: &str,
) -> Result<BehaviorStepExecutionRecord, RunnerFailure> {
    let started = Instant::now();
    let timeout_at = Instant::now() + Duration::from_secs(4);
    loop {
        match client.read_readiness(session_id).await {
            Ok(snapshot) if snapshot_is_readable(action, &snapshot) => {
                return Ok(BehaviorStepExecutionRecord {
                    primitive: "wait_for_readiness".to_string(),
                    outcome: "executed".to_string(),
                    added_latency_ms: i64::try_from(started.elapsed().as_millis())
                        .unwrap_or(i64::MAX),
                    note: Some("page became readable".to_string()),
                });
            }
            Ok(_) => {}
            Err(err) if is_retryable_snapshot_error(&err) => {}
            Err(err) => return Err(err),
        }
        if Instant::now() >= timeout_at {
            return Ok(BehaviorStepExecutionRecord {
                primitive: "wait_for_readiness".to_string(),
                outcome: "skipped".to_string(),
                added_latency_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                note: Some("readiness_timeout".to_string()),
            });
        }
        sleep(Duration::from_millis(150)).await;
    }
}

async fn wait_for_content_stable(
    client: &mut CdpClient,
    session_id: &str,
    stable_window_ms: i64,
) -> Result<BehaviorStepExecutionRecord, RunnerFailure> {
    let started = Instant::now();
    let stable_window = stable_window_ms.max(150);
    let timeout_at = Instant::now()
        + Duration::from_millis(u64::try_from((stable_window * 3).max(300)).unwrap_or(300));
    let mut last_metrics: Option<Value> = None;
    let mut stable_since: Option<Instant> = None;

    loop {
        let metrics = client
            .evaluate_json(session_id, content_metrics_expression())
            .await?;
        if last_metrics.as_ref() == Some(&metrics) {
            let since = stable_since.get_or_insert_with(Instant::now);
            if since.elapsed().as_millis() >= u128::try_from(stable_window).unwrap_or(0) {
                return Ok(BehaviorStepExecutionRecord {
                    primitive: "wait_for_content_stable".to_string(),
                    outcome: "executed".to_string(),
                    added_latency_ms: i64::try_from(started.elapsed().as_millis())
                        .unwrap_or(i64::MAX),
                    note: Some("content_stable".to_string()),
                });
            }
        } else {
            last_metrics = Some(metrics);
            stable_since = None;
        }

        if Instant::now() >= timeout_at {
            return Ok(BehaviorStepExecutionRecord {
                primitive: "wait_for_content_stable".to_string(),
                outcome: "skipped".to_string(),
                added_latency_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                note: Some("stability_timeout".to_string()),
            });
        }

        sleep(Duration::from_millis(150)).await;
    }
}

async fn execute_active_behavior_plan(
    client: &mut CdpClient,
    session_id: &str,
    task: &RunnerTask,
    action: &str,
) -> Result<Option<BehaviorExecutionRuntime>, RunnerFailure> {
    if behavior_mode_from_task(task) != Some("active") {
        return Ok(None);
    }
    let Some(plan) = task.behavior_plan.as_ref() else {
        return Ok(None);
    };
    let Some(steps) = plan.steps_json.as_array() else {
        return Ok(None);
    };

    let budget = behavior_budget_from_plan(Some(plan));
    let mut runtime_explain = BehaviorRuntimeExplain {
        requested_behavior_profile_id: task
            .execution_intent
            .as_ref()
            .and_then(|intent| intent.behavior_profile_id.clone()),
        resolved_behavior_profile_id: task
            .behavior_profile
            .as_ref()
            .map(|profile| profile.id.clone()),
        resolved_version: task
            .behavior_profile
            .as_ref()
            .map(|profile| profile.version),
        resolution_source: "runner_active".to_string(),
        page_archetype: plan.page_archetype.clone(),
        capability_status: "active_executed".to_string(),
        applied_primitives: Vec::new(),
        ignored_primitives: Vec::new(),
        skipped_steps: Vec::new(),
        seed: Some(plan.seed.clone()),
        budget: budget.clone(),
        total_added_latency_ms: 0,
        warnings: Vec::new(),
    };
    let mut trace_summary = BehaviorTraceSummary {
        planned_steps: i64::try_from(steps.len()).unwrap_or(0),
        executed_steps: 0,
        failed_steps: 0,
        aborted: false,
        abort_reason: None,
        session_persisted: false,
        raw_trace_persisted: false,
        total_added_latency_ms: 0,
    };
    let mut trace_lines = Vec::new();

    for (index, step) in steps.iter().enumerate() {
        let primitive = step
            .get("primitive")
            .and_then(Value::as_str)
            .unwrap_or("unknown");

        let record = if primitive == "soft_abort_if_budget_exceeded" {
            let budget_ms = behavior_step_i64(step, "budget_ms")
                .or_else(|| budget.as_ref().map(|item| item.max_added_latency_ms))
                .unwrap_or(0);
            if budget_ms > 0 && trace_summary.total_added_latency_ms > budget_ms {
                trace_summary.aborted = true;
                trace_summary.abort_reason = Some("budget_exceeded".to_string());
                BehaviorStepExecutionRecord {
                    primitive: primitive.to_string(),
                    outcome: "aborted".to_string(),
                    added_latency_ms: 0,
                    note: Some(format!(
                        "added_latency_ms={} exceeds budget_ms={budget_ms}",
                        trace_summary.total_added_latency_ms
                    )),
                }
            } else {
                BehaviorStepExecutionRecord {
                    primitive: primitive.to_string(),
                    outcome: "executed".to_string(),
                    added_latency_ms: 0,
                    note: Some("budget_clear".to_string()),
                }
            }
        } else if !supported_active_behavior_primitive(primitive) {
            runtime_explain
                .ignored_primitives
                .push(primitive.to_string());
            runtime_explain.warnings.push(format!(
                "active primitive not implemented in lightpanda runner yet: {primitive}"
            ));
            BehaviorStepExecutionRecord {
                primitive: primitive.to_string(),
                outcome: "skipped".to_string(),
                added_latency_ms: 0,
                note: Some("unsupported_in_lightpanda_v1".to_string()),
            }
        } else {
            let started = Instant::now();
            let execution = match primitive {
                "idle" | "pause_on_content" => {
                    let duration_ms = behavior_step_i64(step, "duration_ms").unwrap_or(250).max(0);
                    sleep(Duration::from_millis(
                        u64::try_from(duration_ms).unwrap_or(0),
                    ))
                    .await;
                    Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "executed".to_string(),
                        added_latency_ms: i64::try_from(started.elapsed().as_millis())
                            .unwrap_or(i64::MAX),
                        note: Some(format!("duration_ms={duration_ms}")),
                    })
                }
                "wait_for_readiness" => {
                    wait_for_behavior_readiness(client, session_id, action).await
                }
                "wait_for_content_stable" => {
                    let stable_window_ms =
                        behavior_step_i64(step, "stable_window_ms").unwrap_or(600);
                    wait_for_content_stable(client, session_id, stable_window_ms).await
                }
                "scroll_progressive" => {
                    let result = client
                        .evaluate_json(session_id, &scroll_progressive_expression(step))
                        .await?;
                    Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: result
                            .get("outcome")
                            .and_then(Value::as_str)
                            .unwrap_or("executed")
                            .to_string(),
                        added_latency_ms: i64::try_from(started.elapsed().as_millis())
                            .unwrap_or(i64::MAX),
                        note: result
                            .get("note")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    })
                }
                "scroll_to_ratio" => {
                    let result = client
                        .evaluate_json(session_id, &scroll_to_ratio_expression(step))
                        .await?;
                    Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: result
                            .get("outcome")
                            .and_then(Value::as_str)
                            .unwrap_or("executed")
                            .to_string(),
                        added_latency_ms: i64::try_from(started.elapsed().as_millis())
                            .unwrap_or(i64::MAX),
                        note: result
                            .get("note")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    })
                }
                "focus_element" => match behavior_step_string(step, "selector") {
                    Some(selector) => {
                        let result = client
                            .evaluate_json(session_id, &focus_element_expression(&selector))
                            .await?;
                        Ok(BehaviorStepExecutionRecord {
                            primitive: primitive.to_string(),
                            outcome: result
                                .get("outcome")
                                .and_then(Value::as_str)
                                .unwrap_or("executed")
                                .to_string(),
                            added_latency_ms: i64::try_from(started.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            note: result
                                .get("note")
                                .and_then(Value::as_str)
                                .map(|note| format!("{note}; selector={selector}")),
                        })
                    }
                    None => Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "skipped".to_string(),
                        added_latency_ms: 0,
                        note: Some("missing_selector".to_string()),
                    }),
                },
                "blur_element" => match behavior_step_string(step, "selector") {
                    Some(selector) => {
                        let result = client
                            .evaluate_json(session_id, &blur_element_expression(&selector))
                            .await?;
                        Ok(BehaviorStepExecutionRecord {
                            primitive: primitive.to_string(),
                            outcome: result
                                .get("outcome")
                                .and_then(Value::as_str)
                                .unwrap_or("executed")
                                .to_string(),
                            added_latency_ms: i64::try_from(started.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            note: result
                                .get("note")
                                .and_then(Value::as_str)
                                .map(|note| format!("{note}; selector={selector}")),
                        })
                    }
                    None => Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "skipped".to_string(),
                        added_latency_ms: 0,
                        note: Some("missing_selector".to_string()),
                    }),
                },
                "hover_candidate" => {
                    let selector = behavior_step_string(step, "selector");
                    let result = client
                        .evaluate_json(session_id, &hover_candidate_expression(selector.as_deref()))
                        .await?;
                    Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: result
                            .get("outcome")
                            .and_then(Value::as_str)
                            .unwrap_or("executed")
                            .to_string(),
                        added_latency_ms: i64::try_from(started.elapsed().as_millis())
                            .unwrap_or(i64::MAX),
                        note: result
                            .get("note")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    })
                }
                "type_with_rhythm" => match behavior_step_string(step, "selector") {
                    Some(selector) => {
                        let text = behavior_step_string(step, "text").unwrap_or_default();
                        let role = behavior_step_string(step, "role")
                            .unwrap_or_else(|| "custom".to_string());
                        let result = client
                            .evaluate_json(
                                session_id,
                                &type_with_rhythm_expression(
                                    &selector,
                                    &text,
                                    i64::try_from(index).unwrap_or_default()
                                        + form_seed_value(task),
                                    &role,
                                ),
                            )
                            .await?;
                        Ok(BehaviorStepExecutionRecord {
                            primitive: primitive.to_string(),
                            outcome: result
                                .get("outcome")
                                .and_then(Value::as_str)
                                .unwrap_or("executed")
                                .to_string(),
                            added_latency_ms: i64::try_from(started.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            note: result
                                .get("note")
                                .and_then(Value::as_str)
                                .map(|note| format!("{note}; selector={selector}")),
                        })
                    }
                    None => Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "skipped".to_string(),
                        added_latency_ms: 0,
                        note: Some("missing_selector".to_string()),
                    }),
                },
                "clear_with_corrections" => match behavior_step_string(step, "selector") {
                    Some(selector) => {
                        let result = client
                            .evaluate_json(
                                session_id,
                                &clear_with_corrections_expression(
                                    &selector,
                                    i64::try_from(index).unwrap_or_default()
                                        + form_seed_value(task),
                                ),
                            )
                            .await?;
                        Ok(BehaviorStepExecutionRecord {
                            primitive: primitive.to_string(),
                            outcome: result
                                .get("outcome")
                                .and_then(Value::as_str)
                                .unwrap_or("executed")
                                .to_string(),
                            added_latency_ms: i64::try_from(started.elapsed().as_millis())
                                .unwrap_or(i64::MAX),
                            note: result
                                .get("note")
                                .and_then(Value::as_str)
                                .map(|note| format!("{note}; selector={selector}")),
                        })
                    }
                    None => Ok(BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "skipped".to_string(),
                        added_latency_ms: 0,
                        note: Some("missing_selector".to_string()),
                    }),
                },
                "persist_session_state" => Ok(BehaviorStepExecutionRecord {
                    primitive: primitive.to_string(),
                    outcome: "executed".to_string(),
                    added_latency_ms: 0,
                    note: Some("session persistence delegated to outer snapshot flow".to_string()),
                }),
                _ => Ok(BehaviorStepExecutionRecord {
                    primitive: primitive.to_string(),
                    outcome: "skipped".to_string(),
                    added_latency_ms: 0,
                    note: Some("unsupported_in_lightpanda_v1".to_string()),
                }),
            };

            match execution {
                Ok(record) => record,
                Err(err)
                    if matches!(err.error_kind, "cdp_evaluate_failed" | "cdp_protocol_error") =>
                {
                    trace_summary.failed_steps += 1;
                    runtime_explain.warnings.push(format!(
                        "behavior primitive failed and was downgraded to warning: {primitive}: {}",
                        err.message
                    ));
                    BehaviorStepExecutionRecord {
                        primitive: primitive.to_string(),
                        outcome: "failed".to_string(),
                        added_latency_ms: i64::try_from(started.elapsed().as_millis())
                            .unwrap_or(i64::MAX),
                        note: Some(err.message),
                    }
                }
                Err(err) => return Err(err),
            }
        };

        trace_summary.total_added_latency_ms += record.added_latency_ms.max(0);
        runtime_explain.total_added_latency_ms = trace_summary.total_added_latency_ms;

        match record.outcome.as_str() {
            "executed" => {
                trace_summary.executed_steps += 1;
                if record.primitive == "persist_session_state" {
                    trace_summary.session_persisted = true;
                }
                if !runtime_explain
                    .applied_primitives
                    .iter()
                    .any(|item| item == &record.primitive)
                {
                    runtime_explain
                        .applied_primitives
                        .push(record.primitive.clone());
                }
            }
            "aborted" => {
                trace_summary.aborted = true;
                if trace_summary.abort_reason.is_none() {
                    trace_summary.abort_reason = Some("budget_exceeded".to_string());
                }
            }
            "skipped" | "failed" => {
                let reason = record
                    .note
                    .clone()
                    .unwrap_or_else(|| record.outcome.clone());
                runtime_explain
                    .skipped_steps
                    .push(format!("{}:{reason}", record.primitive));
            }
            _ => {}
        }

        trace_lines.push(
            json!({
                "step_index": index,
                "primitive": record.primitive,
                "outcome": record.outcome,
                "added_latency_ms": record.added_latency_ms,
                "note": record.note,
            })
            .to_string(),
        );

        if trace_summary.aborted {
            break;
        }
    }

    if trace_summary.aborted {
        runtime_explain.capability_status = "active_aborted".to_string();
    } else if !runtime_explain.ignored_primitives.is_empty()
        || !runtime_explain.skipped_steps.is_empty()
    {
        runtime_explain.capability_status = "active_partial".to_string();
    }

    Ok(Some(BehaviorExecutionRuntime {
        runtime_explain,
        trace_summary,
        trace_lines,
    }))
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn merge_stderr_preview(
    stderr_preview: Option<String>,
    stderr_hint: Option<String>,
) -> Option<String> {
    match (stderr_preview, stderr_hint) {
        (Some(stderr_preview), Some(stderr_hint)) => {
            let stderr_hint = truncate_output(&stderr_hint, STDERR_PREVIEW_LIMIT);
            if stderr_hint.is_empty() || stderr_preview.contains(&stderr_hint) {
                Some(stderr_preview)
            } else {
                Some(format!("{stderr_preview}\n{stderr_hint}"))
            }
        }
        (Some(stderr_preview), None) => Some(stderr_preview),
        (None, Some(stderr_hint)) => preview_if_non_empty(stderr_hint, STDERR_PREVIEW_LIMIT),
        (None, None) => None,
    }
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

fn is_retryable_snapshot_error(error: &RunnerFailure) -> bool {
    if error.error_kind != "cdp_evaluate_failed" && error.error_kind != "cdp_protocol_error" {
        return false;
    }

    let message = error.message.to_ascii_lowercase();
    message.contains("execution context")
        || message.contains("context with specified id")
        || message.contains("no frame with given id")
        || message.contains("cannot find context")
}

fn allocate_loopback_port() -> io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn form_poll_timeout(task: &RunnerTask) -> Duration {
    let timeout_ms = task
        .timeout_seconds
        .unwrap_or(5)
        .saturating_mul(1_000)
        .clamp(500, 900);
    Duration::from_millis(u64::try_from(timeout_ms).unwrap_or(800))
}

fn is_form_transient_error(error: &RunnerFailure) -> bool {
    if error.error_kind != "cdp_evaluate_failed" && error.error_kind != "cdp_protocol_error" {
        return false;
    }
    let message = error.message.to_ascii_lowercase();
    message.contains("execution context")
        || message.contains("context with specified id")
        || message.contains("no frame with given id")
        || message.contains("cannot find context")
        || message.contains("detached")
        || message.contains("stale")
        || message.contains("context lost")
}

fn push_form_trace_line(
    trace_lines: &mut Vec<String>,
    stage: &str,
    event: &str,
    primitive: &str,
    outcome: &str,
    selector: Option<&str>,
    field: Option<&RunnerFormFieldPlan>,
    added_latency_ms: i64,
    note: Option<String>,
) {
    trace_lines.push(
        json!({
            "stage": stage,
            "event": event,
            "primitive": primitive,
            "outcome": outcome,
            "selector": selector,
            "field_key": field.map(|item| item.key.clone()),
            "field_role": field.map(|item| item.role.clone()),
            "value_source": field.map(|item| item.value_source.clone()),
            "resolved": field.map(|item| item.resolved),
            "added_latency_ms": added_latency_ms,
            "note": note,
        })
        .to_string(),
    );
}

fn primitive_outcome(value: &Value) -> &str {
    value
        .get("outcome")
        .and_then(Value::as_str)
        .unwrap_or("executed")
}

fn primitive_note(value: &Value) -> Option<String> {
    value
        .get("note")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn selector_not_found(value: &Value) -> bool {
    primitive_outcome(value) == "skipped"
        && value
            .get("note")
            .and_then(Value::as_str)
            .is_some_and(|note| note.contains("selector_not_found"))
}

async fn evaluate_form_primitive(
    client: &mut CdpClient,
    session_id: &str,
    expression: String,
    stage: &str,
    event: &str,
    primitive: &str,
    selector: Option<&str>,
    field: Option<&RunnerFormFieldPlan>,
    trace_lines: &mut Vec<String>,
) -> Result<Value, FormActionExecutionError> {
    let started = Instant::now();
    match client.evaluate_json(session_id, &expression).await {
        Ok(value) => {
            push_form_trace_line(
                trace_lines,
                stage,
                event,
                primitive,
                primitive_outcome(&value),
                selector,
                field,
                i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                primitive_note(&value),
            );
            Ok(value)
        }
        Err(error) if is_form_transient_error(&error) => {
            let note = error.message.clone();
            push_form_trace_line(
                trace_lines,
                stage,
                event,
                primitive,
                "failed",
                selector,
                field,
                i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                Some(note.clone()),
            );
            Err(FormActionExecutionError::Retryable {
                signal: "transient_dom_error".to_string(),
                message: note,
            })
        }
        Err(error) => Err(FormActionExecutionError::Fatal(error)),
    }
}

async fn read_field_state(
    client: &mut CdpClient,
    session_id: &str,
    selector: &str,
) -> Result<Value, FormActionExecutionError> {
    match client
        .evaluate_json(session_id, &field_state_expression(selector))
        .await
    {
        Ok(value) => Ok(value),
        Err(error) if is_form_transient_error(&error) => Err(FormActionExecutionError::Retryable {
            signal: "transient_dom_error".to_string(),
            message: error.message,
        }),
        Err(error) => Err(FormActionExecutionError::Fatal(error)),
    }
}

fn failed_form_runtime(
    task: &RunnerTask,
    plan: &RunnerFormActionPlan,
    retry_count: i64,
    signal: String,
    trace_lines: Vec<String>,
    message: String,
) -> FormActionRuntime {
    build_form_action_runtime(
        task,
        plan,
        FORM_ACTION_STATUS_FAILED,
        retry_count,
        Some(signal.clone()),
        trace_lines,
        false,
        false,
        false,
        Some(format!(
            "lightpanda active form action failed: {signal}; {message}"
        )),
    )
}

async fn probe_submit_outcome(
    client: &mut CdpClient,
    session_id: &str,
    plan: &RunnerFormActionPlan,
    trace_lines: &mut Vec<String>,
    poll_timeout: Duration,
) -> Result<bool, FormActionExecutionError> {
    let Some(success) = plan.success.as_ref() else {
        return Err(FormActionExecutionError::Terminal {
            signal: "missing_required_field".to_string(),
            message: "success.ready_selector is missing in active form action".to_string(),
        });
    };
    let poll_started = Instant::now();
    loop {
        let success_probe = match client
            .evaluate_json(
                session_id,
                &success_probe_expression(
                    &success.ready_selector,
                    &success.url_patterns,
                    &success.title_contains,
                ),
            )
            .await
        {
            Ok(value) => value,
            Err(error) if is_form_transient_error(&error) => {
                return Err(FormActionExecutionError::Retryable {
                    signal: "transient_dom_error".to_string(),
                    message: error.message,
                })
            }
            Err(error) => return Err(FormActionExecutionError::Fatal(error)),
        };
        if success_probe
            .get("readySelectorSeen")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            push_form_trace_line(
                trace_lines,
                "post_submit",
                "success_probe",
                "success_probe",
                "executed",
                Some(success.ready_selector.as_str()),
                None,
                i64::try_from(poll_started.elapsed().as_millis()).unwrap_or(i64::MAX),
                Some(format!(
                    "ready_selector_seen=true url_matched={} title_matched={}",
                    success_probe
                        .get("urlMatched")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    success_probe
                        .get("titleMatched")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                )),
            );
            return Ok(true);
        }

        if let Some(error_signals) = plan.error_signals.as_ref() {
            let error_probe = match client
                .evaluate_json(session_id, &error_signal_probe_expression(error_signals))
                .await
            {
                Ok(value) => value,
                Err(error) if is_form_transient_error(&error) => {
                    return Err(FormActionExecutionError::Retryable {
                        signal: "transient_dom_error".to_string(),
                        message: error.message,
                    })
                }
                Err(error) => return Err(FormActionExecutionError::Fatal(error)),
            };
            if error_probe
                .get("matched")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let failure_signal = error_probe
                    .get("failureSignal")
                    .and_then(Value::as_str)
                    .unwrap_or("login_error");
                let selector = error_probe
                    .get("selector")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                push_form_trace_line(
                    trace_lines,
                    "post_submit",
                    "error_probe",
                    "error_signal_probe",
                    "failed",
                    selector.as_deref(),
                    None,
                    i64::try_from(poll_started.elapsed().as_millis()).unwrap_or(i64::MAX),
                    Some(format!("failure_signal={failure_signal}")),
                );
                return Err(FormActionExecutionError::Terminal {
                    signal: failure_signal.to_string(),
                    message: format!("form error signal matched: {failure_signal}"),
                });
            }
        }

        if poll_started.elapsed() >= poll_timeout {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }

    let submit_selector = plan
        .submit
        .as_ref()
        .map(|item| item.selector.as_str())
        .unwrap_or("");
    let no_effect_probe = match client
        .evaluate_json(
            session_id,
            &submit_no_effect_probe_expression(plan.form_selector.as_deref(), submit_selector),
        )
        .await
    {
        Ok(value) => value,
        Err(error) if is_form_transient_error(&error) => {
            return Err(FormActionExecutionError::Retryable {
                signal: "transient_dom_error".to_string(),
                message: error.message,
            })
        }
        Err(error) => return Err(FormActionExecutionError::Fatal(error)),
    };
    if no_effect_probe
        .get("stillOnForm")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && no_effect_probe
            .get("submitVisible")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && no_effect_probe
            .get("submitEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        push_form_trace_line(
            trace_lines,
            "post_submit",
            "submit_no_effect_probe",
            "submit_no_effect_probe",
            "failed",
            Some(submit_selector),
            None,
            i64::try_from(poll_started.elapsed().as_millis()).unwrap_or(i64::MAX),
            Some("submit click had no visible effect".to_string()),
        );
        return Err(FormActionExecutionError::Retryable {
            signal: "submit_no_effect".to_string(),
            message: "submit click produced no success or error signal".to_string(),
        });
    }

    push_form_trace_line(
        trace_lines,
        "post_submit",
        "success_probe",
        "success_probe",
        "failed",
        Some(success.ready_selector.as_str()),
        None,
        i64::try_from(poll_started.elapsed().as_millis()).unwrap_or(i64::MAX),
        Some("timeout_waiting_success".to_string()),
    );
    Err(FormActionExecutionError::Terminal {
        signal: "timeout_waiting_success".to_string(),
        message: "timed out waiting for success.ready_selector".to_string(),
    })
}

async fn execute_form_field(
    client: &mut CdpClient,
    session_id: &str,
    field: &RunnerFormFieldPlan,
    seed: i64,
    trace_lines: &mut Vec<String>,
) -> Result<(), FormActionExecutionError> {
    let Some(selector) = field.selector.as_deref() else {
        let note = format!("selector missing for field '{}'", field.key);
        push_form_trace_line(
            trace_lines,
            "pre_submit",
            "field_validate",
            "resolve_field",
            if field.required { "failed" } else { "skipped" },
            None,
            Some(field),
            0,
            Some(note.clone()),
        );
        if field.required {
            return Err(FormActionExecutionError::Terminal {
                signal: "missing_required_field".to_string(),
                message: note,
            });
        }
        return Ok(());
    };

    let state = read_field_state(client, session_id, selector).await?;
    if !state
        .get("present")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let note = format!("required field '{}' not present", field.key);
        push_form_trace_line(
            trace_lines,
            "pre_submit",
            "field_validate",
            "resolve_field",
            if field.required { "failed" } else { "skipped" },
            Some(selector),
            Some(field),
            0,
            Some(note.clone()),
        );
        if field.required {
            return Err(FormActionExecutionError::Terminal {
                signal: "missing_required_field".to_string(),
                message: note,
            });
        }
        return Ok(());
    }

    if field.role == "remember_me" {
        let desired_checked = field
            .resolved_value
            .as_ref()
            .and_then(bool_from_value)
            .ok_or_else(|| FormActionExecutionError::Terminal {
                signal: "missing_required_field".to_string(),
                message: format!("boolean value missing for field '{}'", field.key),
            })?;
        let current_checked = state
            .get("checked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if current_checked == desired_checked {
            push_form_trace_line(
                trace_lines,
                "pre_submit",
                "toggle_checkbox",
                "checkbox_toggle",
                "skipped",
                Some(selector),
                Some(field),
                0,
                Some("already_aligned".to_string()),
            );
            return Ok(());
        }
        let toggle = evaluate_form_primitive(
            client,
            session_id,
            checkbox_toggle_expression(selector, desired_checked),
            "pre_submit",
            "toggle_checkbox",
            "checkbox_toggle",
            Some(selector),
            Some(field),
            trace_lines,
        )
        .await?;
        if selector_not_found(&toggle) {
            return Err(FormActionExecutionError::Retryable {
                signal: "transient_dom_error".to_string(),
                message: format!("checkbox selector '{selector}' disappeared during toggle"),
            });
        }
        return Ok(());
    }

    let desired_value = field
        .resolved_value
        .as_ref()
        .and_then(string_from_value)
        .ok_or_else(|| FormActionExecutionError::Terminal {
            signal: "missing_required_field".to_string(),
            message: format!("resolved value missing for field '{}'", field.key),
        })?;
    let focus = evaluate_form_primitive(
        client,
        session_id,
        focus_element_expression(selector),
        "pre_submit",
        "focus",
        "focus_element",
        Some(selector),
        Some(field),
        trace_lines,
    )
    .await?;
    if selector_not_found(&focus) {
        return Err(FormActionExecutionError::Retryable {
            signal: "transient_dom_error".to_string(),
            message: format!("field selector '{selector}' disappeared during focus"),
        });
    }

    let current_value = state
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if !current_value.is_empty() && current_value != desired_value {
        let clear = evaluate_form_primitive(
            client,
            session_id,
            clear_with_corrections_expression(selector, seed),
            "pre_submit",
            "clear",
            "clear_with_corrections",
            Some(selector),
            Some(field),
            trace_lines,
        )
        .await?;
        if selector_not_found(&clear) {
            return Err(FormActionExecutionError::Retryable {
                signal: "transient_dom_error".to_string(),
                message: format!("field selector '{selector}' disappeared during clear"),
            });
        }
    } else {
        push_form_trace_line(
            trace_lines,
            "pre_submit",
            "clear",
            "clear_with_corrections",
            "skipped",
            Some(selector),
            Some(field),
            0,
            Some(if current_value.is_empty() {
                "already_empty".to_string()
            } else {
                "already_matching".to_string()
            }),
        );
    }

    if current_value != desired_value {
        let typed = evaluate_form_primitive(
            client,
            session_id,
            type_with_rhythm_expression(selector, &desired_value, seed, &field.role),
            "pre_submit",
            "type",
            "type_with_rhythm",
            Some(selector),
            Some(field),
            trace_lines,
        )
        .await?;
        if selector_not_found(&typed) {
            return Err(FormActionExecutionError::Retryable {
                signal: "transient_dom_error".to_string(),
                message: format!("field selector '{selector}' disappeared during typing"),
            });
        }
    } else {
        push_form_trace_line(
            trace_lines,
            "pre_submit",
            "type",
            "type_with_rhythm",
            "skipped",
            Some(selector),
            Some(field),
            0,
            Some("already_matching".to_string()),
        );
    }

    let blur = evaluate_form_primitive(
        client,
        session_id,
        blur_element_expression(selector),
        "pre_submit",
        "blur",
        "blur_element",
        Some(selector),
        Some(field),
        trace_lines,
    )
    .await?;
    if selector_not_found(&blur) {
        return Err(FormActionExecutionError::Retryable {
            signal: "transient_dom_error".to_string(),
            message: format!("field selector '{selector}' disappeared during blur"),
        });
    }
    Ok(())
}

async fn execute_post_login_first_screen(
    client: &mut CdpClient,
    session_id: &str,
    trace_lines: &mut Vec<String>,
) -> (bool, bool) {
    let readiness = wait_for_behavior_readiness(client, session_id, "open_page").await;
    match readiness {
        Ok(record) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "readiness",
            &record.primitive,
            &record.outcome,
            None,
            None,
            record.added_latency_ms,
            record.note,
        ),
        Err(error) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "readiness",
            "wait_for_readiness",
            "failed",
            None,
            None,
            0,
            Some(error.message),
        ),
    }

    let settle_started = Instant::now();
    sleep(Duration::from_millis(300)).await;
    push_form_trace_line(
        trace_lines,
        "post_submit",
        "settle",
        "idle",
        "executed",
        None,
        None,
        i64::try_from(settle_started.elapsed().as_millis()).unwrap_or(i64::MAX),
        Some("duration_ms=300".to_string()),
    );

    match client
        .evaluate_json(
            session_id,
            &scroll_progressive_expression(&json!({ "segments": 1 })),
        )
        .await
    {
        Ok(value) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "scroll",
            "scroll_progressive",
            primitive_outcome(&value),
            None,
            None,
            0,
            primitive_note(&value),
        ),
        Err(error) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "scroll",
            "scroll_progressive",
            "failed",
            None,
            None,
            0,
            Some(error.message),
        ),
    }

    match client
        .evaluate_json(session_id, &hover_candidate_expression(None))
        .await
    {
        Ok(value) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "hover",
            "hover_candidate",
            primitive_outcome(&value),
            None,
            None,
            0,
            primitive_note(&value),
        ),
        Err(error) => push_form_trace_line(
            trace_lines,
            "post_submit",
            "hover",
            "hover_candidate",
            "failed",
            None,
            None,
            0,
            Some(error.message),
        ),
    }

    push_form_trace_line(
        trace_lines,
        "persist",
        "persist_session",
        "persist_session_state",
        "executed",
        None,
        None,
        0,
        Some("outer_persistence_chain".to_string()),
    );
    (true, true)
}

fn build_form_behavior_runtime(
    task: &RunnerTask,
    plan: &RunnerFormActionPlan,
    status: &str,
    failure_signal: Option<&str>,
    trace_lines: &[String],
    session_persisted: bool,
) -> (BehaviorRuntimeExplain, BehaviorTraceSummary) {
    let mut applied_primitives = Vec::new();
    let mut skipped_steps = Vec::new();
    let mut executed_steps = 0_i64;
    let mut failed_steps = 0_i64;
    let mut total_added_latency_ms = 0_i64;

    for line in trace_lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let primitive = value
            .get("primitive")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let outcome = value
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("executed");
        let added_latency_ms = value
            .get("added_latency_ms")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        total_added_latency_ms += added_latency_ms.max(0);
        match outcome {
            "executed" => {
                executed_steps += 1;
                if !applied_primitives.iter().any(|item| item == &primitive) {
                    applied_primitives.push(primitive);
                }
            }
            "failed" => {
                failed_steps += 1;
                skipped_steps.push(format!(
                    "{}:{}",
                    primitive,
                    value
                        .get("note")
                        .and_then(Value::as_str)
                        .unwrap_or("failed")
                ));
            }
            "skipped" | "scheduled" => skipped_steps.push(format!(
                "{}:{}",
                primitive,
                value.get("note").and_then(Value::as_str).unwrap_or(outcome)
            )),
            _ => {}
        }
    }

    let warnings = failure_signal
        .map(|signal| vec![format!("form action failure signal={signal}")])
        .unwrap_or_default();
    let runtime_explain = BehaviorRuntimeExplain {
        requested_behavior_profile_id: task
            .execution_intent
            .as_ref()
            .and_then(|intent| intent.behavior_profile_id.clone()),
        resolved_behavior_profile_id: task
            .behavior_profile
            .as_ref()
            .map(|profile| profile.id.clone()),
        resolved_version: task
            .behavior_profile
            .as_ref()
            .map(|profile| profile.version),
        resolution_source: "runner_form_active".to_string(),
        page_archetype: task
            .behavior_plan
            .as_ref()
            .and_then(|plan| plan.page_archetype.clone())
            .or_else(|| Some(plan.mode.clone())),
        capability_status: if status == FORM_ACTION_STATUS_SUCCEEDED {
            "active_form_executed".to_string()
        } else {
            "active_form_failed".to_string()
        },
        applied_primitives,
        ignored_primitives: Vec::new(),
        skipped_steps,
        seed: task.behavior_plan.as_ref().map(|plan| plan.seed.clone()),
        budget: behavior_budget_from_plan(task.behavior_plan.as_ref()),
        total_added_latency_ms,
        warnings,
    };
    let trace_summary = BehaviorTraceSummary {
        planned_steps: i64::try_from(trace_lines.len()).unwrap_or(0),
        executed_steps,
        failed_steps,
        aborted: false,
        abort_reason: None,
        session_persisted,
        raw_trace_persisted: false,
        total_added_latency_ms,
    };
    (runtime_explain, trace_summary)
}

fn build_form_action_runtime(
    task: &RunnerTask,
    plan: &RunnerFormActionPlan,
    status: &str,
    retry_count: i64,
    failure_signal: Option<String>,
    trace_lines: Vec<String>,
    success_ready_selector_seen: bool,
    post_login_actions_executed: bool,
    session_persisted: bool,
    error_message: Option<String>,
) -> FormActionRuntime {
    let (behavior_runtime_explain, behavior_trace_summary) = build_form_behavior_runtime(
        task,
        plan,
        status,
        failure_signal.as_deref(),
        &trace_lines,
        session_persisted,
    );
    let mut summary_json = build_form_action_summary_json(
        plan,
        status,
        retry_count,
        plan.blocked_reason.as_deref(),
        failure_signal.as_deref(),
    );
    set_form_action_summary_flags(
        &mut summary_json,
        success_ready_selector_seen,
        post_login_actions_executed,
        session_persisted,
    );
    FormActionRuntime {
        status: status.to_string(),
        mode: plan.mode.clone(),
        retry_count,
        failure_signal,
        summary_json,
        trace_lines,
        session_persisted,
        post_login_actions_executed,
        success_ready_selector_seen,
        behavior_runtime_explain,
        behavior_trace_summary,
        error_message,
    }
}

async fn execute_active_form_action_plan(
    client: &mut CdpClient,
    session_id: &str,
    task: &RunnerTask,
    action: &str,
) -> Result<FormActionRuntime, RunnerFailure> {
    let plan = task
        .form_action_plan
        .as_ref()
        .expect("form action plan should exist when active flow executes");
    let retry_limit = plan.retry_limit.clamp(0, 1);
    let poll_timeout = form_poll_timeout(task);
    let mut trace_lines = Vec::new();
    let mut retry_count = 0_i64;
    let seed = form_seed_value(task);

    'attempts: for attempt in 0..=retry_limit {
        if attempt > 0 {
            push_form_trace_line(
                &mut trace_lines,
                "pre_submit",
                "retry",
                "retry_path",
                "executed",
                None,
                None,
                0,
                Some("refocus -> clear -> retype -> resubmit".to_string()),
            );
        }

        let readiness = wait_for_behavior_readiness(client, session_id, action).await;
        match readiness {
            Ok(record) => push_form_trace_line(
                &mut trace_lines,
                "pre_submit",
                "readiness",
                &record.primitive,
                &record.outcome,
                None,
                None,
                record.added_latency_ms,
                record.note,
            ),
            Err(error) if is_form_transient_error(&error) => {
                if attempt < retry_limit {
                    retry_count += 1;
                    continue;
                }
                return Ok(failed_form_runtime(
                    task,
                    plan,
                    retry_count,
                    "transient_dom_error".to_string(),
                    trace_lines,
                    error.message,
                ));
            }
            Err(error) => return Err(error),
        }

        if let Some(form_selector) = plan.form_selector.as_deref() {
            match client
                .evaluate_json(session_id, &selector_visible_expression(form_selector))
                .await
            {
                Ok(value) => push_form_trace_line(
                    &mut trace_lines,
                    "pre_submit",
                    "locate_form",
                    "locate_form",
                    if value
                        .get("visible")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                        || value
                            .get("present")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                    {
                        "executed"
                    } else {
                        "skipped"
                    },
                    Some(form_selector),
                    None,
                    0,
                    Some(format!("selector_source={}", plan.form_selector_source)),
                ),
                Err(error) if is_form_transient_error(&error) => {
                    if attempt < retry_limit {
                        retry_count += 1;
                        continue;
                    }
                    return Ok(failed_form_runtime(
                        task,
                        plan,
                        retry_count,
                        "transient_dom_error".to_string(),
                        trace_lines,
                        error.message,
                    ));
                }
                Err(error) => return Err(error),
            }
        }

        for (field_index, field) in plan.fields.iter().enumerate() {
            let field_seed = seed + i64::try_from(field_index).unwrap_or_default() + retry_count;
            match execute_form_field(client, session_id, field, field_seed, &mut trace_lines).await
            {
                Ok(()) => {}
                Err(FormActionExecutionError::Retryable { signal, message })
                    if attempt < retry_limit =>
                {
                    retry_count += 1;
                    push_form_trace_line(
                        &mut trace_lines,
                        "pre_submit",
                        "retry_scheduled",
                        "retry_path",
                        "executed",
                        None,
                        Some(field),
                        0,
                        Some(format!("{signal}: {message}")),
                    );
                    continue 'attempts;
                }
                Err(FormActionExecutionError::Retryable { signal, message })
                | Err(FormActionExecutionError::Terminal { signal, message }) => {
                    return Ok(failed_form_runtime(
                        task,
                        plan,
                        retry_count,
                        signal,
                        trace_lines,
                        message,
                    ));
                }
                Err(FormActionExecutionError::Fatal(error)) => return Err(error),
            }
        }

        let stable = wait_for_content_stable(client, session_id, 400).await;
        match stable {
            Ok(record) => push_form_trace_line(
                &mut trace_lines,
                "pre_submit",
                "content_stable",
                &record.primitive,
                &record.outcome,
                None,
                None,
                record.added_latency_ms,
                record.note,
            ),
            Err(error) if is_form_transient_error(&error) => {
                if attempt < retry_limit {
                    retry_count += 1;
                    continue;
                }
                return Ok(failed_form_runtime(
                    task,
                    plan,
                    retry_count,
                    "transient_dom_error".to_string(),
                    trace_lines,
                    error.message,
                ));
            }
            Err(error) => return Err(error),
        }

        let submit = plan.submit.as_ref().ok_or_else(|| {
            RunnerFailure::new(
                "form_submit_missing",
                "active form action requires submit selector",
                Some("action"),
                None,
            )
        })?;
        push_form_trace_line(
            &mut trace_lines,
            "submit",
            "before_click",
            "click_selector",
            "scheduled",
            Some(submit.selector.as_str()),
            None,
            0,
            Some("submit click scheduled".to_string()),
        );
        match evaluate_form_primitive(
            client,
            session_id,
            click_selector_expression(&submit.selector),
            "submit",
            "after_click",
            "click_selector",
            Some(submit.selector.as_str()),
            None,
            &mut trace_lines,
        )
        .await
        {
            Ok(value) if selector_not_found(&value) => {
                if attempt < retry_limit {
                    retry_count += 1;
                    continue;
                }
                return Ok(failed_form_runtime(
                    task,
                    plan,
                    retry_count,
                    "transient_dom_error".to_string(),
                    trace_lines,
                    format!(
                        "submit selector '{}' disappeared during click",
                        submit.selector
                    ),
                ));
            }
            Ok(_) => {}
            Err(FormActionExecutionError::Retryable { signal, message })
                if attempt < retry_limit =>
            {
                retry_count += 1;
                push_form_trace_line(
                    &mut trace_lines,
                    "submit",
                    "retry_scheduled",
                    "retry_path",
                    "executed",
                    Some(submit.selector.as_str()),
                    None,
                    0,
                    Some(format!("{signal}: {message}")),
                );
                continue;
            }
            Err(FormActionExecutionError::Retryable { signal, message })
            | Err(FormActionExecutionError::Terminal { signal, message }) => {
                return Ok(failed_form_runtime(
                    task,
                    plan,
                    retry_count,
                    signal,
                    trace_lines,
                    message,
                ));
            }
            Err(FormActionExecutionError::Fatal(error)) => return Err(error),
        }

        let settle_started = Instant::now();
        sleep(Duration::from_millis(350)).await;
        push_form_trace_line(
            &mut trace_lines,
            "submit",
            "settle",
            "idle",
            "executed",
            None,
            None,
            i64::try_from(settle_started.elapsed().as_millis()).unwrap_or(i64::MAX),
            Some("duration_ms=350".to_string()),
        );

        match probe_submit_outcome(client, session_id, plan, &mut trace_lines, poll_timeout).await {
            Ok(success_ready_selector_seen) => {
                let (post_login_actions_executed, session_persisted) =
                    execute_post_login_first_screen(client, session_id, &mut trace_lines).await;
                return Ok(build_form_action_runtime(
                    task,
                    plan,
                    FORM_ACTION_STATUS_SUCCEEDED,
                    retry_count,
                    None,
                    trace_lines,
                    success_ready_selector_seen,
                    post_login_actions_executed,
                    session_persisted,
                    None,
                ));
            }
            Err(FormActionExecutionError::Retryable { signal, message })
                if attempt < retry_limit =>
            {
                retry_count += 1;
                push_form_trace_line(
                    &mut trace_lines,
                    "post_submit",
                    "retry_scheduled",
                    "retry_path",
                    "executed",
                    Some(submit.selector.as_str()),
                    None,
                    0,
                    Some(format!("{signal}: {message}")),
                );
                continue;
            }
            Err(FormActionExecutionError::Retryable { signal, message })
            | Err(FormActionExecutionError::Terminal { signal, message }) => {
                return Ok(failed_form_runtime(
                    task,
                    plan,
                    retry_count,
                    signal,
                    trace_lines,
                    message,
                ));
            }
            Err(FormActionExecutionError::Fatal(error)) => return Err(error),
        }
    }

    Ok(failed_form_runtime(
        task,
        plan,
        retry_count,
        "submit_no_effect".to_string(),
        trace_lines,
        "retry budget exhausted".to_string(),
    ))
}

async fn perform_browser_action(
    ws_endpoint: &str,
    task: &RunnerTask,
    action: &str,
    url: &str,
    session_cookies: Option<&[Value]>,
    session_local_storage: Option<&Value>,
    session_session_storage: Option<&Value>,
) -> Result<BrowserActionResult, RunnerFailure> {
    let mut client = CdpClient::connect(ws_endpoint).await?;
    let target_id = client.create_target().await?;
    let session_id = client.attach_to_target(&target_id).await?;
    client.enable_page_and_runtime(&session_id).await?;
    if let Some(cookies) = session_cookies.filter(|cookies| !cookies.is_empty()) {
        let _ = client.set_cookies(&session_id, cookies).await;
    }

    let navigate = client.navigate(&session_id, url).await?;
    if let Some(error_text) = navigate.error_text {
        return Err(RunnerFailure::new(
            "browser_navigation_failed",
            format!("lightpanda navigation failed: {error_text}"),
            Some("navigate"),
            Some(error_text),
        ));
    }

    loop {
        match client.read_readiness(&session_id).await {
            Ok(snapshot) if snapshot_is_readable(action, &snapshot) => {
                let _ = client
                    .set_storage(&session_id, session_local_storage, session_session_storage)
                    .await;
                if let Some(plan) = task.form_action_plan.as_ref() {
                    let latest_snapshot = if plan.execution_mode == "active"
                        && plan.blocked_reason.is_none()
                    {
                        let runtime =
                            execute_active_form_action_plan(&mut client, &session_id, task, action)
                                .await?;
                        let refreshed = match client.read_readiness(&session_id).await {
                            Ok(refreshed) => refreshed,
                            Err(error) if is_retryable_snapshot_error(&error) => snapshot.clone(),
                            Err(error) => return Err(error),
                        };
                        let mut result = match action {
                            "open_page" | "get_title" | "get_final_url" => {
                                Ok(BrowserActionResult::from_readiness(refreshed))
                            }
                            "get_html" => client
                                .read_html(&session_id)
                                .await
                                .map(BrowserActionResult::from_html),
                            "extract_text" => client
                                .read_text(&session_id)
                                .await
                                .map(BrowserActionResult::from_text),
                            _ => Ok(BrowserActionResult::from_readiness(refreshed)),
                        };
                        if let Ok(ref mut action_result) = result {
                            apply_form_action_runtime(action_result, runtime);
                            action_result.cookies = client
                                .get_cookies(&session_id, url)
                                .await
                                .unwrap_or_default();
                            let (local_storage, session_storage) = client
                                .get_storage(&session_id)
                                .await
                                .unwrap_or((None, None));
                            action_result.local_storage = local_storage;
                            action_result.session_storage = session_storage;
                        }
                        return result;
                    } else {
                        snapshot
                    };

                    let mut result = match action {
                        "open_page" | "get_title" | "get_final_url" => {
                            Ok(BrowserActionResult::from_readiness(latest_snapshot))
                        }
                        "get_html" => client
                            .read_html(&session_id)
                            .await
                            .map(BrowserActionResult::from_html),
                        "extract_text" => client
                            .read_text(&session_id)
                            .await
                            .map(BrowserActionResult::from_text),
                        _ => Ok(BrowserActionResult::from_readiness(latest_snapshot)),
                    };
                    if let Ok(ref mut action_result) = result {
                        apply_non_active_form_action_result(action_result, task);
                        action_result.cookies = client
                            .get_cookies(&session_id, url)
                            .await
                            .unwrap_or_default();
                        let (local_storage, session_storage) = client
                            .get_storage(&session_id)
                            .await
                            .unwrap_or((None, None));
                        action_result.local_storage = local_storage;
                        action_result.session_storage = session_storage;
                    }
                    return result;
                }

                let behavior_runtime =
                    execute_active_behavior_plan(&mut client, &session_id, task, action).await?;
                let mut result = match action {
                    "open_page" | "get_title" | "get_final_url" => {
                        Ok(BrowserActionResult::from_readiness(snapshot))
                    }
                    "get_html" => client
                        .read_html(&session_id)
                        .await
                        .map(BrowserActionResult::from_html),
                    "extract_text" => client
                        .read_text(&session_id)
                        .await
                        .map(BrowserActionResult::from_text),
                    _ => Ok(BrowserActionResult::from_readiness(snapshot)),
                };
                if let Ok(ref mut action_result) = result {
                    if let Some(runtime) = behavior_runtime {
                        action_result.behavior_runtime_explain = Some(runtime.runtime_explain);
                        action_result.behavior_trace_summary = Some(runtime.trace_summary);
                        action_result.behavior_trace_lines = runtime.trace_lines;
                    }
                    action_result.cookies = client
                        .get_cookies(&session_id, url)
                        .await
                        .unwrap_or_default();
                    let (local_storage, session_storage) = client
                        .get_storage(&session_id)
                        .await
                        .unwrap_or((None, None));
                    action_result.local_storage = local_storage;
                    action_result.session_storage = session_storage;
                }
                return result;
            }
            Ok(_) => {}
            Err(err) if is_retryable_snapshot_error(&err) => {}
            Err(err) => return Err(err),
        }

        sleep(Duration::from_millis(LIGHTPANDA_WAIT_POLL_MS)).await;
    }
}

async fn wait_for_ws_endpoint(child: &mut Child, port: u16) -> Result<String, RunnerFailure> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .map_err(|err| {
            RunnerFailure::new(
                "spawn_failed",
                format!("failed to build HTTP client for lightpanda launcher: {err}"),
                Some("launch"),
                Some(err.to_string()),
            )
        })?;
    let version_url = format!("http://127.0.0.1:{port}/json/version");

    loop {
        if let Some(status) = child.try_wait().map_err(|err| {
            RunnerFailure::new(
                "process_wait_failed",
                format!("failed to check lightpanda serve process status: {err}"),
                Some("launch"),
                Some(err.to_string()),
            )
        })? {
            let exit_code = status.code();
            return Err(RunnerFailure::new(
                classify_exit_code(exit_code),
                format!(
                    "lightpanda serve exited before websocket endpoint became ready (exit_code={exit_code:?})"
                ),
                Some("launch"),
                None,
            ));
        }

        match client.get(&version_url).send().await {
            Ok(response) if response.status().is_success() => {
                let version =
                    response
                        .json::<LightpandaVersionResponse>()
                        .await
                        .map_err(|err| {
                            RunnerFailure::new(
                                "launch_endpoint_unavailable",
                                format!(
                                    "failed to decode lightpanda /json/version response: {err}"
                                ),
                                Some("launch"),
                                Some(err.to_string()),
                            )
                        })?;

                if !version.web_socket_debugger_url.trim().is_empty() {
                    return Ok(version.web_socket_debugger_url);
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }

        sleep(Duration::from_millis(LIGHTPANDA_WAIT_POLL_MS)).await;
    }
}

fn apply_lightpanda_env(
    cmd: &mut Command,
    task: &RunnerTask,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
) {
    if let Some(runtime) = fingerprint_runtime {
        for (key, value) in &runtime.envs {
            cmd.env(key, value);
        }
    }

    if let Some(proxy) = &task.proxy {
        cmd.env("LIGHTPANDA_PROXY_ID", &proxy.id)
            .env("LIGHTPANDA_PROXY_SCHEME", &proxy.scheme)
            .env("LIGHTPANDA_PROXY_HOST", &proxy.host)
            .env("LIGHTPANDA_PROXY_PORT", proxy.port.to_string());

        if let Some(username) = &proxy.username {
            cmd.env("LIGHTPANDA_PROXY_USERNAME", username);
        }
        if let Some(password) = &proxy.password {
            cmd.env("LIGHTPANDA_PROXY_PASSWORD", password);
        }
    }
}

fn server_timeout_seconds(task_timeout_seconds: u64) -> u64 {
    task_timeout_seconds.saturating_add(15).clamp(15, 300)
}

fn spawn_lightpanda_serve(
    bin: &str,
    task: &RunnerTask,
    timeout_seconds: u64,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
) -> Result<(SpawnedLightpanda, u16), io::Error> {
    let port = allocate_loopback_port()?;
    let mut cmd = Command::new(bin);
    cmd.arg("serve")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--timeout")
        .arg(server_timeout_seconds(timeout_seconds).to_string())
        .arg("--log-format")
        .arg("pretty")
        .arg("--log-level")
        .arg("info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    apply_lightpanda_env(&mut cmd, task, fingerprint_runtime);

    #[cfg(unix)]
    {
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = cmd.spawn()?;
    let pid = child.id().ok_or_else(|| {
        io::Error::new(io::ErrorKind::Other, "lightpanda child did not expose pid")
    })?;

    let stdout_handle = tokio::spawn(read_stream_to_string(child.stdout.take()));
    let stderr_handle = tokio::spawn(read_stream_to_string(child.stderr.take()));

    Ok((
        SpawnedLightpanda {
            child,
            pid,
            stdout_handle,
            stderr_handle,
        },
        port,
    ))
}

#[cfg(unix)]
fn signal_process_group(pid: u32, signal: i32) -> Result<(), String> {
    let rc = unsafe { libc::kill(-(pid as i32), signal) };
    if rc == 0 {
        return Ok(());
    }

    let err = io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(err.to_string())
    }
}

#[cfg(not(unix))]
fn signal_process_group(_pid: u32, _signal: i32) -> Result<(), String> {
    Ok(())
}

async fn shutdown_spawned_process(spawned: &mut SpawnedLightpanda) -> Result<Option<i32>, String> {
    if let Some(status) = spawned
        .child
        .try_wait()
        .map_err(|err| format!("failed to inspect child state before cleanup: {err}"))?
    {
        return Ok(status.code());
    }

    #[cfg(unix)]
    signal_process_group(spawned.pid, libc::SIGTERM)?;

    #[cfg(not(unix))]
    spawned
        .child
        .kill()
        .await
        .map_err(|err| format!("failed to terminate child process: {err}"))?;

    match timeout(Duration::from_secs(2), spawned.child.wait()).await {
        Ok(wait_result) => wait_result
            .map(|status| status.code())
            .map_err(|err| format!("failed to wait for lightpanda child after SIGTERM: {err}")),
        Err(_) => {
            #[cfg(unix)]
            signal_process_group(spawned.pid, libc::SIGKILL)?;

            #[cfg(not(unix))]
            spawned
                .child
                .kill()
                .await
                .map_err(|err| format!("failed to force-kill child process: {err}"))?;

            timeout(Duration::from_secs(2), spawned.child.wait())
                .await
                .map_err(|_| "lightpanda child did not exit after SIGKILL".to_string())?
                .map(|status| status.code())
                .map_err(|err| format!("failed to wait for lightpanda child after SIGKILL: {err}"))
        }
    }
}

async fn collect_joined_output(handle: JoinHandle<String>, label: &str) -> String {
    handle
        .await
        .unwrap_or_else(|err| format!("<{label} join error: {err}>"))
}

fn reconcile_failure_with_exit_code(
    mut failure: RunnerFailure,
    exit_code: Option<i32>,
) -> RunnerFailure {
    let exit_kind = classify_exit_code(exit_code);
    if matches!(
        failure.error_kind,
        "cdp_connect_failed" | "launch_endpoint_unavailable" | "runner_connection_closed"
    ) {
        failure.error_kind = exit_kind;
        if !failure.message.contains("exit_code") {
            failure.message = format!(
                "{} (exit_code={exit_code:?})",
                failure.message.trim_end_matches('.')
            );
        }
    }
    failure
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

impl LightpandaRunner {
    fn register_pid(&self, task_id: &str, pid: u32) {
        let mut guard = self
            .running_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.insert(task_id.to_string(), pid);
    }

    fn unregister_pid(&self, task_id: &str) {
        let mut guard = self
            .running_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.remove(task_id);
    }
}

#[async_trait]
impl TaskRunner for LightpandaRunner {
    fn name(&self) -> &'static str {
        "lightpanda"
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_timeout: true,
            supports_cancel_running: true,
            supports_artifacts: false,
        }
    }

    async fn cancel_running(&self, task_id: &str) -> RunnerCancelResult {
        let pid = {
            let guard = self
                .running_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.get(task_id).copied()
        };

        match pid {
            Some(pid) => match signal_process_group(pid, libc::SIGTERM) {
                Ok(()) => {
                    self.unregister_pid(task_id);
                    RunnerCancelResult {
                        accepted: true,
                        message: format!(
                            "lightpanda runner sent SIGTERM to running process group for task_id={task_id}, pid={pid}"
                        ),
                    }
                }
                Err(err) => RunnerCancelResult {
                    accepted: false,
                    message: format!(
                        "lightpanda runner failed to terminate process group for task_id={task_id}, pid={pid}: {err}"
                    ),
                },
            },
            None => RunnerCancelResult {
                accepted: false,
                message: format!("lightpanda runner has no registered running process for task_id={task_id}"),
            },
        }
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        let requested_action = extract_action(&task);
        let action = match normalize_action(&requested_action) {
            Some(action) => action,
            None => {
                return invalid_input(
                    &task,
                    requested_action.as_str(),
                    requested_action.as_str(),
                    "lightpanda runner currently supports only action=open_page, action=get_html, action=get_title, action=get_final_url, action=extract_text (fetch is accepted as an alias for open_page)",
                    extract_url(&task.payload).as_deref(),
                )
            }
        };

        let url = match extract_url(&task.payload) {
            Some(url) => url,
            None => {
                return invalid_input(
                    &task,
                    requested_action.as_str(),
                    action,
                    "lightpanda runner requires a non-empty url in task payload",
                    None,
                )
            }
        };

        if !looks_like_url(&url) {
            return invalid_input(
                &task,
                requested_action.as_str(),
                action,
                "lightpanda runner currently only accepts http:// or https:// urls",
                Some(&url),
            );
        }

        let timeout_seconds = task.timeout_seconds.unwrap_or(10).clamp(1, 120) as u64;
        let bin = lightpanda_bin();
        let fingerprint_runtime = build_lightpanda_fingerprint_runtime(&task);
        let test_ws_endpoint = lightpanda_test_ws_endpoint();

        if let Some(ws_endpoint) = test_ws_endpoint {
            let execution = timeout(
                Duration::from_secs(timeout_seconds),
                perform_browser_action(
                    &ws_endpoint,
                    &task,
                    action,
                    &url,
                    task.session_cookies.as_deref(),
                    task.session_local_storage.as_ref(),
                    task.session_session_storage.as_ref(),
                ),
            )
            .await;

            return match execution {
                Ok(Ok(browser_result)) => {
                    let form_failed = browser_result
                        .form_action_status
                        .as_deref()
                        .is_some_and(|status| status == FORM_ACTION_STATUS_FAILED);
                    let message = browser_result
                        .form_action_error_message
                        .clone()
                        .unwrap_or_else(|| format!("lightpanda serve + cdp completed {action} successfully"));
                    build_result(
                        if form_failed {
                            RunnerOutcomeStatus::Failed
                        } else {
                            RunnerOutcomeStatus::Succeeded
                        },
                        !form_failed,
                        if form_failed { "failed" } else { "succeeded" },
                        form_failed.then_some("form_action_failed"),
                        None,
                        if form_failed { Some("action") } else { None },
                        requested_action.as_str(),
                        action,
                        &task,
                        Some(&url),
                        Some(timeout_seconds),
                        Some(&bin),
                        None,
                        None,
                        None,
                        Some(&browser_result),
                        fingerprint_runtime.as_ref(),
                        message,
                    )
                }
                Ok(Err(failure)) => {
                    let stderr_preview = merge_stderr_preview(None, failure.stderr_hint.clone());
                    let browser_failure_signal =
                        detect_browser_failure_signal(stderr_preview.as_deref(), None);
                    let outcome = if failure.error_kind == "runner_cancelled" {
                        RunnerOutcomeStatus::Cancelled
                    } else {
                        RunnerOutcomeStatus::Failed
                    };
                    let status_text = match outcome {
                        RunnerOutcomeStatus::Cancelled => "cancelled",
                        _ => "failed",
                    };
                    build_result(
                        outcome,
                        false,
                        status_text,
                        Some(failure.error_kind),
                        browser_failure_signal,
                        failure.stage_hint,
                        requested_action.as_str(),
                        action,
                        &task,
                        Some(&url),
                        Some(timeout_seconds),
                        Some(&bin),
                        None,
                        None,
                        stderr_preview,
                        None,
                        fingerprint_runtime.as_ref(),
                        failure.message,
                    )
                }
                Err(_) => build_result(
                    RunnerOutcomeStatus::TimedOut,
                    false,
                    RUN_STATUS_TIMED_OUT,
                    Some("timeout"),
                    None,
                    Some("navigate"),
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    None,
                    None,
                    Some(format!(
                        "lightpanda test websocket endpoint did not finish within {timeout_seconds}s"
                    )),
                    None,
                    fingerprint_runtime.as_ref(),
                    format!("lightpanda serve + cdp timed out after {timeout_seconds}s"),
                ),
            };
        }

        let (mut spawned, port) = match spawn_lightpanda_serve(
            &bin,
            &task,
            timeout_seconds,
            fingerprint_runtime.as_ref(),
        ) {
            Ok(spawned) => spawned,
            Err(err) => {
                let message = format!("failed to spawn lightpanda binary: {err}");
                return build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    "failed",
                    Some(classify_spawn_error(&err)),
                    None,
                    Some("launch"),
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    None,
                    None,
                    None,
                    None,
                    fingerprint_runtime.as_ref(),
                    message,
                );
            }
        };

        self.register_pid(&task.task_id, spawned.pid);

        let execution = timeout(Duration::from_secs(timeout_seconds), async {
            let ws_endpoint = wait_for_ws_endpoint(&mut spawned.child, port).await?;
            perform_browser_action(
                &ws_endpoint,
                &task,
                action,
                &url,
                task.session_cookies.as_deref(),
                task.session_local_storage.as_ref(),
                task.session_session_storage.as_ref(),
            )
            .await
        })
        .await;

        self.unregister_pid(&task.task_id);

        let shutdown_result = shutdown_spawned_process(&mut spawned).await;
        let stdout = collect_joined_output(spawned.stdout_handle, "stdout").await;
        let stderr = collect_joined_output(spawned.stderr_handle, "stderr").await;
        let stdout_preview = preview_if_non_empty(stdout, STDOUT_PREVIEW_LIMIT);
        let stderr_preview = preview_if_non_empty(stderr, STDERR_PREVIEW_LIMIT);

        let process_exit_code = shutdown_result.as_ref().ok().copied().flatten();

        match execution {
            Ok(Ok(browser_result)) => match shutdown_result {
                Ok(_) => {
                    let form_failed = browser_result
                        .form_action_status
                        .as_deref()
                        .is_some_and(|status| status == FORM_ACTION_STATUS_FAILED);
                    let message = if form_failed {
                        browser_result
                            .form_action_error_message
                            .clone()
                            .unwrap_or_else(|| "lightpanda form action failed".to_string())
                    } else if let Some(runtime) = fingerprint_runtime.as_ref() {
                        let ignored_note = if runtime.ignored_fields.is_empty() {
                            String::new()
                        } else {
                            "; warning=fingerprint profile contains fields that lightpanda does not currently consume".to_string()
                        };
                        format!(
                            "lightpanda serve + cdp completed {action} successfully; fingerprint_runtime: applied_fields={:?}, ignored_fields={:?}, applied_count={}, ignored_count={}, consumption_status={}{}",
                            runtime.applied_fields,
                            runtime.ignored_fields,
                            runtime.applied_fields.len(),
                            runtime.ignored_fields.len(),
                            fingerprint_consumption_status(runtime),
                            ignored_note,
                        )
                    } else {
                        format!("lightpanda serve + cdp completed {action} successfully")
                    };

                    build_result(
                        if form_failed {
                            RunnerOutcomeStatus::Failed
                        } else {
                            RunnerOutcomeStatus::Succeeded
                        },
                        !form_failed,
                        if form_failed { "failed" } else { "succeeded" },
                        form_failed.then_some("form_action_failed"),
                        None,
                        Some("action"),
                        requested_action.as_str(),
                        action,
                        &task,
                        Some(&url),
                        Some(timeout_seconds),
                        Some(&bin),
                        process_exit_code,
                        stdout_preview,
                        stderr_preview,
                        Some(&browser_result),
                        fingerprint_runtime.as_ref(),
                        message,
                    )
                }
                Err(cleanup_err) => build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    "failed",
                    Some("process_wait_failed"),
                    None,
                    Some("action"),
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    process_exit_code,
                    stdout_preview,
                    merge_stderr_preview(
                        stderr_preview,
                        Some(format!("cleanup failed: {cleanup_err}")),
                    ),
                    Some(&browser_result),
                    fingerprint_runtime.as_ref(),
                    format!("lightpanda serve + cdp cleanup failed after successful {action}: {cleanup_err}"),
                ),
            },
            Ok(Err(failure)) => {
                let failure = if process_exit_code.is_some() {
                    reconcile_failure_with_exit_code(failure, process_exit_code)
                } else {
                    failure
                };
                let stderr_preview = merge_stderr_preview(stderr_preview, failure.stderr_hint.clone());
                let browser_failure_signal =
                    detect_browser_failure_signal(stderr_preview.as_deref(), stdout_preview.as_deref());
                let outcome = if failure.error_kind == "runner_cancelled" {
                    RunnerOutcomeStatus::Cancelled
                } else {
                    RunnerOutcomeStatus::Failed
                };
                let status_text = match outcome {
                    RunnerOutcomeStatus::Cancelled => "cancelled",
                    _ => "failed",
                };

                build_result(
                    outcome,
                    false,
                    status_text,
                    Some(failure.error_kind),
                    browser_failure_signal,
                    failure.stage_hint,
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    process_exit_code,
                    stdout_preview,
                    stderr_preview,
                    None,
                    fingerprint_runtime.as_ref(),
                    failure.message,
                )
            }
            Err(_) => {
                let stderr_preview = merge_stderr_preview(
                    stderr_preview,
                    match shutdown_result {
                        Ok(_) => Some(format!(
                            "lightpanda serve + cdp timed out after {timeout_seconds}s"
                        )),
                        Err(cleanup_err) => Some(format!(
                            "lightpanda serve + cdp timed out after {timeout_seconds}s; cleanup error: {cleanup_err}"
                        )),
                    },
                );
                build_result(
                    RunnerOutcomeStatus::TimedOut,
                    false,
                    RUN_STATUS_TIMED_OUT,
                    Some("timeout"),
                    None,
                    Some("navigate"),
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    process_exit_code,
                    stdout_preview,
                    stderr_preview,
                    None,
                    fingerprint_runtime.as_ref(),
                    format!("lightpanda serve + cdp timed out after {timeout_seconds}s"),
                )
            }
        }
    }
}
