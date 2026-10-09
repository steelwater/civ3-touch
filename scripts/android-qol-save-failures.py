#!/usr/bin/env python3
"""Missing slot and private-write failure through real M7 controls.

Disposable synthetic session only. Temporarily renames its test quick save and
restores it in finally. An empty staging directory simulates inability to open a
write target without filling the emulator disk or touching normal saves.
"""
import importlib.util
from pathlib import Path
import json

spec = importlib.util.spec_from_file_location('qol', Path(__file__).with_name('android-qol-acceptance.py'))
qol = importlib.util.module_from_spec(spec); spec.loader.exec_module(qol)
loop = qol.loop
root = 'no_backup/synthetic-saves/'


def app(*args): return loop.adb('shell', 'run-as', loop.PACKAGE, *args)


def main():
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    qol.menu('Resume recovery save'); loop.find('Saved game loaded.', True)
    before = loop.state(); prior = qol.saved('quick')
    assert 'quick-test-backup.json' not in qol.files()
    app('mv', root + 'quick.json', root + 'quick-test-backup.json')
    try:
        qol.menu('Quick Load'); loop.find('Empty slot', True); loop.tap('Confirm')
        loop.find('Could not complete action.', True); assert loop.state() == before
    finally: app('mv', root + 'quick-test-backup.json', root + 'quick.json')
    assert qol.saved('quick') == prior
    app('mkdir', root + 'quick.json.pending')
    qol.menu('Quick Save'); loop.tap('Confirm'); loop.find('Could not complete action.', True)
    assert qol.saved('quick') == prior and loop.state() == before
    assert 'quick.json.pending' not in qol.files()
    print('PASS: missing quick slot and private staging-write failure preserve the current game and previous quick save', flush=True)
    # The accepted pre-M7 manual slot survives installation, and still loads and plays.
    manual = json.loads(qol.saved('manual'))
    qol.menu('Load saved game'); loop.find('Saved game loaded.', True)
    assert loop.state()['view']['turn'] == manual['turn']
    continued = loop.end(); assert continued['view']['turn'] == manual['turn'] + 1
    loop.end(); snapshot = qol.quick_save(); expected = loop.end()
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    loaded = qol.quick_load()
    assert loaded['view'] == snapshot['view'] and loaded['queues'] == snapshot['queues']
    assert loop.end()['view'] == expected['view']
    print('PASS: retained pre-M7 manual save plays multiple turns, Quick Save/relaunch/Quick Load restores state and matching future turn', flush=True)


if __name__ == '__main__': main()
