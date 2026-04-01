use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{bail, Result};
use axum::serve;
use tokio::{net::TcpListener, time::sleep};

use AutoOpenBrowser::{
    network_identity::proxy_selection::proxy_selection_tuning_from_env,
    api::routes::build_router,
    app::state::AppState,
    db::init::init_db,
    queue::memory::MemoryTaskQueue,
    runner::{
        fake::FakeRunner, lightpanda::LightpandaRunner, runner_concurrency_from_env,
        runner_reclaim_seconds_from_env, spawn_runner_workers, RunnerKind, TaskRunner,
    },
    workflow::{run_minimal_cycle_steps, tick_workflow_file, WorkflowExecutionState, DEFAULT_WORKFLOW_STATE_PATH},
};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if let Some(cmd) = args.get(1).map(|s| s.as_str()) {
        match cmd {
            "workflow" => {
                return handle_workflow_cli(&args[2..]).await;
            }
            "--help" | "-h" | "help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
    }

    let database_url = "sqlite://data/auto_open_browser.db";
    let db = init_db(database_url).await?;
    let queue = MemoryTaskQueue::new();
    let api_key = std::env::var("AUTO_OPEN_BROWSER_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let runner: Arc<dyn TaskRunner> = match RunnerKind::from_env() {
        RunnerKind::Fake => Arc::new(FakeRunner),
        RunnerKind::Lightpanda => Arc::new(LightpandaRunner::default()),
    };

    let workflow_state = WorkflowExecutionState::ensure_default_state_file(DEFAULT_WORKFLOW_STATE_PATH, "AutoOpenBrowser")?;
    let worker_count = runner_concurrency_from_env();
    let state = AppState {
        db,
        queue,
        api_key,
        runner: runner.clone(),
        worker_count,
        proxy_selection_tuning: proxy_selection_tuning_from_env(),
    };

    spawn_runner_workers(state.clone(), runner, worker_count).await;

    let app = build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("AutoOpenBrowser listening on http://{}", addr);
    println!("Database initialized at {}", database_url);
    println!("Runner kind: {:?}", RunnerKind::from_env());
    println!("Runner concurrency: {}", worker_count);
    println!("Runner reclaim after: {:?}", runner_reclaim_seconds_from_env());
    println!("Workflow state initialized at {}", DEFAULT_WORKFLOW_STATE_PATH);
    println!("Workflow stage: {:?}", workflow_state.stage);
    serve(listener, app).await?;

    Ok(())
}

async fn handle_workflow_cli(args: &[String]) -> Result<()> {
    match args.first().map(|s| s.as_str()) {
        Some("tick") => {
            let state = tick_workflow_file(DEFAULT_WORKFLOW_STATE_PATH, "AutoOpenBrowser")?;
            println!("workflow tick ok: stage={:?}, iteration={}, focus={}", state.stage, state.loop_iteration, state.current_focus);
        }
        Some("run-steps") => {
            let steps = args.get(1).and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
            let state = run_minimal_cycle_steps(DEFAULT_WORKFLOW_STATE_PATH, "AutoOpenBrowser", steps)?;
            println!("workflow run-steps ok: steps={}, stage={:?}, iteration={}", steps, state.stage, state.loop_iteration);
        }
        Some("daemon") => {
            run_workflow_daemon(&args[1..]).await?;
        }
        Some("show") => {
            let state = WorkflowExecutionState::ensure_default_state_file(DEFAULT_WORKFLOW_STATE_PATH, "AutoOpenBrowser")?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        _ => {
            print_help();
        }
    }
    Ok(())
}

fn print_help() {
    println!("AutoOpenBrowser usage:");
    println!("  AutoOpenBrowser                 Start API server");
    println!("  AutoOpenBrowser workflow show   Show workflow state");
    println!("  AutoOpenBrowser workflow tick   Execute one workflow tick and persist RUN_STATE.json");
    println!("  AutoOpenBrowser workflow run-steps <n>   Execute n workflow steps and persist RUN_STATE.json");
    println!("  AutoOpenBrowser workflow daemon [--interval-seconds N] [--ticks M]   Run periodic workflow ticks");
}

async fn run_workflow_daemon(args: &[String]) -> Result<()> {
    let mut interval_seconds: u64 = 300;
    let mut max_ticks: usize = 0;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--interval-seconds" => {
                let value = args.get(i + 1).ok_or_else(|| anyhow::anyhow!("missing value for --interval-seconds"))?;
                interval_seconds = value.parse::<u64>()?;
                i += 2;
            }
            "--ticks" => {
                let value = args.get(i + 1).ok_or_else(|| anyhow::anyhow!("missing value for --ticks"))?;
                max_ticks = value.parse::<usize>()?;
                i += 2;
            }
            other => bail!("unknown workflow daemon arg: {}", other),
        }
    }

    if interval_seconds == 0 {
        bail!("--interval-seconds must be > 0");
    }

    println!("workflow daemon start: interval={}s, ticks={}", interval_seconds, max_ticks);
    let mut executed = 0usize;
    loop {
        let state = tick_workflow_file(DEFAULT_WORKFLOW_STATE_PATH, "AutoOpenBrowser")?;
        executed += 1;
        println!(
            "workflow daemon tick {} ok: stage={:?}, iteration={}, focus={}",
            executed,
            state.stage,
            state.loop_iteration,
            state.current_focus
        );
        if max_ticks > 0 && executed >= max_ticks {
            println!("workflow daemon completed requested ticks");
            break;
        }
        sleep(Duration::from_secs(interval_seconds)).await;
    }
    Ok(())
}
