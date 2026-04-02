#!/usr/bin/env python3
from pathlib import Path
from collections import Counter
import re, sys, json

branch_re = re.compile(r'perf_probe event=refresh_proxy_trust_views_for_scope branch=([^\s]+)')
event_re = re.compile(r'perf_probe event=([^\s]+)')

counter = Counter()
event_counter = Counter()
files = []
for arg in sys.argv[1:]:
    p = Path(arg)
    if not p.exists():
        continue
    files.append(str(p))
    for line in p.read_text().splitlines():
        m = branch_re.search(line)
        if m:
            counter[m.group(1)] += 1
        m2 = event_re.search(line)
        if m2:
            event_counter[m2.group(1)] += 1

print(json.dumps({
    'files': files,
    'branch_counts': dict(counter),
    'event_counts': dict(event_counter),
}, ensure_ascii=False, indent=2))
