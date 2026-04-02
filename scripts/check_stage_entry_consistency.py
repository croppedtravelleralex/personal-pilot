#!/usr/bin/env python3
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parents[1]
files = {
    'README': root / 'README.md',
    'AI': root / 'AI.md',
    'STATUS': root / 'STATUS.md',
    'TODO': root / 'TODO.md',
    'PROGRESS': root / 'PROGRESS.md',
}

for name, path in files.items():
    if not path.exists():
        print(f'[FAIL] missing required file: {path}')
        sys.exit(1)

readme = files['README'].read_text()
ai = files['AI'].read_text()
status = files['STATUS'].read_text()
todo = files['TODO'].read_text()
progress = files['PROGRESS'].read_text()

checks = []

def ok(cond, msg):
    checks.append((cond, msg))

ok('## Current Stage Snapshot' in readme, 'README contains Current Stage Snapshot')
ok('Stage status:' in readme, 'README snapshot contains stage status')
ok('## 2.1 当前阶段快照（Current Stage Snapshot）' in ai, 'AI.md contains Current Stage Snapshot')
ok('Frozen in current stage:' in readme, 'README snapshot contains frozen lines')
ok('Frozen in current stage:' in ai, 'AI snapshot contains frozen lines')
ok('Reopen rule:' in readme, 'README snapshot contains reopen rule')
ok('Reopen rule:' in ai, 'AI snapshot contains reopen rule')
ok('refresh-scope 不再继续扩实现' in status or 'refresh-scope work is closed' in readme, 'refresh-scope closure is reflected in entry surfaces')
ok('providerRegion' in readme and 'providerRegion' in ai and 'providerRegion' in status, 'providerRegion status appears in README, AI, and STATUS')
ok('selection redesign' in status or 'selection intentionally unchanged' in readme, 'selection boundary appears in entry surfaces')
ok('后续若入口摘要新增内容，先做一致性检查再更新' in todo or '按 checklist' in todo, 'TODO preserves entry-summary update discipline')
ok('阶段入口一致性检查' in progress or 'entry summary update checklist' in progress, 'PROGRESS records control-surface maintenance work')

failed = False
for cond, msg in checks:
    prefix = '[OK]' if cond else '[FAIL]'
    print(f'{prefix} {msg}')
    if not cond:
        failed = True

if failed:
    sys.exit(1)

print('\nStage entry consistency: PASS')
