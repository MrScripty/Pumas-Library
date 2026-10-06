"""Read-volume evidence from actual kernel traces, only fixture input paths."""
import json
from pathlib import Path
import re
import sys

log, trace, output = map(Path, sys.argv[1:])
targets = {}
for line in log.open():
    match = re.search(r'post-checker projection fixture: file=(\S+) grow=(true|false)', line)
    if match:
        targets[match[1]] = match[2] == 'true'
counts = dict.fromkeys(targets, 0)
largest = dict.fromkeys(targets, 0)
pending = {}
focused = []
for line in trace.open():
    match = re.match(r'(\d+)\s+read\(\d+<([^>]+)>,', line)
    if match:
        pid, path = match.groups()
        result = re.search(r'\)\s+=\s+(\d+)', line)
        if path in targets:
            focused.append(line)
        if result:
            if path in targets:
                amount = int(result[1])
                counts[path] += amount
                largest[path] = max(largest[path], amount)
        elif '<unfinished ...>' in line:
            pending[pid] = path
    elif (match := re.match(r'(\d+)\s+<\.\.\. read resumed>.*\)\s+=\s+(\d+)', line)):
        pid, result = match.groups()
        path = pending.pop(pid, None)
        if path in targets:
            focused.append(line)
            amount = int(result)
            counts[path] += amount
            largest[path] = max(largest[path], amount)
rows = [{'file': Path(path).name, 'fixture_path': path, 'growth': growth,
         'total_data_read_bytes': counts[path], 'largest_read_result': largest[path]}
        for path, growth in targets.items()]
output.write_text(json.dumps({'cases': rows, 'scope': 'All observed processes, matching fixture path only; includes small reads before mutation',
                             'sparse_growth_bytes': 32 * 1024 * 1024}, indent=2) + '\n')
output.with_suffix('.focused.trace').write_text(''.join(focused))
print(json.dumps(rows, indent=2))
