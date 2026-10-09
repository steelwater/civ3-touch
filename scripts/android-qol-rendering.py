#!/usr/bin/env python3
"""Observe imported animation scheduling on a dedicated emulator, without screenshots.

Run after the existing artwork import acceptance with SERIAL --imported.
Counts app-rendered frames; this is not a physical-device energy benchmark.
"""
import importlib.util
from pathlib import Path
import re
import time

spec = importlib.util.spec_from_file_location('loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec); spec.loader.exec_module(loop)
assert loop.args.imported, 'Use imported animation; synthetic markers do not animate'


def frames():
    report = loop.adb('shell', 'dumpsys', 'gfxinfo', loop.PACKAGE)
    return int(re.search(r'Total frames rendered: (\d+)', report).group(1))


loop.adb('shell', 'am', 'force-stop', loop.PACKAGE)
loop.launch(); loop.tap('Play'); loop.find('settler • Show actions')
before = loop.state()
time.sleep(1); start = frames(); time.sleep(3); foreground = frames() - start
assert foreground > 0, 'Imported map did not animate'
loop.tap('Actions'); loop.find('Game settings'); time.sleep(1)
start = frames(); time.sleep(3); covered = frames() - start
assert covered == 0, ('Covered map kept drawing', covered)
loop.tap('Return to map'); time.sleep(1)
loop.adb('shell', 'input', 'keyevent', 'KEYCODE_HOME'); time.sleep(1)
start = frames(); time.sleep(3); background = frames() - start
assert background == 0, ('Background map kept drawing', background)
started = time.monotonic(); loop.launch(); resumed_in = time.monotonic() - started
time.sleep(1); start = frames(); time.sleep(3); resumed = frames() - start
assert resumed > 0 and loop.state() == before
print(f'PASS: 3-second frame deltas foreground={foreground}, menu={covered}, background={background}, resumed={resumed}; launch command returned in {resumed_in:.2f}s; native state unchanged')
print('Emulator scheduling observation only; no before/after hardware energy or frame-pacing claim.')
