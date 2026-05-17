#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys


root = Path(__file__).resolve().parents[1]
files = {
    "README": root / "README.md",
    "AI": root / "AI.md",
    "STATUS": root / "STATUS.md",
    "PLAN": root / "PLAN.md",
    "ROADMAP": root / "ROADMAP.md",
    "PROGRESS": root / "PROGRESS.md",
    "TODO": root / "TODO.md",
    "DOCS_README": root / "docs" / "README.md",
    "CURRENT_STATE": root / "docs" / "02-current-state.md",
    "ROADMAP_DOC": root / "docs" / "03-roadmap.md",
    "BACKLOG_DOC": root / "docs" / "04-improvement-backlog.md",
    "PLAYBOOK_DOC": root / "docs" / "05-ai-maintenance-playbook.md",
    "FINAL_PROGRESS_DOC": root / "docs" / "final-goal-progress-breakdown.md",
    "RUNBOOK_DOC": root / "docs" / "agent-alexstudio-gateway-runbook.md",
    "ROOT_MAP_DOC": root / "docs" / "root-entrypoint-map.md",
}

for name, path in files.items():
    if not path.exists():
        print(f"[FAIL] missing required file: {path}")
        sys.exit(1)

texts = {name: path.read_text(encoding="utf-8") for name, path in files.items()}
checks = []


def ok(cond: bool, msg: str) -> None:
    checks.append((cond, msg))


def git_tracked_progress_entries() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "--stage"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    entries = []
    for line in result.stdout.splitlines():
        if not line:
            continue
        path = line.split("\t", 1)[1]
        if path.lower() == "progress.md":
            entries.append(path)
    return entries


root_entrypoints = {
    "README": texts["README"],
    "AI": texts["AI"],
    "STATUS": texts["STATUS"],
    "PLAN": texts["PLAN"],
    "ROADMAP": texts["ROADMAP"],
    "PROGRESS": texts["PROGRESS"],
}

ok("/docs/README.md" in texts["README"], "README routes to docs/README.md")
ok("/docs/root-entrypoint-map.md" in texts["README"], "README routes to docs/root-entrypoint-map.md")
ok("/docs/02-current-state.md" in texts["STATUS"], "STATUS routes to docs/02-current-state.md")
ok("upstream_configured=false" in texts["STATUS"], "STATUS keeps the gateway upstream guardrail")
ok("/docs/05-ai-maintenance-playbook.md" in texts["AI"], "AI routes to docs/05-ai-maintenance-playbook.md")
ok("/docs/03-roadmap.md" in texts["PLAN"] and "/docs/04-improvement-backlog.md" in texts["PLAN"], "PLAN routes to roadmap and backlog docs")
ok("/docs/03-roadmap.md" in texts["ROADMAP"], "ROADMAP routes to docs/03-roadmap.md")
ok("/docs/02-current-state.md" in texts["PROGRESS"], "PROGRESS routes to docs/02-current-state.md")
ok("/docs/final-goal-progress-breakdown.md" in texts["PROGRESS"], "PROGRESS routes to docs/final-goal-progress-breakdown.md")
ok("/PROGRESS.md" in texts["ROOT_MAP_DOC"], "root entrypoint map documents PROGRESS.md")
ok("/README.md" in texts["ROOT_MAP_DOC"] and "/STATUS.md" in texts["ROOT_MAP_DOC"], "root entrypoint map lists root compatibility files")
ok("Runtime Alive" in texts["CURRENT_STATE"], "02-current-state keeps the runtime section")
ok("Build Status" in texts["CURRENT_STATE"], "02-current-state keeps the build section")
ok("Reporting Rule From Now On" in texts["CURRENT_STATE"], "02-current-state keeps the reporting rule section")
ok("Final Goal Progress Breakdown" in texts["FINAL_PROGRESS_DOC"], "final progress breakdown doc is present")
ok("upstream_configured=false" in texts["RUNBOOK_DOC"], "gateway runbook keeps the shell-vs-upstream guardrail")
ok("real-upstream acceptance blocked by current runtime state" in texts["RUNBOOK_DOC"], "gateway runbook keeps the real-upstream blocker wording")

banned = [
    "Stage-closeout baseline",
    "**93%**",
    "cargo test -q",
    "real-upstream already closed",
]
for name, text in root_entrypoints.items():
    ok(all(term not in text for term in banned), f"{name} entrypoint no longer carries stale stage-closeout wording")

progress_entries = git_tracked_progress_entries()
ok(progress_entries == ["PROGRESS.md"], "git tracks only PROGRESS.md as the root progress entrypoint")

failed = False
for cond, msg in checks:
    prefix = "[OK]" if cond else "[FAIL]"
    print(f"{prefix} {msg}")
    if not cond:
        failed = True

if failed:
    sys.exit(1)

print()
print("Stage entry consistency: PASS")
