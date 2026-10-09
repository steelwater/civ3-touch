#!/usr/bin/env python3
import hashlib
import json
import subprocess
from pathlib import Path

record = json.loads(Path('docs/freec3-baseline.json').read_text())
patches = json.loads(Path('docs/freec3-resource-patches.json').read_text())
if patches['baseline_revision'] != record['revision']:
    raise SystemExit('Patch manifest targets a different upstream revision')
expected_files = dict(record['files'])
for name, patch in patches['files'].items():
    if patch['original_sha256'] != record['files'].get(name):
        raise SystemExit('Patch original hash differs from preserved baseline: ' + name)
    expected_files[name] = patch['sha256']
for name, expected in expected_files.items():
    actual = hashlib.sha256((Path('vendor/freec3') / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit('Unexpected vendor content: ' + name)
proposed = subprocess.check_output([
    'git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', 'vendor/freec3'
]).decode().split('\0')
for name in filter(None, proposed):
    relative = str(Path(name).relative_to('vendor/freec3'))
    if relative not in expected_files:
        raise SystemExit('Unrecorded vendor addition: ' + relative)
print(f"PASS: {len(record['files'])} baseline files and {len(patches['files'])} recorded resource patches verified")
