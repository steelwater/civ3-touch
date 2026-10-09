#!/usr/bin/env python3
"""M7 synthetic display/gesture checks; restores device settings and app display defaults.

Use the disposable emulator after android-qol-acceptance (a city-bearing recovery save).
All screenshots use synthetic art. This is emulator evidence, not hardware accessibility certification.
"""
import importlib.util
from pathlib import Path
import re
import os
import subprocess
import time

spec = importlib.util.spec_from_file_location('qol', Path(__file__).with_name('android-qol-acceptance.py'))
qol = importlib.util.module_from_spec(spec); spec.loader.exec_module(qol)
loop = qol.loop
output = Path('.local/m7-evidence'); output.mkdir(parents=True, exist_ok=True)


def setting(key): return loop.adb('shell', 'settings', 'get', 'system', key).strip()


def map_node():
    return next(n for n in loop.tree().iter('node') if n.get('content-desc', '').startswith('Generated map.'))


def capture(name):
    with (output / (name + '.png')).open('wb') as image:
        subprocess.run(['adb', '-s', loop.args.serial, 'exec-out', 'screencap', '-p'], stdout=image, check=True)


def main():
    settings = {k: setting(k) for k in ['font_scale', 'user_rotation', 'accelerometer_rotation']}
    size = re.search(r'Override size: (\d+x\d+)', loop.adb('shell', 'wm', 'size'))
    density = re.search(r'Override density: (\d+)', loop.adb('shell', 'wm', 'density'))
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    qol.menu('Resume recovery save'); loop.find('Saved game loaded.', True)
    expected = loop.state()
    try:
        loop.adb('shell', 'settings', 'put', 'system', 'accelerometer_rotation', '0')
        for name, dimensions, dpi, rotation, theme, scale, font in [
                ('phone-light', '1080x2400', '420', '0', 'light', 100, 100),
                ('phone-dark-max', '1080x2400', '420', '0', 'dark', 130, 150),
                ('phone-landscape-max', '1080x2400', '420', '1', 'light', 130, 150),
                ('tablet-dark-max', '1600x2560', '240', '0', 'dark', 130, 150),
                ('tablet-landscape-max', '1600x2560', '240', '1', 'light', 130, 150)]:
            if os.environ.get('QOL_LAYOUT_CASE') and not name.startswith(os.environ['QOL_LAYOUT_CASE']): continue
            loop.adb('shell', 'wm', 'size', dimensions); loop.adb('shell', 'wm', 'density', dpi)
            loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', rotation)
            loop.adb('shell', 'settings', 'put', 'system', 'font_scale', '1.0'); time.sleep(1)
            qol.preference('Display and readability', 'UI scale ' + str(scale) + '%')
            qol.preference('Display and readability', 'Font scale ' + str(font) + '%')
            qol.preference('Display and readability', 'High-contrast ' + theme)
            loop.selected = None; loop.choose(loop.unit('worker'))
            box = loop.bounds(map_node()); assert box[3] - box[1] >= 64 * int(dpi) / 160, (name, box)
            assert loop.state() == expected, 'Layout change advanced native state'
            capture(name + '-map')
            qol.menu('Cities and production'); loop.tap(expected['view']['own_cities'][0]['name'] + ' • population', True)
            loop.find('Choose production (changing item resets shields)'); capture(name + '-city')
            loop.adb('shell', 'input', 'keyevent', 'KEYCODE_BACK')
            qol.menu('Diplomacy'); loop.find('No other civilization', True); capture(name + '-diplomacy')
            loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', '1' if rotation == '0' else '0')
            time.sleep(1); map_node(); assert loop.state() == expected
            print('PASS:', name, 'map bounds, city/diplomacy, Back and open-screen rotation', flush=True)
        # Fixed gesture roles: inspect tap never moves; disabling drag still consumes a drag.
        qol.preference('Display and readability', 'Reset display defaults')
        loop.adb('shell', 'settings', 'put', 'system', 'user_rotation', '0'); time.sleep(1)
        qol.preference('Gestures and input help', 'Tap: inspect only')
        box = loop.bounds(map_node()); loop.tap_bounds(box); loop.find('Tile ', True)
        loop.tap('Return to map'); assert loop.state() == expected
        qol.preference('Gestures and input help', 'Disable drag panning')
        before = map_node().get('content-desc'); l,t,r,b = loop.bounds(map_node())
        loop.adb('shell', 'input', 'swipe', str((l+r)//2), str((t+b)//2), str((l+r)//2+100), str((t+b)//2), '350')
        assert map_node().get('content-desc') == before and loop.state() == expected
        qol.preference('Gestures and input help', 'Disable pinch zoom')
        l,t,r,b = loop.bounds(map_node()); x,y = str((l+r)//2), str((t+b)//2)
        before = map_node().get('content-desc')
        loop.adb('shell', 'CLASSPATH=/data/local/tmp/civ3touch-pinch.dex', 'app_process', '/system/bin', 'Pinch', x, y, '100', '200')
        assert map_node().get('content-desc') == before and loop.state() == expected
        qol.preference('Gestures and input help', 'Reset gesture defaults')
        loop.tap('Center'); l,t,r,b = loop.bounds(map_node()); x,y = str((l+r)//2), str((t+b)//2)
        loop.adb('shell', 'input', 'mouse', 'tap', x, y)
        before = map_node().get('content-desc')
        loop.adb('shell', 'input', 'keyevent', 'KEYCODE_DPAD_RIGHT')
        assert map_node().get('content-desc') != before and loop.state() == expected
        loop.adb('shell', 'input', 'keyevent', 'KEYCODE_C')
        before = map_node().get('content-desc')
        loop.adb('shell', 'input', 'mouse', 'scroll', x, y, '--axis', 'VSCROLL,1')
        assert map_node().get('content-desc') != before and loop.state() == expected
        for _ in range(12):
            loop.adb('shell', 'input', 'keyevent', 'KEYCODE_TAB')
            if any(n.get('focused') == 'true' and n.get('text') == 'Actions' for n in loop.tree().iter('node')):
                break
        else: raise AssertionError('Actions is unreachable through keyboard focus')
        loop.adb('shell', 'input', 'keyevent', 'KEYCODE_ENTER')
        loop.find('Game settings'); loop.tap('Return to map'); assert loop.state() == expected
        print('PASS: inspect-only tap, disabled drag/pinch, reset, keyboard camera/focus and mouse wheel without native commands', flush=True)
    finally:
        # Every settings screen offers a reset reachable by scrolling at any supported scale.
        loop.adb('shell', 'input', 'keyevent', 'KEYCODE_BACK')
        loop.launch()
        qol.preference('Display and readability', 'Reset display defaults')
        loop.adb('shell', 'wm', 'size', size.group(1) if size else 'reset')
        loop.adb('shell', 'wm', 'density', density.group(1) if density else 'reset')
        for key, value in settings.items():
            if value == 'null': loop.adb('shell', 'settings', 'delete', 'system', key)
            else: loop.adb('shell', 'settings', 'put', 'system', key, value)


if __name__ == '__main__': main()
