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

const READY_PREFIX: &str = "ANTBROWSER_CORE_READY ";

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

    fn stop(&self) {
        let (mut child, bridge_url, bridge_token) = {
            let mut inner = self.inner.lock().expect("core mutex poisoned");
            let child = inner.child.take();
            let bridge_url = inner.bridge_url.take();
            let bridge_token = inner.bridge_token.take();
            inner.event_url = None;
            (child, bridge_url, bridge_token)
        };

        if let Some(url) = bridge_url {
            let _ = http_post_json(&url, "/shutdown", bridge_token.as_deref(), &json!({}));
        }

        if let Some(mut child) = child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
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
        .env("ANTBROWSER_TAURI_SIDECAR", "1")
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
    if let Ok(raw) = env::var("ANTBROWSER_CORE_EXE") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(path);
        }
    }

    let app_root = resolve_app_root();
    let mut candidates = vec![
        app_root.join("bin").join("antbrowser-core.exe"),
        app_root
            .join("bin")
            .join("antbrowser-core-x86_64-pc-windows-msvc.exe"),
    ];

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("antbrowser-core.exe"));
        candidates.push(resource_dir.join("antbrowser-core-x86_64-pc-windows-msvc.exe"));
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("antbrowser-core.exe"));
            candidates.push(exe_dir.join("antbrowser-core-x86_64-pc-windows-msvc.exe"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "antbrowser-core sidecar executable was not found".to_string())
}

fn resolve_app_root() -> PathBuf {
    if let Ok(raw) = env::var("ANTBROWSER_APP_ROOT") {
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
    let (host, port) = parse_local_http_url(base_url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|err| format!("failed to connect sidecar {base_url}: {err}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|err| err.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|err| err.to_string())?;

    let payload = serde_json::to_vec(body).map_err(|err| err.to_string())?;
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\n"
    );
    if let Some(token) = bridge_token {
        request.push_str(&format!("X-Antbrowser-Bridge-Token: {token}\r\n"));
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
    let text = String::from_utf8_lossy(&response);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid sidecar HTTP response".to_string())?;
    if !head.starts_with("HTTP/1.1 2") && !head.starts_with("HTTP/1.0 2") {
        return Err(format!(
            "sidecar HTTP error: {}",
            head.lines().next().unwrap_or(head)
        ));
    }
    serde_json::from_str(body.trim()).map_err(|err| format!("invalid sidecar JSON response: {err}"))
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
fn core_rpc(
    app: tauri::AppHandle,
    core: tauri::State<CoreManager>,
    method: String,
    args: Vec<Value>,
) -> Result<Value, String> {
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
fn app_quit(app: tauri::AppHandle, core: tauri::State<CoreManager>) -> Result<(), String> {
    core.stop();
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn app_quit_app_only(app: tauri::AppHandle, core: tauri::State<CoreManager>) -> Result<(), String> {
    let status = core.ensure_started(&app)?;
    let base_url = status
        .bridge_url
        .ok_or_else(|| "Go sidecar bridge URL is not ready".to_string())?;
    let bridge_token = status
        .bridge_token
        .ok_or_else(|| "Go sidecar bridge token is not ready".to_string())?;
    let _ = http_post_json(
        &base_url,
        "/rpc",
        Some(&bridge_token),
        &json!({ "method": "QuitAppOnly", "args": [] }),
    )?;
    core.stop();
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn app_quit_full(app: tauri::AppHandle, core: tauri::State<CoreManager>) -> Result<(), String> {
    let status = core.ensure_started(&app)?;
    let base_url = status
        .bridge_url
        .ok_or_else(|| "Go sidecar bridge URL is not ready".to_string())?;
    let bridge_token = status
        .bridge_token
        .ok_or_else(|| "Go sidecar bridge token is not ready".to_string())?;
    let _ = http_post_json(
        &base_url,
        "/rpc",
        Some(&bridge_token),
        &json!({ "method": "ForceQuit", "args": [] }),
    )?;
    core.stop();
    app.exit(0);
    Ok(())
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
