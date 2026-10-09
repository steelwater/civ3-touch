#!/usr/bin/env python3
"""M7 synthetic save/QoL journey on an explicitly chosen disposable API-36 emulator.

Uses real controls; reads snapshots and save bytes as assertions. Corruption cases
only replace this disposable session's synthetic quick slot, restoring it in finally.
Does not touch imported game data or normal save slots.
"""
import importlib.util
import json
from pathlib import Path
import subprocess
import time

spec = importlib.util.spec_from_file_location('loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)
assert not loop.args.imported, 'M7 public evidence requires synthetic mode'


def menu(label):
    loop.tap('Actions'); loop.tap(label)


def saved(name):
    return loop.adb('shell', 'run-as', loop.PACKAGE, 'cat', 'no_backup/synthetic-saves/' + name + '.json')


def files():
    return loop.adb('shell', 'run-as', loop.PACKAGE, 'ls', 'no_backup/synthetic-saves').split()


def overwrite_quick(value):
    subprocess.run(['adb', '-s', loop.args.serial, 'shell', 'run-as', loop.PACKAGE,
                    'sh', '-c', "'cat > no_backup/synthetic-saves/quick.json'"], input=value, text=True, check=True)


def quick_save():
    before = loop.state(); menu('Quick Save'); loop.tap('Confirm'); current = loop.wait(before['_sequence'])
    loop.find('Quick Save complete.', True)
    assert current['view'] == before['view']
    return current


def quick_load():
    menu('Quick Load'); loop.tap('Confirm'); loop.find('Saved game loaded.', True)
    return loop.state()


def preference(section, value):
    menu('Game settings'); loop.tap(section); loop.tap(value); time.sleep(.7)


def main():
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    loop.tap('New Game'); loop.find('settler • Show actions')
    # Existing manual slot is never modified by Quick Save or turn autosaves.
    manual = saved('manual') if 'manual.json' in files() else None
    loop.unit_action('settler', 'Build City'); loop.selected = None
    loop.city_production('warrior'); loop.city_production('worker', queue=True)
    loop.menu('Research')
    options = next(a['SetResearch']['options'] for a in loop.state()['available'] if isinstance(a, dict) and 'SetResearch' in a)
    before = loop.state(); loop.tap(options[0]['name'] + ' • ', True); loop.wait(before['_sequence'])
    snapshot = quick_save(); quick = saved('quick')
    expected_next = loop.end()
    before = loop.state(); menu('Quick Save'); loop.tap('Cancel')
    assert saved('quick') == quick and loop.state() == before
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    # Relaunch remains a welcome screen until an explicit load.
    loop.find('New Game'); menu('Quick Load'); loop.tap('Confirm'); loop.find('Saved game loaded.', True)
    assert loop.state()['view'] == snapshot['view'] and loop.state()['queues'] == snapshot['queues']
    assert loop.end()['view'] == expected_next['view']
    loop.end(); loop.end(); loop.end()
    autos = [name for name in files() if name.startswith('auto-') and name.endswith('.json')]
    assert len(autos) == 3, autos
    turns = sorted(json.loads(saved(name[:-5]))['turn'] for name in autos)
    turn = loop.state()['view']['turn']; assert turns == [turn-2, turn-1, turn], turns
    print('PASS: explicit quick save/relaunch/load, cancelled overwrite, production/research state, next-turn equality and 3-slot retention', flush=True)
    preference('Autosave settings', 'Turn autosaves off')
    retained = {name: saved(name[:-5]) for name in autos}
    loop.end(); assert retained == {name: saved(name[:-5]) for name in autos}
    preference('Autosave settings', 'Autosave every 3 turn(s)')
    for _ in range(3): loop.end()
    assert retained != {name: saved(name[:-5]) for name in autos}
    preference('Autosave settings', 'Autosave every 1 turn(s)')
    if manual is not None: assert saved('manual') == manual
    before = loop.state(); valid = saved('quick')
    try:
        for bad in ['not JSON', json.dumps(dict(json.loads(valid), version=999))]:
            overwrite_quick(bad); menu('Quick Load'); loop.tap('Confirm')
            loop.find('Could not complete action.', True)
            assert loop.state() == before, 'Rejected load replaced the live game'
    finally: overwrite_quick(valid)
    print('PASS: cadence off/3, independent manual slot and malformed/incompatible load preservation', flush=True)
    # Config and foreground transitions must not dispatch native commands.
    before = loop.state()
    preference('Display and readability', 'High-contrast dark')
    preference('Display and readability', 'Font scale 150%')
    assert loop.state() == before
    preference('Display and readability', 'Reset display defaults')
    loop.adb('shell', 'input', 'keyevent', 'KEYCODE_HOME'); loop.launch()
    assert loop.state() == before
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    menu('Resume recovery save'); loop.find('Saved game loaded.', True)
    assert loop.state()['view'] == before['view'] and loop.state()['queues'] == before['queues']
    print('PASS: theme/font recreation, background/resume and explicit process-death recovery without duplicate turns', flush=True)
    menu('Saves and backups'); loop.find('Load auto-', True)
    loop.tap('Return to map')
    print('PASS: dated turn autosaves are reachable through backup management', flush=True)


if __name__ == '__main__': main()
