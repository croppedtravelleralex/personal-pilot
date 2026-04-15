#!/usr/bin/env python3
import json
import subprocess
import time
from datetime import datetime, timezone, timedelta
from pathlib import Path

BASE = Path('/root/.openclaw/workspace/PersonaPilot')
RUN_STATE = BASE / 'RUN_STATE.json'
LOG = BASE / 'scheduler-daemon.log'
TZ = timezone(timedelta(hours=8))

END_AFTER_SECONDS = 8 * 60 * 60
INTERVAL_SECONDS = 5 * 60


def now():
    return datetime.now(TZ)


def log(msg):
    with LOG.open('a', encoding='utf-8') as f:
        f.write(f"[{now().isoformat()}] {msg}\n")


def read_state():
    return json.loads(RUN_STATE.read_text())


def main():
    start = time.time()
    log('scheduler daemon started')
    while True:
        elapsed = time.time() - start
        if elapsed >= END_AFTER_SECONDS:
            log('scheduler daemon finished: reached 8h limit')
            break
        try:
            state = read_state()
            planned = state.get('nextPlannedAt')
            if planned:
                try:
                    target = datetime.fromisoformat(planned)
                    delay = (target - now()).total_seconds()
                    if delay > 0:
                        sleep_for = min(delay, 30)
                        time.sleep(sleep_for)
                        continue
                except Exception as e:
                    log(f'failed to parse nextPlannedAt: {e}')
            res = subprocess.run(
                ['python3', str(BASE / 'scripts' / 'run_round.py')],
                cwd=str(BASE.parent),
                capture_output=True,
                text=True,
            )
            log(f'run_round exit={res.returncode} stdout={res.stdout.strip()} stderr={res.stderr.strip()}')
            time.sleep(5)
        except Exception as e:
            log(f'daemon loop error: {e}')
            time.sleep(15)


if __name__ == '__main__':
    main()
