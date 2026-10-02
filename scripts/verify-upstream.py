#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path

record = json.loads(Path('docs/freec3-baseline.json').read_text())
for name, expected in record['files'].items():
    actual = hashlib.sha256((Path('vendor/freec3') / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit('Modified upstream baseline: ' + name)
print('PASS: all 115 vendored source files match pinned FreeC3 revision ' + record['revision'])
