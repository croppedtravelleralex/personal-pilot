use std::{
    env, fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use persona_pilot::runner::{
    lightpanda::LightpandaRunner, RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask,
    TaskRunner,
};
use serde::Serialize;
use serde_json::{json, Value};

const COLLECTOR_VERSION: &str = "validation-lightpanda-smoke-v1";

#[derive(Debug, Clone)]
struct SmokeConfig {
    url: String,
    repetitions: usize,
    timeout_seconds: i64,
    output_dir: PathBuf,
    profile_id: Option<String>,
    use_wsl_lightpanda: bool,
    wsl_distro: String,
    wsl_cdp_port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SmokeAttempt {
    attempt: usize,
    runner: String,
    status: String,
    evidence_status: String,
    failure_reason: Option<String>,
    signal_count: usize,
    categories: Vec<String>,
    signals: Vec<Value>,
    raw_result: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SmokeReport {
    report_id: String,
    generated_at: String,
    profile_id: Option<String>,
    collector_version: String,
    runner: String,
    url: String,
    repetitions: usize,
    timeout_seconds: i64,
    status: String,
    exit_reason: String,
    report_path: String,
    attempts: Vec<SmokeAttempt>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = parse_args(env::args().skip(1).collect())?;
    let report = run_smoke(config).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<SmokeConfig> {
    let mut url = "https://example.com/".to_string();
    let mut repetitions = 2usize;
    let mut timeout_seconds = 20i64;
    let mut output_dir = PathBuf::from("data/reports/validation-smoke");
    let mut profile_id = None;
    let mut use_wsl_lightpanda = false;
    let mut wsl_distro = "HermesUbuntu".to_string();
    let mut wsl_cdp_port = 9222u16;

    let mut index = 0usize;
    while index < args.len() {
        let key = &args[index];
        if key == "--use-wsl-lightpanda" {
            use_wsl_lightpanda = true;
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| anyhow!("missing value for {key}"))?;
        match key.as_str() {
            "--url" => url = value.clone(),
            "--repetitions" => repetitions = value.parse::<usize>().context("parse repetitions")?,
            "--timeout-seconds" => {
                timeout_seconds = value.parse::<i64>().context("parse timeout seconds")?
            }
            "--output-dir" => output_dir = PathBuf::from(value),
            "--profile-id" => profile_id = Some(value.clone()),
            "--wsl-distro" => wsl_distro = value.clone(),
            "--wsl-cdp-port" => wsl_cdp_port = value.parse::<u16>().context("parse WSL CDP port")?,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(anyhow!("unknown argument: {key}")),
        }
        index += 2;
    }

    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(anyhow!("--url must start with http:// or https://"));
    }
    if repetitions == 0 || repetitions > 10 {
        return Err(anyhow!("--repetitions must be between 1 and 10"));
    }
    if !(1..=120).contains(&timeout_seconds) {
        return Err(anyhow!("--timeout-seconds must be between 1 and 120"));
    }

    Ok(SmokeConfig {
        url,
        repetitions,
        timeout_seconds,
        output_dir,
        profile_id,
        use_wsl_lightpanda,
        wsl_distro,
        wsl_cdp_port,
    })
}

fn print_help() {
    println!(
        "validation_lightpanda_smoke --use-wsl-lightpanda --url https://example.com/ --repetitions 2 --timeout-seconds 20 --output-dir data/reports/validation-smoke"
    );
}

async fn run_smoke(config: SmokeConfig) -> Result<SmokeReport> {
    fs::create_dir_all(&config.output_dir)
        .with_context(|| format!("create {}", config.output_dir.display()))?;
    if config.use_wsl_lightpanda {
        prepare_wsl_lightpanda(&config)?;
    }
    let runner = LightpandaRunner::default();
    let report_id = format!("validation-lightpanda-smoke-{}", epoch_millis());
    let generated_at = now_ts_string();
    let mut attempts = Vec::new();

    for attempt in 1..=config.repetitions {
        let task = RunnerTask {
            task_id: format!("{report_id}-attempt-{attempt}"),
            attempt: attempt as i64,
            kind: "validation_probe".to_string(),
            payload: json!({
                "action": "validation_probe",
                "url": config.url.clone(),
                "collector": COLLECTOR_VERSION,
            }),
            timeout_seconds: Some(config.timeout_seconds),
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        attempts.push(summarize_attempt(attempt, runner.name(), runner.execute(task).await));
    }

    let status = overall_status(&attempts);
    let exit_reason = match status.as_str() {
        "passed" => "all attempts returned profile-browser runtime validation signals",
        "blocked" => "no attempt returned complete real Lightpanda/CDP validation evidence",
        _ => "one or more attempts failed or returned incomplete evidence",
    }
    .to_string();
    let report_path = config.output_dir.join(format!("{report_id}.json"));

    let mut report = SmokeReport {
        report_id,
        generated_at,
        profile_id: config.profile_id,
        collector_version: COLLECTOR_VERSION.to_string(),
        runner: runner.name().to_string(),
        url: config.url,
        repetitions: config.repetitions,
        timeout_seconds: config.timeout_seconds,
        status,
        exit_reason,
        report_path: report_path.to_string_lossy().to_string(),
        attempts,
    };
    report.report_path = report_path.to_string_lossy().to_string();
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("write {}", report_path.display()))?;
    Ok(report)
}

fn prepare_wsl_lightpanda(config: &SmokeConfig) -> Result<()> {
    let version = run_wsl(
        &config.wsl_distro,
        "LIGHTPANDA_DISABLE_TELEMETRY=true ~/.local/bin/lightpanda version",
    )?;
    if version.trim().is_empty() {
        return Err(anyhow!("WSL Lightpanda did not print a version"));
    }
    let version_url = format!("http://127.0.0.1:{}/json/version", config.wsl_cdp_port);
    if fetch_json(&version_url).is_err() {
        let start_command = format!(
            "LIGHTPANDA_DISABLE_TELEMETRY=true ~/.local/bin/lightpanda serve --host 127.0.0.1 --port {} --timeout 120 --log-level info --log-format pretty >/tmp/persona-pilot-lightpanda-cdp.log 2>&1",
            config.wsl_cdp_port
        );
        Command::new("wsl.exe")
            .args(["-d", &config.wsl_distro, "--", "bash", "-lc", &start_command])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("start WSL Lightpanda CDP server")?;
        for _ in 0..20 {
            if fetch_json(&version_url).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
    }
    let probe = fetch_json(&version_url).context("read WSL Lightpanda /json/version")?;
    let endpoint = probe
        .get("webSocketDebuggerUrl")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("WSL Lightpanda /json/version did not expose webSocketDebuggerUrl"))?;
    env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", endpoint);
    env::set_var("LIGHTPANDA_BIN", "wsl-lightpanda-cdp");
    env::set_var("NO_PROXY", "*");
    env::set_var("no_proxy", "*");
    eprintln!("[validation-smoke] WSL Lightpanda {} at {}", version.trim(), endpoint);
    Ok(())
}

fn run_wsl(distro: &str, command: &str) -> Result<String> {
    let output = Command::new("wsl.exe")
        .args(["-d", distro, "--", "bash", "-lc", command])
        .output()
        .context("run WSL command")?;
    if !output.status.success() {
        return Err(anyhow!(
            "WSL command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn fetch_json(url: &str) -> Result<Value> {
    let (host, port, path) = parse_local_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).context("connect local HTTP endpoint")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("set read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("set write timeout")?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .context("write HTTP request")?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("read HTTP response")?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(anyhow!("local HTTP endpoint did not return 200"));
    }
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .ok_or_else(|| anyhow!("local HTTP response did not include a body"))?;
    serde_json::from_str::<Value>(body).context("decode JSON")
}

fn parse_local_http_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("only http:// URLs are supported for local probe"))?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, port_text) = authority
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("local probe URL must include a port"))?;
    Ok((
        host.to_string(),
        port_text.parse::<u16>().context("parse local probe port")?,
        format!("/{path}"),
    ))
}

fn summarize_attempt(
    attempt: usize,
    runner: &str,
    result: RunnerExecutionResult,
) -> SmokeAttempt {
    let signals = result
        .result_json
        .as_ref()
        .and_then(|value| value.get("validation_signals"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let categories = unique_categories(&signals);
    let has_profile_runtime = signals.iter().any(is_profile_browser_runtime_signal);
    let status = runner_status(result.status).to_string();
    let evidence_status = if result.status_is_succeeded() && has_profile_runtime {
        "passed"
    } else if result.error_message.is_some() || signals.is_empty() {
        "blocked"
    } else {
        "warning"
    }
    .to_string();
    SmokeAttempt {
        attempt,
        runner: runner.to_string(),
        status,
        evidence_status,
        failure_reason: result.error_message,
        signal_count: signals.len(),
        categories,
        signals,
        raw_result: result.result_json,
    }
}

trait RunnerStatusExt {
    fn status_is_succeeded(&self) -> bool;
}

impl RunnerStatusExt for RunnerExecutionResult {
    fn status_is_succeeded(&self) -> bool {
        matches!(self.status, RunnerOutcomeStatus::Succeeded)
    }
}

fn runner_status(status: RunnerOutcomeStatus) -> &'static str {
    match status {
        RunnerOutcomeStatus::Succeeded => "succeeded",
        RunnerOutcomeStatus::Failed => "failed",
        RunnerOutcomeStatus::Cancelled => "cancelled",
        RunnerOutcomeStatus::TimedOut => "timed_out",
    }
}

fn unique_categories(signals: &[Value]) -> Vec<String> {
    let mut categories = signals
        .iter()
        .filter_map(|signal| signal.get("category").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    categories.sort();
    categories.dedup();
    categories
}

fn is_profile_browser_runtime_signal(signal: &Value) -> bool {
    signal
        .get("detail")
        .and_then(Value::as_str)
        .is_some_and(|detail| detail.contains("scope=profile-browser-runtime"))
}

fn overall_status(attempts: &[SmokeAttempt]) -> String {
    if attempts
        .iter()
        .all(|attempt| attempt.evidence_status == "passed")
    {
        "passed".to_string()
    } else if attempts
        .iter()
        .all(|attempt| attempt.evidence_status == "blocked")
    {
        "blocked".to_string()
    } else {
        "warning".to_string()
    }
}

fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn now_ts_string() -> String {
    epoch_millis().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_runtime_signal_requires_runtime_scope_detail() {
        assert!(is_profile_browser_runtime_signal(&json!({
            "detail": "scope=profile-browser-runtime; collector=cdp-runtime-evaluate"
        })));
        assert!(!is_profile_browser_runtime_signal(&json!({
            "detail": "scope=fake-runner; target-profile-browser=false"
        })));
    }

    #[test]
    fn overall_status_keeps_blocked_separate_from_passed() {
        let passed = SmokeAttempt {
            attempt: 1,
            runner: "lightpanda".to_string(),
            status: "succeeded".to_string(),
            evidence_status: "passed".to_string(),
            failure_reason: None,
            signal_count: 1,
            categories: vec!["webrtc".to_string()],
            signals: vec![],
            raw_result: None,
        };
        let blocked = SmokeAttempt {
            evidence_status: "blocked".to_string(),
            ..passed.clone()
        };
        assert_eq!(overall_status(&[passed.clone(), passed]), "passed");
        assert_eq!(overall_status(&[blocked.clone(), blocked]), "blocked");
    }
}
