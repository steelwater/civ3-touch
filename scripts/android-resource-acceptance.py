#!/usr/bin/env python3
"""Verify M5 resource presentation through real touches on a disposable emulator.

Uses the existing synthetic launch and separate synthetic saves. No state injection.
Run after other device journeys, never concurrently against the same device.
"""
import importlib.util
from pathlib import Path
import re
import subprocess
import time

spec = importlib.util.spec_from_file_location('loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)
assert not loop.args.imported, 'Resource screenshot evidence must use synthetic assets'

loop.adb('shell', 'am', 'force-stop', loop.PACKAGE)
loop.launch()
loop.tap('New Game')
time.sleep(1)
before = loop.state()
assert before['ruleset'] == 'freec3-90fc7ee-civ3touch-m5-resources-v1'
resource = next(t for t in before['view']['visible_tiles'] if t.get('resource'))
assert resource['resource'] in ['Wheat', 'Cattle', 'Gold']
node = next(n for n in loop.tree().iter('node') if n.get('content-desc', '').startswith('Generated map.'))
l, t, r, b = loop.bounds(node)
cx, cy, zoom = map(float, re.search(r'View center (\d+\.\d+), (\d+\.\d+); zoom (\d+\.\d+)', node.get('content-desc')).groups())
density = int(re.findall(r'density: (\d+)', loop.adb('shell', 'wm', 'density'))[-1]) / 160
dx, dy = resource['coord']['x'] - cx, resource['coord']['y'] - cy
x = round((l + r) / 2 + (dx - dy) * 48 * density * zoom)
y = round((t + b) / 2 + (dx + dy) * 28 * density * zoom)
assert l <= x < r and t <= y < b
loop.adb('shell', 'input', 'swipe', str(x), str(y), str(x), str(y), '900')
assert any('Resource: ' + resource['resource'] in n.get('text', '') for n in loop.tree().iter('node'))
assert loop.state() == before, 'Inspecting resources must not submit a game command'
output = Path('.local/milestone-5/resource-inspector.png')
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('wb') as image:
    subprocess.run(['adb', '-s', loop.args.serial, 'exec-out', 'screencap', '-p'], stdout=image, check=True)
loop.adb('shell', 'input', 'keyevent', 'KEYCODE_BACK')
assert loop.state() == before
print('PASS: M5 resource name reaches tile inspector through long press; inspection and Back preserve state')
