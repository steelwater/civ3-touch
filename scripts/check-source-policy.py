#!/usr/bin/env python3
"""Check proposed repository files, including untracked non-ignored files."""
import subprocess
from pathlib import Path

paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z']).decode().split('\0')
forbidden = {'.exe', '.dll', '.pcx', '.flc', '.biq', '.bic', '.bix', '.sav', '.wav', '.mp3', '.ogg', '.amb', '.ttf', '.apk', '.aab', '.keystore', '.jks'}
errors = []
for name in filter(None, paths):
    path = Path(name)
    if path.parts[0] in ('installer', 'local-data', 'extracted', '.local', 'target') or path.suffix.lower() in forbidden or path.name.startswith('.env') or path.name == 'local.properties':
        errors.append(name)
if errors:
    raise SystemExit('Disallowed repository inputs:\n' + '\n'.join(errors))
print('PASS: no known proprietary payload, local-tool, secret-file, or build-output paths in proposed source set')
