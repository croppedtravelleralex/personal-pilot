# Resource & Memory Health Checklist

## Goal
Build a low-cost, repeatable checklist to judge whether the system has memory/resource health issues before escalating to heavier profilers.

## Phase 1: Cheap Observation

### 1. Process memory trend
- Record RSS / VSZ before workload
- Run several batches of representative tasks
- Record RSS / VSZ after each batch
- Check whether memory returns toward baseline after idle time

Suggested commands:
- `ps -o pid,rss,vsz,etime,cmd -p <PID>`
- `pmap -x <PID> | tail -n 1`

### 2. Child process hygiene
- Verify lightpanda child processes are cleaned up after success / timeout / failure
- Check for zombie or lingering browser processes

Suggested commands:
- `ps -ef | grep -E "lightpanda|PersonaPilot"`
- `pgrep -af lightpanda`

### 3. File descriptor growth
- Measure fd count before workload and after repeated task execution
- Confirm fd count does not grow monotonically

Suggested commands:
- `ls /proc/<PID>/fd | wc -l`
- `lsof -p <PID> | wc -l`

### 4. Database size growth
- Record SQLite file size before/after workload
- Inspect whether task / run / proxy-log growth matches expectations

Suggested commands:
- `du -h data/persona_pilot.db`
- `sqlite3 data/persona_pilot.db ".tables"`
- `sqlite3 data/persona_pilot.db "SELECT COUNT(*) FROM tasks;"`

## Phase 2: Targeted Risk Paths

### Highest-risk paths
1. `lightpanda runner`
2. `memory queue`
3. `reclaim / retry / timeout`
4. `running_tasks` shared state
5. proxy verification / batch verification persistence

### What to look for
- state not cleared after terminal completion
- lingering child process after timeout
- queue entry retained after execution
- lock-protected state growing unexpectedly
- task/runs/proxy logs growing faster than workload justifies

## Phase 3: Escalation Criteria
Only escalate to heavier profiling if one of these happens:
- RSS keeps climbing after several workload cycles and does not settle
- fd count grows monotonically
- child processes linger after timeout / failure
- DB grows abnormally fast without matching workload volume
- retry/reclaim paths leave visible state residue

## Suggested Deliverables
1. Observation command sheet
2. Risk-point checklist
3. Pass/fail criteria for memory/resource health

## Pass/Fail Heuristic
Pass if:
- memory stabilizes after workload
- no zombie/lingering child processes remain
- fd count roughly returns to steady state
- database growth is proportional to workload
- reclaim/retry/timeout paths clear state as expected

Fail if:
- memory rises steadily across repeated runs with no settle-back
- child processes accumulate
- fd count keeps increasing
- DB/log growth is disproportionate
- state persists after terminal cleanup
