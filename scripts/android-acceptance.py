#!/usr/bin/env python3
"""Exercise real touch -> JNI -> FreeC3 -> rendered state on an installed debug APK.

Only synthetic engine snapshots are read from debug logs. No game data required.
"""
import argparse
import json
import re
import subprocess
import time
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser()
parser.add_argument('serial', help='explicit ARM64 device/emulator serial')
parser.add_argument("--imported", action="store_true", help="use an existing completed GOG import instead of debug synthetic mode")
args = parser.parse_args()
package = 'org.civ3touch.spike'


def adb(*command):
    return subprocess.check_output(['adb', '-s', args.serial, *command], text=True)


def tree():
    adb('shell', 'uiautomator', 'dump', '/sdcard/civ3touch-window.xml')
    return ET.fromstring(adb('shell', 'cat', '/sdcard/civ3touch-window.xml'))


def node(predicate):
    for _ in range(12):
        for item in tree().iter('node'):
            if predicate(item):
                return item
        time.sleep(.25)
    raise AssertionError('Expected visible UI node was not found')


def bounds(item):
    return list(map(int, re.findall(r'\d+', item.attrib['bounds'])))


def tap_button(text):
    item = node(lambda n: n.get('text', '').lower() == text.lower() and n.get('enabled') == 'true')
    left, top, right, bottom = bounds(item)
    adb('shell', 'input', 'tap', str((left + right) // 2), str((top + bottom) // 2))


def states(pid):
    logs = adb('logcat', '-d', '--pid=' + pid, '-s', 'Civ3Touch:I', '*:S')
    return [json.loads(line.split('GAME_STATE ', 1)[1]) for line in logs.splitlines() if 'GAME_STATE ' in line]


def wait_state(pid, previous):
    for _ in range(80):
        found = states(pid)
        if len(found) > previous:
            # Wait for main-thread rendering, not only the earlier native log.
            node(lambda n: n.get('content-desc', '').startswith('Generated map. settler'))
            return found[-1], len(found)
        time.sleep(.25)
    raise AssertionError('No native snapshot observed after touch')


def map_node():
    return node(lambda n: n.get('content-desc', '').startswith('Generated map. settler'))


def tap_tile(dx=0, dy=0):
    left, top, right, bottom = bounds(map_node())
    density = int(re.findall(r'density: (\d+)', adb('shell', 'wm', 'density'))[-1]) / 160
    x = (left + right) / 2 + (dx - dy) * 48 * density
    y = (top + bottom) / 2 + (dx + dy) * 28 * density
    assert left <= x < right and top <= y < bottom, 'Tile must be visible for this test'
    adb('shell', 'input', 'tap', str(round(x)), str(round(y)))


def launch(missing=False):
    adb('shell', 'am', 'force-stop', package)
    adb('shell', 'am', 'start', '-W', '-n', package + '/.MainActivity', '--ez', 'missingRules', str(missing).lower(), '--ez', 'synthetic', str(not args.imported).lower())
    return adb('shell', 'pidof', package).strip()



def main():
    pid = launch()
    tap_button('Play' if args.imported else 'New Game')
    initial, count = wait_state(pid, 0)
    unit = initial['view']['known_units'][0]
    assert initial['view']['turn'] == 1 and initial['view']['visible_tile_count']
    assert unit['unit_type_name'] == 'settler'
    assert initial['moves'][0]['destinations']
    tap_tile()
    node(lambda n: '. Selected.' in n.get('content-desc', ''))
    # A two-tile destination must be refused without queueing future movement.
    tap_tile(2, 0)
    rejected, count = wait_state(pid, count)
    assert rejected['result']['errors'] and rejected['view'] == initial['view']
    assert rejected['view']['known_units'][0]['destination'] is None
    # Select a visible immediate step advertised by the engine.
    destination = initial['moves'][0]['destinations'][0]
    dx = destination['x'] - unit['position']['x']
    dy = destination['y'] - unit['position']['y']
    tap_tile(dx, dy)
    moved, count = wait_state(pid, count)
    assert not moved['result']['errors']
    assert moved['view']['known_units'][0]['position'] == destination
    assert sum('UnitMoved' in e for e in moved['result']['events']) == 1
    node(lambda n: f"at {destination['x']}, {destination['y']}. Selected." in n.get('content-desc', ''))
    tap_button('End Turn')
    ended, count = wait_state(pid, count)
    assert ended['view']['turn'] == 2 and not ended['result']['errors']
    assert ended['view']['known_units'][0]['position'] == destination
    assert ended['view']['known_units'][0]['movement'] == unit['max_movement']
    node(lambda n: n.get('text', '').startswith('Turn 2'))
    print('PASS: New Game -> touch select -> rejected distant move -> legal one-tile move -> End Turn')
    tap_button('New Game')
    reset, count = wait_state(pid, count)
    assert reset['view']['turn'] == 1
    assert reset['view']['known_units'][0]['position'] == unit['position']
    assert '. Selected.' not in map_node().get('content-desc')
    print('PASS: New Game resets turn, unit and selection')
    launch(missing=True)
    tap_button('Play' if args.imported else 'New Game')
    node(lambda n: n.get('text', '').startswith('Could not complete action.'))
    print('PASS: Missing rules show a readable error')
    pid = launch()
    tap_button('Play' if args.imported else 'New Game')
    recovered, _ = wait_state(pid, 0)
    assert recovered['view']['turn'] == 1
    print('PASS: Normal launch recovers after missing rules')


if __name__ == "__main__":
    main()
