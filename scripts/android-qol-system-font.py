#!/usr/bin/env python3
"""Combined Android/app font extremes on a disposable synthetic city-bearing recovery save.

Restores display settings. Run after android-qol-layout and before save-failure checks.
"""
import importlib.util
from pathlib import Path
import time
spec = importlib.util.spec_from_file_location('layout', Path(__file__).with_name('android-qol-layout.py'))
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
l = m.loop
old = {k:m.setting(k) for k in ['font_scale','user_rotation','accelerometer_rotation']}
size = m.re.search(r'Override size: (\d+x\d+)', l.adb('shell','wm','size'))
density = m.re.search(r'Override density: (\d+)', l.adb('shell','wm','density'))
try:
    l.adb('shell','am','force-stop',l.PACKAGE); l.launch()
    m.qol.menu('Resume recovery save'); l.find('Saved game loaded.',True)
    before = l.state()
    l.adb('shell','wm','size','1080x2400'); l.adb('shell','wm','density','420')
    l.adb('shell','settings','put','system','accelerometer_rotation','0')
    l.adb('shell','settings','put','system','user_rotation','0')
    m.qol.preference('Display and readability','UI scale 130%')
    m.qol.preference('Display and readability','Font scale 150%')
    l.adb('shell','settings','put','system','font_scale','1.5'); time.sleep(1)
    for rotation in ['0','1']:
        l.adb('shell','settings','put','system','user_rotation',rotation); time.sleep(1)
        box=l.bounds(m.map_node()); assert box[3]-box[1]>=168,box
        m.capture('system-font-150-rotation-'+rotation)
        l.tap('Actions')
        if rotation == '0': l.find('New Game')
        l.tap('Game settings'); l.find('Display and readability'); l.tap('Return to map')
        assert l.state()==before
    l.adb('shell', 'am', 'force-stop', l.PACKAGE); l.launch()
    m.qol.menu('Game settings'); l.find('Display and readability'); l.tap('Return to map')
    m.qol.menu('Resume recovery save')
    l.tap('Turn ', True); l.find('Saved game loaded.', True); l.tap('Return to map')
    assert l.state()['view'] == before['view'] and l.state()['queues'] == before['queues']
    print('PASS: maximum-scale settings remain reachable after process recreation; explicit recovery restores state', flush=True)
    print('PASS: Android text 150% combined with app UI130/font150, phone portrait/landscape map and settings, unchanged native state',flush=True)
finally:
    l.adb('shell','settings','put','system','font_scale','1.0')
    l.launch(); l.adb('shell','input','keyevent','KEYCODE_BACK'); l.launch()
    m.qol.preference('Display and readability','Reset display defaults')
    l.adb('shell','wm','size',size.group(1) if size else 'reset')
    l.adb('shell','wm','density',density.group(1) if density else 'reset')
    for k,v in old.items():
        if v == 'null': l.adb('shell', 'settings', 'delete', 'system', k)
        else: l.adb('shell', 'settings', 'put', 'system', k, v)
