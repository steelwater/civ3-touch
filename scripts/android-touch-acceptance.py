#!/usr/bin/env python3
"""M4 real gestures and touch screens; read snapshots only, no simulation injection.

Build tests/touch/Pinch.java with the installed SDK, dex it with d8, and push the
result to /data/local/tmp/civ3touch-pinch.dex before running (see testing.md).
Uses the loop harness's serial and --imported options and disposable-save warning.
"""
import importlib.util
from pathlib import Path
import re
import subprocess
import time

spec = importlib.util.spec_from_file_location('loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)


def map_node():
    return next(n for n in loop.tree().iter('node') if n.get('content-desc', '').startswith('Generated map.'))


def capture(name):
    if loop.args.imported:
        return  # Original artwork must never enter review evidence.
    output = Path('.local/m4-evidence')
    output.mkdir(parents=True, exist_ok=True)
    with (output / (name + '.png')).open('wb') as image:
        subprocess.run(['adb', '-s', loop.args.serial, 'exec-out', 'screencap', '-p'], stdout=image, check=True)


def camera():
    return tuple(map(float, re.search(r'View center (\d+\.\d+), (\d+\.\d+); zoom (\d+\.\d+)', map_node().get('content-desc')).groups()))


def center():
    l, t, r, b = loop.bounds(map_node())
    return (l+r)//2, (t+b)//2


def gesture(x1, y1, x2, y2, duration=400):
    loop.adb('shell', 'input', 'swipe', str(x1), str(y1), str(x2), str(y2), str(duration))


def unchanged(before):
    assert loop.state() == before, 'Map navigation must never submit a command'


loop.adb('shell', 'am', 'force-stop', loop.PACKAGE)
loop.launch(); loop.tap('Play' if loop.args.imported else 'New Game'); time.sleep(1)
# Select with an actual tap and exercise the real engine-populated bottom sheet.
x,y = center(); loop.adb('shell', 'input', 'tap', str(x), str(y))
loop.find('Build City')
capture('unit-sheet')
before = loop.state(); initial_camera = camera()
x,y = center(); gesture(x,y,x+160,y+80)
assert camera() != initial_camera
unchanged(before)
# A second drag remains navigation without a queued movement command.
x,y = center(); gesture(x,y,x+160,y)
unchanged(before)
loop.tap('Center'); assert camera() == initial_camera
x,y = center()
loop.adb('shell', 'CLASSPATH=/data/local/tmp/civ3touch-pinch.dex', 'app_process', '/system/bin', 'Pinch', str(x), str(y), '160', '360')
assert camera()[2] > initial_camera[2]
unchanged(before)
loop.adb('shell', 'CLASSPATH=/data/local/tmp/civ3touch-pinch.dex', 'app_process', '/system/bin', 'Pinch', str(x), str(y), '360', '160')
assert camera()[2] < 2
unchanged(before)
print('PASS: drag and real two-pointer pinch change only the viewport', flush=True)
loop.tap('Center'); x,y = center(); gesture(x,y,x,y,900)
loop.find('Select settler', True)
unchanged(before)
loop.tap('Select worker', True)
assert 'Generated map. worker' in map_node().get('content-desc')
unchanged(before)
x,y = center(); gesture(x,y,x,y,900)
loop.tap('Select settler', True)
assert 'Generated map. settler' in map_node().get('content-desc')
unchanged(before)
loop.tap('settler • Hide actions'); assert not any(n.get('text') == 'Build City' for n in loop.tree().iter('node'))
loop.tap('settler • Show actions'); loop.find('Build City')
loop.tap('Turn 1', True); loop.find('Turn 1 · Unit selected', True); loop.tap('Turn 1', True)
unchanged(before)
print('PASS: long press inspects without a command; information and unit sheet collapse/expand', flush=True)
# Snapshot preserves game state across configuration change; the retained camera survives too.
x,y = center(); gesture(x,y,x+90,y+60)
pose = camera()
old_rotation = loop.adb('shell', 'settings', 'get', 'system', 'user_rotation').strip()
old_auto = loop.adb('shell', 'settings', 'get', 'system', 'accelerometer_rotation').strip()
loop.adb('shell', 'settings', 'put', 'system', 'accelerometer_rotation', '0')
try:
    loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', '1'); time.sleep(2)
    assert camera() == pose; unchanged(before)
finally:
    loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', old_rotation)
    loop.adb('shell', 'settings', 'put', 'system', 'accelerometer_rotation', old_auto)
time.sleep(2)
loop.tap('Center')
settler = loop.unit('settler')
destination = next(m['destinations'][0] for m in before['moves'] if m['unit_id'] == settler['id'])
loop.move(settler, destination); before = loop.end()
print('PASS: tap projection still moves to the correct legal tile after pan/pinch/rotation', flush=True)
loop.tap('Build City'); founded = loop.wait(before['_sequence']); assert founded['view']['own_cities']
loop.city_production('warrior'); loop.city_production('worker', queue=True)
loop.menu('Cities and production'); loop.tap(founded['view']['own_cities'][0]['name'] + ' • population', True)
loop.find('Choose production (changing item resets shields)')
capture('city')
before_clear = loop.state()
loop.tap('Clear pending queue'); cleared = loop.wait(before_clear['_sequence'])
assert not any(items for _, items in cleared['queues'])
print('PASS: touch city production selection, queue addition and queue clearing', flush=True)
loop.menu('Diplomacy'); loop.find('No other civilization', True); capture('diplomacy'); loop.tap('Return to map')
loop.save_and_reload()
print('PASS: rotation, sheet city founding, city production/queue, empty diplomacy and replay lifecycle', flush=True)
