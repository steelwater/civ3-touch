#!/usr/bin/env python3
"""Inspect synthetic touch screens across phone/tablet, landscape and large text.

Requires a synthetic manual save with a city (created by android-touch-acceptance).
Restores all changed display settings. Never run with --imported: evidence is public.
"""
import importlib.util
from pathlib import Path
import re
import os
import subprocess
import time

spec = importlib.util.spec_from_file_location('loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)
assert not loop.args.imported, 'Layout evidence must use synthetic assets'
output = Path('.local/m4-evidence'); output.mkdir(parents=True, exist_ok=True)


def setting(key):
    return loop.adb('shell', 'settings', 'get', 'system', key).strip()


def capture(name):
    time.sleep(.4)
    with (output / (name + '.png')).open('wb') as image:
        subprocess.run(['adb', '-s', loop.args.serial, 'exec-out', 'screencap', '-p'], stdout=image, check=True)


def map_box():
    return loop.bounds(next(n for n in loop.tree().iter('node') if n.get('content-desc', '').startswith('Generated map.')))


settings = {k: setting(k) for k in ['font_scale', 'user_rotation', 'accelerometer_rotation']}
size = re.search(r'Override size: (\d+x\d+)', loop.adb('shell', 'wm', 'size'))
density = re.search(r'Override density: (\d+)', loop.adb('shell', 'wm', 'density'))
loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
loop.menu('Load saved game'); loop.find('Saved game loaded.', True)
expected = loop.state()['view']
assert expected['own_cities']
try:
    loop.adb('shell', 'settings', 'put', 'system', 'accelerometer_rotation', '0')
    for name, dimensions, dpi, rotation, font in [
            ('phone', '1080x2400', '420', '0', '1.0'),
            ('phone-landscape-large', '1080x2400', '420', '1', '1.5'),
            ('tablet-large', '1600x2560', '240', '0', '1.5'),
            ('tablet-landscape-large', '1600x2560', '240', '1', '1.5')]:
        if os.environ.get('CIV3TOUCH_LAYOUT_CASE', name) != name:
            continue
        loop.adb('shell', 'wm', 'size', dimensions); loop.adb('shell', 'wm', 'density', dpi)
        loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', rotation)
        loop.adb('shell', 'settings', 'put', 'system', 'font_scale', font); time.sleep(2)
        loop.selected = None; loop.choose(loop.unit('worker'))
        l,t,r,b = map_box(); assert b-t >= 64*int(dpi)/160, (name, 'map too small', b-t)
        capture(name + '-map')
        if name == 'phone-landscape-large':
            loop.tap('Turn ' + str(expected['turn']), True)
            capture(name + '-information')
            l,t,r,b = map_box(); assert b-t >= 64*int(dpi)/160, (name, 'expanded information hides map', b-t)
            print('Expanded information map height:', b-t, flush=True)
            loop.tap('Turn ' + str(expected['turn']), True)
        loop.menu('Cities and production'); loop.tap(expected['own_cities'][0]['name'] + ' • population', True)
        loop.find('Choose production (changing item resets shields)')
        capture(name + '-city')
        loop.adb('shell', 'input', 'keyevent', 'KEYCODE_BACK')
        map_box(); assert loop.state()['view'] == expected
        # Rotation of an open screen must return to the retained map without leaking a window.
        loop.menu('Diplomacy'); loop.find('No other civilization', True)
        capture(name + '-diplomacy')
        loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', '1' if rotation == '0' else '0')
        time.sleep(2); map_box(); assert loop.state()['view'] == expected
        print('PASS:', name, 'map, city, diplomacy, Back and screen rotation', flush=True)
finally:
    loop.adb('shell', 'wm', 'size', size.group(1) if size else 'reset')
    loop.adb('shell', 'wm', 'density', density.group(1) if density else 'reset')
    for key, value in settings.items():
        if value == 'null': loop.adb('shell', 'settings', 'delete', 'system', key)
        else: loop.adb('shell', 'settings', 'put', 'system', key, value)
