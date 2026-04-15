#!/usr/bin/env python3
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
files = {
    "README": root / "README.md",
    "AI": root / "AI.md",
    "STATUS": root / "STATUS.md",
    "CURRENT_TASK": root / "CURRENT_TASK.md",
    "PLAN": root / "PLAN.md",
    "TODO": root / "TODO.md",
    "progress": root / "progress.md",
    "PROGRESS": root / "PROGRESS.md",
    "RUNBOOK": root / "docs" / "agent-alexstudio-gateway-runbook.md",
    "RELEASE_SCRIPT": root / "scripts" / "release_baseline_verify.sh",
    "FAST_RELEASE_SCRIPT": root / "scripts" / "release_fast_verify.sh",
    "PREFLIGHT_SCRIPT": root / "scripts" / "preflight_release_env.sh",
    "GATEWAY_SCRIPT": root / "scripts" / "gateway_verify.sh",
}

for name, path in files.items():
    if not path.exists():
        print(f"[FAIL] missing required file: {path}")
        sys.exit(1)

texts = {
    name: path.read_text(encoding="utf-8")
    for name, path in files.items()
    if name not in {"RELEASE_SCRIPT", "FAST_RELEASE_SCRIPT", "PREFLIGHT_SCRIPT", "GATEWAY_SCRIPT"}
}
checks = []


def ok(cond: bool, msg: str) -> None:
    checks.append((cond, msg))


def head(text: str, lines: int = 120) -> str:
    return "\n".join(text.splitlines()[:lines])


snapshot_readme = head(texts["README"])
snapshot_ai = head(texts["AI"])
snapshot_status = head(texts["STATUS"])
snapshot_current = head(texts["CURRENT_TASK"])
snapshot_plan = head(texts["PLAN"])
snapshot_progress = head(texts["progress"])

ok("## Current Stage Snapshot" in texts["README"], "README contains Current Stage Snapshot")
ok("93%" in snapshot_readme and "lightpanda serve + CDP" in snapshot_readme, "README snapshot records 93% serve+CDP baseline")
ok("release_baseline_verify.sh" in snapshot_readme, "README snapshot includes strict release verify entry")
ok("release_fast_verify.sh" in snapshot_readme, "README snapshot includes fast verify entry")
ok("preflight_release_env.sh" in snapshot_readme, "README snapshot includes preflight entry")
ok("93%" in snapshot_ai and "serve + CDP" in snapshot_ai, "AI snapshot records 93% serve+CDP baseline")
ok("127.0.0.1:8787" in snapshot_status and "127.0.0.1:3000" in snapshot_status, "STATUS records both gateway and control-plane ports")
ok("/usr/local/bin/lightpanda" in snapshot_status, "STATUS records real Lightpanda binary path")
ok("no-token" in snapshot_status and "real-upstream" in snapshot_status, "STATUS distinguishes gateway no-token and real-upstream acceptance")
ok("preflight_release_env.sh" in texts["STATUS"], "STATUS includes preflight script entry")
ok("release_fast_verify.sh" in texts["STATUS"], "STATUS includes fast verify script entry")
ok("src/runner/fake.rs" in snapshot_current and "fake/stub/test only" in snapshot_current, "CURRENT_TASK records fake runner boundary closeout")
ok("88%" in snapshot_plan and "93%" in snapshot_plan, "PLAN records 88% -> 93% closeout")
ok("src/runner/fake.rs" in texts["TODO"], "TODO keeps fake.rs trace item")
ok("cargo test -q" in snapshot_progress and "93%" in snapshot_progress, "progress.md records green 93% baseline")
ok("release_baseline_verify.sh" in texts["progress"], "progress.md records strict baseline script")
ok("preflight_release_env.sh" in texts["progress"], "progress.md records preflight script")
ok("127.0.0.1:8787" in texts["PROGRESS"] and "/usr/local/bin/lightpanda" in texts["PROGRESS"], "PROGRESS records runtime alignment facts")
ok("release_fast_verify.sh" in texts["RUNBOOK"] and "preflight_release_env.sh" in texts["RUNBOOK"], "RUNBOOK includes preflight + fast verify commands")
ok("no-token" in texts["RUNBOOK"] and "real-upstream" in texts["RUNBOOK"], "RUNBOOK distinguishes no-token and real-upstream validation")

banned = ["fetch-based mainline", "lightpanda fetch"]
for name, snapshot in [
    ("README", snapshot_readme),
    ("AI", snapshot_ai),
    ("STATUS", snapshot_status),
    ("CURRENT_TASK", snapshot_current),
    ("PLAN", snapshot_plan),
    ("progress", snapshot_progress),
]:
    ok(all(term not in snapshot for term in banned), f"{name} snapshot no longer treats fetch-based as current mainline")

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
