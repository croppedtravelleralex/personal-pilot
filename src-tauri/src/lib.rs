use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

const READY_PREFIX: &str = "PERSONAL_PILOT_CORE_READY ";
const BODY_PREVIEW_LIMIT: usize = 240;
const SIDECAR_QUIT_READ_TIMEOUT: Duration = Duration::from_secs(3);
const SIDECAR_QUIT_WRITE_TIMEOUT: Duration = Duration::from_secs(3);

type HttpHeaders = Vec<(String, String)>;

#[derive(Clone, Copy)]
enum SidecarShutdownMode {
    Full,
    AppOnly,
}

impl SidecarShutdownMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::AppOnly => "app-only",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreStatus {
    running: bool,
    event_url: Option<String>,
    #[serde(skip_serializing)]
    bridge_url: Option<String>,
    #[serde(skip_serializing)]
    bridge_token: Option<String>,
    pid: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoreReady {
    bridge_url: String,
    event_url: String,
    bridge_token: String,
}

struct CoreInner {
    child: Option<Child>,
    bridge_url: Option<String>,
    bridge_token: Option<String>,
    event_url: Option<String>,
    quitting: bool,
}

struct CoreManager {
    inner: Mutex<CoreInner>,
    ready: Condvar,
}

impl CoreManager {
    fn new() -> Self {
        Self {
            inner: Mutex::new(CoreInner {
                child: None,
                bridge_url: None,
                bridge_token: None,
                event_url: None,
                quitting: false,
            }),
            ready: Condvar::new(),
        }
    }

    fn status(&self) -> CoreStatus {
        let mut inner = self.inner.lock().expect("core mutex poisoned");
        prune_exited_child(&mut inner);
        CoreStatus {
            running: inner.child.is_some(),
            event_url: inner.event_url.clone(),
            bridge_url: inner.bridge_url.clone(),
            bridge_token: inner.bridge_token.clone(),
            pid: inner.child.as_ref().map(|child| child.id()),
        }
    }

    fn ensure_started(&self, app: &tauri::AppHandle) -> Result<CoreStatus, String> {
        let mut needs_stdout_watcher = false;
        {
            let mut inner = self.inner.lock().map_err(|_| "core mutex poisoned")?;
            prune_exited_child(&mut inner);
            if inner.quitting {
                return Err("app is quitting".to_string());
            }
            if inner.child.is_some() && inner.bridge_url.is_some() && inner.bridge_token.is_some() {
                return Ok(CoreStatus {
                    running: true,
                    event_url: inner.event_url.clone(),
                    bridge_url: inner.bridge_url.clone(),
                    bridge_token: inner.bridge_token.clone(),
                    pid: inner.child.as_ref().map(|child| child.id()),
                });
            }
            if inner.child.is_none() {
                let child = spawn_core(app)?;
                inner.bridge_url = None;
                inner.bridge_token = None;
                inner.event_url = None;
                inner.child = Some(child);
                needs_stdout_watcher = true;
            }
        }
        if needs_stdout_watcher {
            start_stdout_watcher(self, app);
        }

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut inner = self.inner.lock().map_err(|_| "core mutex poisoned")?;
        loop {
            prune_exited_child(&mut inner);
            if inner.quitting {
                return Err("app is quitting".to_string());
            }
            if inner.child.is_none() {
                return Err("Go sidecar exited before reporting ready".to_string());
            }
            if inner.bridge_url.is_some() && inner.bridge_token.is_some() {
                return Ok(CoreStatus {
                    running: true,
                    event_url: inner.event_url.clone(),
                    bridge_url: inner.bridge_url.clone(),
                    bridge_token: inner.bridge_token.clone(),
                    pid: inner.child.as_ref().map(|child| child.id()),
                });
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("Timed out waiting for Go sidecar readiness".to_string());
            }
            let wait_for = deadline
                .saturating_duration_since(now)
                .min(Duration::from_millis(250));
            let (guard, _) = self
                .ready
                .wait_timeout(inner, wait_for)
                .map_err(|_| "core condvar poisoned")?;
            inner = guard;
        }
    }

    fn begin_app_quit(&self) {
        let mut inner = self.inner.lock().expect("core mutex poisoned");
        inner.quitting = true;
        self.ready.notify_all();
    }

    fn stop(&self) {
        self.stop_with_mode(SidecarShutdownMode::Full, Duration::from_secs(5));
    }

    fn stop_with_mode(&self, mode: SidecarShutdownMode, wait_timeout: Duration) {
        let (mut child, bridge_url, bridge_token) = {
            let mut inner = self.inner.lock().expect("core mutex poisoned");
            let child = inner.child.take();
            let bridge_url = inner.bridge_url.take();
            let bridge_token = inner.bridge_token.take();
            inner.event_url = None;
            (child, bridge_url, bridge_token)
        };

        if let Some(url) = bridge_url {
            let _ = http_post_json_with_timeouts(
                &url,
                "/shutdown",
                bridge_token.as_deref(),
                &json!({ "mode": mode.as_str() }),
                SIDECAR_QUIT_READ_TIMEOUT,
                SIDECAR_QUIT_WRITE_TIMEOUT,
            );
        }

        if let Some(mut child) = child.take() {
            let deadline = Instant::now() + wait_timeout;
            while Instant::now() < deadline {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn prune_exited_child(inner: &mut CoreInner) {
    if let Some(child) = inner.child.as_mut() {
        if matches!(child.try_wait(), Ok(Some(_))) {
            inner.child = None;
            inner.bridge_url = None;
            inner.bridge_token = None;
            inner.event_url = None;
        }
    }
}

fn start_stdout_watcher(manager: &CoreManager, app: &tauri::AppHandle) {
    let state = app.state::<CoreManager>().inner();
    let stdout = {
        let mut inner = state.inner.lock().expect("core mutex poisoned");
        inner.child.as_mut().and_then(|child| child.stdout.take())
    };

    if let Some(stdout) = stdout {
        let app_handle = app.clone();
        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut buffer = String::new();
            loop {
                buffer.clear();
                match std::io::BufRead::read_line(&mut reader, &mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = buffer.trim();
                        if let Some(raw) = line.strip_prefix(READY_PREFIX) {
                            if let Ok(ready) = serde_json::from_str::<CoreReady>(raw) {
                                let state = app_handle.state::<CoreManager>();
                                let mut inner = state.inner.lock().expect("core mutex poisoned");
                                inner.bridge_url = Some(ready.bridge_url);
                                inner.bridge_token = Some(ready.bridge_token);
                                inner.event_url = Some(ready.event_url);
                                state.ready.notify_all();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    } else {
        manager.ready.notify_all();
    }

    let stderr = {
        let mut inner = manager.inner.lock().expect("core mutex poisoned");
        inner.child.as_mut().and_then(|child| child.stderr.take())
    };
    if let Some(stderr) = stderr {
        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stderr);
            let mut sink = String::new();
            while std::io::BufRead::read_line(&mut reader, &mut sink).unwrap_or(0) > 0 {
                sink.clear();
            }
        });
    }
}

fn spawn_core(app: &tauri::AppHandle) -> Result<Child, String> {
    let sidecar = resolve_sidecar_path(app)?;
    let app_root = resolve_app_root();

    let mut command = Command::new(&sidecar);
    command
        .arg("--app-root")
        .arg(&app_root)
        .arg("--version")
        .arg(env!("CARGO_PKG_VERSION"))
        .current_dir(&app_root)
        .env("PERSONAL_PILOT_TAURI_SIDECAR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    command
        .spawn()
        .map_err(|err| format!("failed to start Go sidecar {}: {err}", sidecar.display()))
}

fn resolve_sidecar_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Ok(raw) = env::var("PERSONAL_PILOT_CORE_EXE") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(path);
        }
    }

    let app_root = resolve_app_root();
    let mut candidates = vec![
        app_root.join("bin").join("personal-pilot-core.exe"),
        app_root
            .join("bin")
            .join("personal-pilot-core-x86_64-pc-windows-msvc.exe"),
    ];

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("personal-pilot-core.exe"));
        candidates.push(resource_dir.join("personal-pilot-core-x86_64-pc-windows-msvc.exe"));
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("personal-pilot-core.exe"));
            candidates.push(exe_dir.join("personal-pilot-core-x86_64-pc-windows-msvc.exe"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "personal-pilot-core sidecar executable was not found".to_string())
}

fn resolve_app_root() -> PathBuf {
    if let Ok(raw) = env::var("PERSONAL_PILOT_APP_ROOT") {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            return path;
        }
    }

    if cfg!(debug_assertions) {
        return Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    }

    env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn http_post_json(
    base_url: &str,
    path: &str,
    bridge_token: Option<&str>,
    body: &Value,
) -> Result<Value, String> {
    http_post_json_with_timeouts(
        base_url,
        path,
        bridge_token,
        body,
        Duration::from_secs(120),
        Duration::from_secs(10),
    )
}

fn http_post_json_with_timeouts(
    base_url: &str,
    path: &str,
    bridge_token: Option<&str>,
    body: &Value,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<Value, String> {
    let (host, port) = parse_local_http_url(base_url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|err| format!("failed to connect sidecar {base_url}: {err}"))?;
    stream
        .set_read_timeout(Some(read_timeout))
        .map_err(|err| err.to_string())?;
    stream
        .set_write_timeout(Some(write_timeout))
        .map_err(|err| err.to_string())?;

    let payload = serde_json::to_vec(body).map_err(|err| err.to_string())?;
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\n"
    );
    if let Some(token) = bridge_token {
        request.push_str(&format!("X-Personal-Pilot-Bridge-Token: {token}\r\n"));
    }
    request.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    ));
    stream
        .write_all(request.as_bytes())
        .and_then(|_| stream.write_all(&payload))
        .map_err(|err| format!("failed to write sidecar request: {err}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|err| format!("failed to read sidecar response: {err}"))?;
    parse_http_json_response(&response)
}

fn parse_local_http_url(raw: &str) -> Result<(String, u16), String> {
    let without_scheme = raw
        .strip_prefix("http://")
        .ok_or_else(|| format!("unsupported sidecar URL: {raw}"))?;
    let authority = without_scheme
        .split('/')
        .next()
        .ok_or_else(|| format!("invalid sidecar URL: {raw}"))?;
    let (host, port_raw) = authority
        .rsplit_once(':')
        .ok_or_else(|| format!("missing sidecar URL port: {raw}"))?;
    if host != "127.0.0.1" && host != "localhost" {
        return Err(format!("sidecar URL must be localhost: {raw}"));
    }
    let port = port_raw
        .parse::<u16>()
        .map_err(|err| format!("invalid sidecar URL port: {err}"))?;
    Ok((host.to_string(), port))
}

fn parse_http_json_response(response: &[u8]) -> Result<Value, String> {
    let (head, body_start) = split_http_head(response).ok_or_else(|| {
        format!(
            "invalid sidecar HTTP response: {}",
            response_context("<missing>", &HttpHeaders::new(), response)
        )
    })?;
    let raw_body = &response[body_start..];
    let head = std::str::from_utf8(head).map_err(|err| {
        format!(
            "invalid sidecar HTTP header: {err}; {}",
            response_context("<unreadable>", &HttpHeaders::new(), raw_body)
        )
    })?;
    let (status_line, headers) = parse_http_head(head)?;
    let decoded_body = decode_http_body(&headers, raw_body);
    let context_body = decoded_body.as_deref().unwrap_or(raw_body);

    if !is_success_status(status_line) {
        return Err(format!(
            "sidecar HTTP error: {}",
            response_context(status_line, &headers, context_body)
        ));
    }

    let body = decoded_body.map_err(|err| {
        format!(
            "{err}; {}",
            response_context(status_line, &headers, raw_body)
        )
    })?;
    serde_json::from_slice(&body).map_err(|err| {
        format!(
            "invalid sidecar JSON response: {err}; {}",
            response_context(status_line, &headers, &body)
        )
    })
}

fn split_http_head(response: &[u8]) -> Option<(&[u8], usize)> {
    response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|head_end| (&response[..head_end], head_end + 4))
}

fn parse_http_head(head: &str) -> Result<(&str, HttpHeaders), String> {
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .ok_or_else(|| "invalid sidecar HTTP response: missing status line".to_string())?;
    let mut headers = HttpHeaders::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| format!("invalid sidecar HTTP header: {line}"))?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    Ok((status_line, headers))
}

fn is_success_status(status_line: &str) -> bool {
    let mut parts = status_line.split_whitespace();
    let Some(version) = parts.next() else {
        return false;
    };
    if !version.starts_with("HTTP/") {
        return false;
    }
    parts
        .next()
        .and_then(|code| code.parse::<u16>().ok())
        .is_some_and(|code| (200..300).contains(&code))
}

fn decode_http_body(headers: &HttpHeaders, raw_body: &[u8]) -> Result<Vec<u8>, String> {
    if has_chunked_transfer_encoding(headers) {
        return decode_chunked_body(raw_body);
    }

    if let Some(length) = content_length(headers)? {
        if raw_body.len() < length {
            return Err(format!(
                "incomplete sidecar HTTP body: expected {length} bytes, got {} bytes",
                raw_body.len()
            ));
        }
        return Ok(raw_body[..length].to_vec());
    }

    Ok(raw_body.to_vec())
}

fn has_chunked_transfer_encoding(headers: &HttpHeaders) -> bool {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("transfer-encoding"))
        .any(|(_, value)| {
            value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
        })
}

fn content_length(headers: &HttpHeaders) -> Result<Option<usize>, String> {
    let mut parsed_length = None;
    for (_, value) in headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        let length = value
            .parse::<usize>()
            .map_err(|err| format!("invalid sidecar Content-Length header: {err}"))?;
        if parsed_length.is_some_and(|existing| existing != length) {
            return Err("conflicting sidecar Content-Length headers".to_string());
        }
        parsed_length = Some(length);
    }
    Ok(parsed_length)
}

fn decode_chunked_body(raw_body: &[u8]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let mut position = 0;

    loop {
        let line_end = find_crlf(&raw_body[position..])
            .ok_or_else(|| "invalid sidecar chunked body: missing chunk size".to_string())?;
        let size_line = std::str::from_utf8(&raw_body[position..position + line_end])
            .map_err(|err| format!("invalid sidecar chunk size: {err}"))?;
        let size_text = size_line
            .split_once(';')
            .map(|(size, _)| size)
            .unwrap_or(size_line)
            .trim();
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|err| format!("invalid sidecar chunk size `{size_text}`: {err}"))?;
        position += line_end + 2;

        if size == 0 {
            loop {
                let trailer_end = find_crlf(&raw_body[position..]).ok_or_else(|| {
                    "invalid sidecar chunked body: missing final chunk terminator".to_string()
                })?;
                position += trailer_end + 2;
                if trailer_end == 0 {
                    return Ok(body);
                }
            }
        }

        let data_end = position
            .checked_add(size)
            .ok_or_else(|| "sidecar chunk size overflow".to_string())?;
        let next_position = data_end
            .checked_add(2)
            .ok_or_else(|| "sidecar chunk size overflow".to_string())?;
        if raw_body.len() < next_position {
            return Err(format!(
                "incomplete sidecar chunked body: expected chunk of {size} bytes"
            ));
        }
        if &raw_body[data_end..next_position] != b"\r\n" {
            return Err("invalid sidecar chunked body: missing chunk terminator".to_string());
        }
        body.extend_from_slice(&raw_body[position..data_end]);
        position = next_position;
    }
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

fn response_context(status_line: &str, headers: &HttpHeaders, body: &[u8]) -> String {
    format!(
        "status_line=\"{}\", headers=\"{}\", body_preview=\"{}\"",
        status_line,
        key_header_preview(headers),
        body_preview(body)
    )
}

fn key_header_preview(headers: &HttpHeaders) -> String {
    let mut preview = Vec::new();
    for (name, value) in headers {
        if matches!(
            name.as_str(),
            "content-length" | "transfer-encoding" | "content-type"
        ) {
            preview.push(format!("{name}: {value}"));
        }
    }
    if preview.is_empty() {
        "none".to_string()
    } else {
        preview.join("; ")
    }
}

fn body_preview(body: &[u8]) -> String {
    if body.is_empty() {
        return "<empty>".to_string();
    }
    let preview_len = body.len().min(BODY_PREVIEW_LIMIT);
    let mut preview = String::from_utf8_lossy(&body[..preview_len])
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('"', "\\\"");
    if body.len() > BODY_PREVIEW_LIMIT {
        preview.push_str("...");
    }
    preview
}

#[tauri::command]
fn core_start(
    app: tauri::AppHandle,
    core: tauri::State<CoreManager>,
) -> Result<CoreStatus, String> {
    core.ensure_started(&app)
}

#[tauri::command]
fn core_status(core: tauri::State<CoreManager>) -> Result<CoreStatus, String> {
    Ok(core.status())
}

#[tauri::command]
async fn core_rpc(
    app: tauri::AppHandle,
    method: String,
    args: Vec<Value>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let core = app.state::<CoreManager>();
        let status = core.ensure_started(&app)?;
        let base_url = status
            .bridge_url
            .ok_or_else(|| "Go sidecar bridge URL is not ready".to_string())?;
        let bridge_token = status
            .bridge_token
            .ok_or_else(|| "Go sidecar bridge token is not ready".to_string())?;
        let response = http_post_json(
            &base_url,
            "/rpc",
            Some(&bridge_token),
            &json!({ "method": method, "args": args }),
        )?;
        if response.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(response.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("sidecar RPC failed")
                .to_string())
        }
    })
    .await
    .map_err(|err| format!("sidecar RPC worker failed: {err}"))?
}

#[tauri::command]
fn core_stop(core: tauri::State<CoreManager>) -> Result<(), String> {
    core.stop();
    Ok(())
}

#[tauri::command]
fn app_environment() -> Result<Value, String> {
    Ok(json!({
        "buildType": if cfg!(debug_assertions) { "dev" } else { "production" },
        "platform": env::consts::OS,
        "arch": env::consts::ARCH
    }))
}

#[tauri::command]
fn app_window_hide(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn app_window_show(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|err| err.to_string())?;
        window.set_focus().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn app_window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.minimize().map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn app_quit(app: tauri::AppHandle) -> Result<(), String> {
    start_app_quit(app, SidecarShutdownMode::Full, Duration::from_secs(10));
    Ok(())
}

#[tauri::command]
fn app_quit_app_only(app: tauri::AppHandle) -> Result<(), String> {
    start_app_quit(app, SidecarShutdownMode::AppOnly, Duration::from_secs(5));
    Ok(())
}

#[tauri::command]
fn app_quit_full(app: tauri::AppHandle) -> Result<(), String> {
    start_app_quit(app, SidecarShutdownMode::Full, Duration::from_secs(10));
    Ok(())
}

fn start_app_quit(app: tauri::AppHandle, mode: SidecarShutdownMode, wait_timeout: Duration) {
    thread::spawn(move || {
        let core = app.state::<CoreManager>();
        core.begin_app_quit();
        core.stop_with_mode(mode, wait_timeout);
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(headers: &[(&str, String)], body: &[u8]) -> Vec<u8> {
        let mut response = b"HTTP/1.1 200 OK\r\n".to_vec();
        for (name, value) in headers {
            response.extend_from_slice(name.as_bytes());
            response.extend_from_slice(b": ");
            response.extend_from_slice(value.as_bytes());
            response.extend_from_slice(b"\r\n");
        }
        response.extend_from_slice(b"\r\n");
        response.extend_from_slice(body);
        response
    }

    #[test]
    fn parses_content_length_json_body() {
        let body = br#"{"ok":true,"result":{"answer":42}}"#;
        let response = response(
            &[
                ("Content-Type", "application/json".to_string()),
                ("Content-Length", body.len().to_string()),
            ],
            body,
        );

        let parsed = parse_http_json_response(&response).expect("content-length JSON parses");

        assert_eq!(parsed, json!({"ok": true, "result": {"answer": 42}}));
    }

    #[test]
    fn parses_chunked_json_body() {
        let chunked = b"6\r\n{\"ok\":\r\n4\r\ntrue\r\n1\r\n}\r\n0\r\n\r\n";
        let response = response(
            &[
                ("Content-Type", "application/json".to_string()),
                ("Transfer-Encoding", "chunked".to_string()),
            ],
            chunked,
        );

        let parsed = parse_http_json_response(&response).expect("chunked JSON parses");

        assert_eq!(parsed, json!({"ok": true}));
    }

    #[test]
    fn rejects_concatenated_json_bodies() {
        let body = br#"{"ok":true}{"extra":true}"#;
        let response = response(
            &[
                ("Content-Type", "application/json".to_string()),
                ("Content-Length", body.len().to_string()),
            ],
            body,
        );

        let error =
            parse_http_json_response(&response).expect_err("concatenated JSON bodies must fail");

        assert!(error.contains("invalid sidecar JSON response"));
        assert!(error.contains("status_line=\"HTTP/1.1 200 OK\""));
        assert!(error.contains("content-length"));
        assert!(error.contains("{\\\"ok\\\":true}{\\\"extra\\\":true}"));
    }
}

pub fn run() {
    tauri::Builder::default()
        .manage(CoreManager::new())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            core_start,
            core_status,
            core_rpc,
            core_stop,
            app_environment,
            app_window_hide,
            app_window_show,
            app_window_minimize,
            app_quit,
            app_quit_app_only,
            app_quit_full
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("app:request-close", ());
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<CoreManager>().stop();
            }
        });
}
