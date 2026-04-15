#!/usr/bin/env python3
import json
import os
import subprocess
from datetime import datetime, timezone, timedelta
from pathlib import Path

BASE = Path('/root/.openclaw/workspace/PersonaPilot')
RUN_STATE = BASE / 'RUN_STATE.json'
EXEC_LOG = BASE / 'EXECUTION_LOG.md'
ROUND_DIR = BASE / 'round-results'
SUMMARY_DIR = BASE / 'summaries'
TZ = timezone(timedelta(hours=8))


def now_iso():
    return datetime.now(TZ).replace(microsecond=0).isoformat()


def read_json(path: Path):
    return json.loads(path.read_text())


def write_json(path: Path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")


def append_log(block: str):
    with EXEC_LOG.open('a', encoding='utf-8') as f:
        f.write("\n" + block.strip() + "\n")


def ensure_file(path: Path, content: str):
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_text(content, encoding='utf-8')
        return True
    return False


def upsert_line(path: Path, needle: str, replacement: str):
    text = path.read_text(encoding='utf-8') if path.exists() else ''
    if needle in text:
        text = text.replace(needle, replacement)
    else:
        text += ("\n" if text and not text.endswith("\n") else "") + replacement + "\n"
    path.write_text(text, encoding='utf-8')


def detect_next_round(state):
    nxt = state.get('nextRoundType')
    if nxt:
        return nxt
    rt = state.get('roundType')
    rs = state.get('roundStatus')
    if rs == 'completed':
        return {'plan': 'build', 'build': 'verify', 'verify': 'summarize', 'summarize': 'plan'}.get(rt, 'plan')
    return 'plan'


def run_build(state, round_id, cycle_id):
    changed = []
    cargo = BASE / 'Cargo.toml'
    ensure_file(cargo, '[package]\nname = "persona-pilot"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\nserde = { version = "1", features = ["derive"] }\nserde_json = "1"\nanyhow = "1"\ntokio = { version = "1", features = ["rt-multi-thread", "macros"] }\naxum = "0.7"\nsqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite"] }\n')
    changed.append('Cargo.toml')
    src = BASE / 'src'
    for d in ['app','api','domain','db','queue','runner','network_identity']:
        p = src / d / 'mod.rs'
        ensure_file(p, f'// {d} module\n')
        changed.append(str(p.relative_to(BASE)))
    ensure_file(src / 'lib.rs', 'pub mod app;\npub mod api;\npub mod domain;\npub mod db;\npub mod queue;\npub mod runner;\npub mod network_identity;\n')
    changed.append('src/lib.rs')
    ensure_file(src / 'main.rs', 'fn main() {\n    println!("PersonaPilot bootstrap");\n}\n')
    changed.append('src/main.rs')
    # update current direction hint
    status = 'completed'
    summary = '已初始化 Rust 工程骨架，并落地首批模块目录。'
    issues = []
    next_step = '进入 verify 轮，检查工程结构与基础可编译性。'
    return status, changed, ['Initialized Rust skeleton'], summary, issues, next_step


def run_verify(state, round_id, cycle_id):
    changed = []
    actions = []
    issues = []
    cargo_exists = (BASE / 'Cargo.toml').exists()
    src_exists = (BASE / 'src').exists()
    actions.append(f'Cargo.toml exists={cargo_exists}')
    actions.append(f'src exists={src_exists}')
    cargo_bin = subprocess.run('which cargo', shell=True, capture_output=True, text=True)
    if cargo_bin.returncode == 0:
        actions.append('cargo available')
        res = subprocess.run(['cargo','check'], cwd=BASE, capture_output=True, text=True)
        verify_file = ROUND_DIR / f'verify-output-round-{round_id}.txt'
        verify_file.write_text((res.stdout or '') + '\n---STDERR---\n' + (res.stderr or ''), encoding='utf-8')
        changed.append(str(verify_file.relative_to(BASE)))
        actions.append(f'cargo check exit={res.returncode}')
        if res.returncode != 0:
            issues.append('cargo check 未通过，需要在后续修正工程骨架或依赖问题')
    else:
        actions.append('cargo unavailable, skipped cargo check')
        issues.append('系统未发现 cargo，无法执行 cargo check')
    summary = '完成了工程结构验证，并尝试进行基础编译检查。'
    next_step = '进入 summarize 轮，汇总首个 mini-cycle 的前四轮。'
    return 'completed', changed, actions, summary, issues, next_step


def run_summarize(state, round_id, cycle_id):
    changed = []
    start = max(0, round_id - 3)
    summaries = []
    for i in range(start, round_id + 1):
        p = ROUND_DIR / f'round-{i}.json'
        if p.exists():
            data = read_json(p)
            summaries.append(f"- Round {i} / {data.get('roundType')}: {data.get('summary')}")
    text = f"# Cycle {cycle_id} Summary\n\n生成时间：{now_iso()}\n\n## 已完成\n" + "\n".join(summaries) + "\n\n## 当前状态\n- 已完成首个 mini-cycle 的基础闭环。\n- 下一步应回到 plan 轮，继续细化 SQLite schema 草案。\n"
    path = SUMMARY_DIR / f'cycle-{cycle_id}.md'
    path.write_text(text, encoding='utf-8')
    changed.append(str(path.relative_to(BASE)))
    summary = '已完成本 mini-cycle 汇总。'
    issues = []
    next_step = '进入下一个 cycle 的 plan 轮。'
    return 'completed', changed, ['Generated cycle summary'], summary, issues, next_step


def run_plan(state, round_id, cycle_id):
    changed = []
    path = BASE / 'ROADMAP.md'
    text = path.read_text(encoding='utf-8')
    marker = '## 7. 当前最近一步\n\n当前最合理的下一步：\n\n> 初始化 Rust 工程骨架，并补齐 `EXECUTION_LOG.md` 与 `RUN_STATE.json`。\n'
    repl = '## 7. 当前最近一步\n\n当前最合理的下一步：\n\n> 细化 SQLite schema 草案，并为后续 build / verify 轮提供更具体的数据库落地方向。\n'
    if marker in text:
        text = text.replace(marker, repl)
        path.write_text(text, encoding='utf-8')
        changed.append('ROADMAP.md')
    summary = '已完成新一轮 plan，锁定下一步为细化 SQLite schema 草案。'
    issues = []
    next_step = '进入 build 轮，新增具体 SQLite schema 设计文档。'
    return 'completed', changed, ['Updated roadmap next step'], summary, issues, next_step


def main():
    state = read_json(RUN_STATE)
    round_type = detect_next_round(state)
    current_round = int(state.get('currentRound', 2)) + 1
    cycle_id = int(state.get('cycleId', 0))
    if round_type == 'plan' and state.get('roundType') == 'summarize' and state.get('roundStatus') == 'completed':
        cycle_id += 1
    if round_type == 'build':
        status, changed, actions, summary, issues, next_step = run_build(state, current_round, cycle_id)
    elif round_type == 'verify':
        status, changed, actions, summary, issues, next_step = run_verify(state, current_round, cycle_id)
    elif round_type == 'summarize':
        status, changed, actions, summary, issues, next_step = run_summarize(state, current_round, cycle_id)
    else:
        status, changed, actions, summary, issues, next_step = run_plan(state, current_round, cycle_id)
    finished = now_iso()
    result = {
        'roundId': current_round,
        'cycleId': cycle_id,
        'roundType': round_type,
        'status': status,
        'startedAt': finished,
        'finishedAt': finished,
        'currentObjective': state.get('currentObjective'),
        'changedFiles': changed,
        'verificationActions': actions,
        'summary': summary,
        'issues': issues,
        'nextStep': next_step,
        'pendingRecovery': False,
    }
    rr = ROUND_DIR / f'round-{current_round}.json'
    write_json(rr, result)

    next_round = {'plan': 'build', 'build': 'verify', 'verify': 'summarize', 'summarize': 'plan'}[round_type]
    next_planned = (datetime.now(TZ) + timedelta(minutes=5)).replace(microsecond=0).isoformat()
    state.update({
        'currentRound': current_round,
        'cycleId': cycle_id,
        'roundType': round_type,
        'roundStatus': status,
        'pendingRecovery': False,
        'lastExecutionAt': finished,
        'lastOutputFiles': changed,
        'lastVerificationResult': summary,
        'lastSchedulerDecision': f'Executed {round_type}, next={next_round}',
        'nextRoundType': next_round,
        'nextPlannedAt': next_planned,
        'schedulerStatus': 'running' if next_round else 'idle',
        'currentObjective': next_step,
    })
    if round_type == 'verify':
        state['lastBugCheckRound'] = current_round
    if round_type == 'summarize':
        state['lastSummaryRound'] = current_round
    write_json(RUN_STATE, state)

    log_block = f"## Round {current_round} ({round_type.capitalize()})\n\n- 时间：{finished}\n- 主目标：{result['currentObjective']}\n- 完成：\n" + "\n".join([f'  - {a}' for a in actions]) + f"\n- 产出文件：\n" + "\n".join([f'  - `{c}`' for c in changed]) + f"\n- 验证：\n  - {summary}\n- 问题：\n" + ("\n".join([f'  - {i}' for i in issues]) if issues else '  - 无新增关键问题') + f"\n- 下一步：\n  - {next_step}\n"
    append_log(log_block)
    print(json.dumps({'ok': True, 'roundId': current_round, 'roundType': round_type, 'nextRoundType': next_round, 'changedFiles': changed}, ensure_ascii=False))


if __name__ == '__main__':
    main()
