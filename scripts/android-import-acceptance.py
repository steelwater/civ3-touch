#!/usr/bin/env python3
"""Actual SAF import/failure/restart tests on an explicitly selected test device.

Copies only profile files from a developer-owned GOG folder. Use a disposable
emulator: this replaces this app's active import and leaves uniquely named test
folders in Documents. Never upload the copied data or screenshots to CI/Drive.
Run after building validate_gog: cargo build --locked --bin validate_gog.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time
import uuid
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser()
parser.add_argument('serial')
parser.add_argument('source', type=Path, help='local GOG installation root (read only)')
parser.add_argument('--validator', default='target/debug/validate_gog')
args = parser.parse_args()
package = 'org.civ3touch.spike'
prefix = 'Civ3Touch-' + uuid.uuid4().hex[:8]
profile = json.loads(subprocess.check_output([args.validator, '--profile'], text=True))


def adb(*command):
    return subprocess.check_output(['adb', '-s', args.serial, *command], text=True)


def tree():
    for _ in range(4):
        try:
            result = adb('shell', 'uiautomator', 'dump', '/sdcard/civ3touch-window.xml')
        except subprocess.CalledProcessError:
            time.sleep(.25)
            continue
        if 'dumped to:' in result:
            return ET.fromstring(adb('shell', 'cat', '/sdcard/civ3touch-window.xml'))
        time.sleep(.25)
    raise AssertionError('UI Automator did not produce a fresh hierarchy')


def node(predicate, scroll=False):
    for _ in range(20):
        for item in tree().iter('node'):
            if predicate(item):
                return item
        if scroll:
            adb("shell", "input", "swipe", "900", "1800", "900", "700", "250")
        time.sleep(.2)
    raise AssertionError('Expected UI state was not found')


def tap(label):
    item = node(lambda n: n.get('text', '').lower() == label.lower() and (n.get('enabled') == 'true' or label == 'Documents'), scroll=label.startswith(prefix))
    left, top, right, bottom = map(int, re.findall(r'\d+', item.get('bounds')))
    adb('shell', 'input', 'tap', str((left + right) // 2), str((top + bottom) // 2))
    time.sleep(.35)


def launch():
    adb('shell', 'am', 'force-stop', package)
    adb('shell', 'am', 'start', '-W', '-f', '0x10008000', '-n', package + '/.MainActivity')
    node(lambda n: n.get('text', '').lower() == 'import civilization iii complete' and n.get('enabled') == 'true')


def choose(variant):
    if not any(n.get('text', '').lower() == 'import civilization iii complete' for n in tree().iter('node')):
        tap('Actions'); tap('Game settings')
    tap('Import Civilization III Complete')
    tap('Documents')
    tap(prefix + '-' + variant)
    tap('Use this folder')
    tap('Allow')


def active():
    return adb('shell', 'run-as', package, 'cat', 'no_backup/imports/active').strip()


# Validate the developer source before touching device storage.
subprocess.run([args.validator, str(args.source)], check=True, stdout=subprocess.DEVNULL)
with tempfile.TemporaryDirectory(prefix='civ3touch-test-') as scratch:
    scratch = Path(scratch)
    for variant in ('valid', 'missing', 'unsupported', 'corrupt'):
        for entry in profile['files']:
            name = entry['path']
            source = args.source / name
            if not source.exists():
                continue
            if variant == 'missing' and entry['kind'] == 'run':
                continue
            if variant == 'unsupported' and entry['kind'] == 'identity':
                source = scratch / 'unsupported-info'
                source.write_text('{"gameId":"unsupported-test-distribution"}')
            if variant == 'corrupt' and name.endswith('xggc.pcx'):
                source = scratch / 'corrupt-image'
                source.write_bytes(b'truncated synthetic image')
            target = '/sdcard/Documents/' + prefix + '-' + variant + '/' + name
            adb('shell', 'mkdir', '-p', str(Path(target).parent))
            subprocess.run(['adb', '-s', args.serial, 'push', str(source), target],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

launch()
choose('valid')
tap('Play')
node(lambda n: n.get('text', '').startswith('Turn 1'))
record = active()
print('PASS: real folder selection -> validate -> import -> Play')
for variant, diagnostic in [('missing', 'Missing required files'),
                            ('unsupported', 'Unsupported installation'),
                            ('corrupt', 'Unsupported terrain PCX')]:
    choose(variant)
    node(lambda n: diagnostic in n.get('text', ''))
    assert active() == record, 'Failed import replaced active data'
    assert 'stage-' not in adb('shell', 'run-as', package, 'ls', 'no_backup/imports')
    tap('New Game')
    node(lambda n: n.get('text', '').startswith('Turn 1'))
    print('PASS:', variant, 'diagnostic; previous import and Play preserved')
launch()
tap('Play')
node(lambda n: n.get('text', '').startswith('Turn 1'))
assert active() == record
print('PASS: process restart restores validated private data')
print('Device-only test folders:', prefix + '-{valid,missing,unsupported,corrupt}')
