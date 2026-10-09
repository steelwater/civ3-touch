#!/usr/bin/env python3
"""Play the M3 loop by real UI touches; read debug snapshots only as evidence.

Requires the debug APK and a disposable ARM64 emulator. --imported uses its
existing user-imported GOG data. No game state is injected or rewritten.
"""
import argparse
from collections import deque
import json
import re
import subprocess
import time
import xml.etree.ElementTree as ET

parser = argparse.ArgumentParser()
parser.add_argument('serial')
parser.add_argument('--imported', action='store_true')
args = parser.parse_args()
PACKAGE = 'org.civ3touch.spike'
selected = None
end_bounds = None


def adb(*parts):
    return subprocess.check_output(['adb', '-s', args.serial, *parts], text=True)


def tree():
    for _ in range(4):
        try:
            result = adb('shell', 'uiautomator', 'dump', '/sdcard/civ3touch-loop-window.xml')
        except subprocess.CalledProcessError:
            time.sleep(.25)
            continue
        if 'dumped to:' in result:
            return ET.fromstring(adb('shell', 'cat', '/sdcard/civ3touch-loop-window.xml'))
        time.sleep(.25)
    raise AssertionError('UI Automator did not produce a fresh hierarchy')


def bounds(node):
    return list(map(int, re.findall(r'\d+', node.get('bounds'))))


def tap_bounds(box):
    l, t, r, b = box
    x, y = str((l+r)//2), str((t+b)//2)
    # Let the inspected window settle before injecting input.
    time.sleep(.5)
    # Keep down/up separated like a finger tap, including on a loaded emulator.
    adb('shell', 'input', 'swipe', x, y, x, y, '100')
    # Let native window transitions settle before starting a new UI Automator connection.
    time.sleep(.35)


def find(label, prefix=False):
    for _ in range(12):
        hierarchy = tree()
        for n in hierarchy.iter('node'):
            text = n.get('text', '')
            if (text.startswith(label) if prefix else text.lower() == label.lower()) and n.get('enabled') == 'true':
                return n
        scrolls = [n for n in hierarchy.iter('node') if n.get('scrollable') == 'true']
        if scrolls:
            l, t, r, b = bounds(scrolls[-1])
            adb('shell', 'input', 'swipe', str((l+r)//2), str(b-30), str((l+r)//2), str(t+30), '350')
        time.sleep(.2)
    raise AssertionError('Missing UI control: ' + label)


def tap(label, prefix=False):
    tap_bounds(bounds(find(label, prefix)))


def state():
    return json.loads(adb('shell', 'run-as', PACKAGE, 'cat', 'files/debug-state.json'))


def wait(before):
    for _ in range(160):
        current = state()
        if current['_sequence'] != before:
            assert not current['result']['errors'], current['result']['errors']
            time.sleep(.12)
            return current
        time.sleep(.15)
    raise AssertionError('Game action did not complete')


def menu(label):
    tap('Actions')
    if label not in ['Choose unit', 'Cities and production', 'Research', 'Diplomacy', 'Game settings', 'Save game', 'Load saved game', 'Resume recovery save']:
        tap('Selected unit actions')
    tap(label)


def choose(unit):
    global selected
    uid = unit['id']['index']
    if selected != uid:
        menu('Choose unit')
        tap(unit['unit_type_name'] + ' #' + str(uid) + ' ', True)
        selected = uid


def unit(kind):
    return next(u for u in state()['view']['known_units'] if u['owner'] == 0 and u['unit_type_name'] == kind)


def move(u, destination):
    choose(u)
    current = state()
    n = next(n for n in tree().iter('node') if n.get('content-desc', '').startswith('Generated map.'))
    l,t,r,b = bounds(n)
    density = int(re.findall(r'density: (\d+)', adb('shell', 'wm', 'density'))[-1])/160
    dx = destination['x']-u['position']['x']; dy = destination['y']-u['position']['y']
    zoom = float(re.search(r'zoom (\d+\.\d+)', n.get('content-desc')).group(1))
    x = (l+r)/2+(dx-dy)*48*density*zoom; y = (t+b)/2+(dx+dy)*28*density*zoom
    assert l <= x < r and t <= y < b
    adb('shell', 'input', 'tap', str(round(x)), str(round(y)))
    return wait(current['_sequence'])


def end():
    global end_bounds
    current = state()
    if end_bounds is None: end_bounds = bounds(find('End Turn'))
    tap_bounds(end_bounds)
    return wait(current['_sequence'])


def unit_action(kind, label):
    choose(unit(kind)); before = state()['_sequence']; menu(label); return wait(before)


def city_production(option, queue=False):
    before = state(); city = before['view']['own_cities'][0]
    menu('Cities and production'); tap(city['name'] + ' • population', True)
    tap('Add to production queue' if queue else 'Choose production (changing item resets shields)')
    tap(option + ' • ', True); return wait(before['_sequence'])


def save_and_reload():
    before = state(); menu('Save game'); saved = wait(before['_sequence'])
    find('Game saved.', True)
    # A later live turn must not overwrite the separate manual slot.
    expected_next = end()
    adb('shell', 'input', 'keyevent', 'KEYCODE_HOME')
    adb('shell', 'am', 'force-stop', PACKAGE)
    launch()
    menu('Load saved game')
    for _ in range(100):
        current = state()
        if current['view'] == saved['view']: break
        time.sleep(.2)
    else: raise AssertionError('Manual save did not restore the complete view')
    find('Saved game loaded.', True)
    assert current['queues'] == saved['queues']
    global selected
    selected = None
    continued = end()
    assert continued['view'] == expected_next['view']
    assert continued['queues'] == expected_next['queues']
    adb('shell', 'input', 'keyevent', 'KEYCODE_HOME')
    adb('shell', 'am', 'start', '-W', '-n', PACKAGE + '/.MainActivity')
    assert state()['view'] == continued['view']
    adb('shell', 'am', 'force-stop', PACKAGE)
    launch()
    menu('Resume recovery save'); find('Saved game loaded.', True)
    assert state()['view'] == continued['view']
    selected = None
    end()
    print('PASS: separate manual save, quit/reload, continued play, suspend/resume and process-death recovery', flush=True)


def launch():
    adb('shell', 'am', 'start', '-W', '-n', PACKAGE + '/.MainActivity', '--ez', 'synthetic', str(not args.imported).lower())


def check_audio_settings():
    before = state()
    menu('Game settings')
    label = next(n.get('text') for n in tree().iter('node') if n.get('text') in ['Audio on', 'Audio off'])
    tap(label)
    menu('Game settings')
    tap('Audio off' if label == 'Audio on' else 'Audio on')
    assert state() == before
    print('PASS: in-game audio toggle is reachable and preserves simulation state', flush=True)


def check_known_diplomacy(before):
    owners = sorted({u['owner'] for u in before['view']['known_units'] + before['view']['known_cities'] if u['owner'] != before['view']['player']})
    assert owners, 'An attack target must be present in the known player view'
    menu('Diplomacy')
    find('Other civilizations in your known map information: ' + str(owners), True)
    tap('Return to map'); assert state() == before
    print('PASS: diplomacy shows only opponents in the known player view', flush=True)


def explore_or_attack():
    current = state()
    warriors = [u for u in current['view']['known_units'] if u['owner'] == 0 and u['attack'] > 0 and u['movement'] > 0]
    for u in warriors:
        attack = next((a['Attack'] for a in current['available'] if isinstance(a, dict) and 'Attack' in a and a['Attack']['unit_id'] == u['id']), None)
        if attack:
            check_known_diplomacy(current)
            choose(u); before = state()['_sequence']; menu('Attack enemy ' + str(attack['targets'][0]['index']))
            after = wait(before)
            if any('CombatResolved' in e for e in after['result']['events']): return True
    if not warriors: return False
    u = warriors[0]
    # Find a frontier using only the human's discovered terrain; no hidden world inspection.
    tiles = {(t['coord']['x'],t['coord']['y']): t for t in current['view']['visible_tiles']}
    start = (u['position']['x'],u['position']['y'])
    todo = deque([(start, [])]); seen = {start}; path = None; frontiers = []
    width = current['view']['map_width']; height = current['view']['map_height']
    while todo:
        pos, route = todo.popleft()
        neighbors = [(pos[0]+dx,pos[1]+dy) for dx in [-1,0,1] for dy in [-1,0,1] if dx or dy]
        if route and any(0 <= x < width and 0 <= y < height and (x,y) not in tiles for x,y in neighbors):
            frontiers.append((pos, route))
        for neighbor in neighbors:
            t = tiles.get(neighbor)
            if neighbor in seen or t is None or t['terrain'] in ['Ocean','Coast','Mountain','Ice']: continue
            seen.add(neighbor); todo.append((neighbor, route+[neighbor]))
    if frontiers:
        path = min(frontiers, key=lambda f: (f[0][1], len(f[1]), f[0][0]))[1]
    if path:
        destination = {'x':path[0][0], 'y':path[0][1]}
        options = next((m['destinations'] for m in current['moves'] if m['unit_id'] == u['id']), [])
        if destination in options: move(u, destination)
    return False


def main():
    global selected
    adb('shell', 'am', 'force-stop', PACKAGE); launch()
    tap('Play' if args.imported else 'New Game')
    find('settler • Show actions')
    initial = state(); selected = None
    settler = unit('settler')
    destination = next(m['destinations'][0] for m in initial['moves'] if m['unit_id'] == settler['id'])
    moved = move(settler, destination)
    assert unit('settler')['movement'] < settler['movement']
    # A friendly-occupied highlighted tile remains a legal move, not a selection shortcut.
    move(unit('worker'), destination)
    end(); founded = unit_action('settler', 'Build City')
    assert founded['view']['own_cities']; selected = None
    city_production('warrior'); city_production('worker', queue=True)
    before = state(); menu('Research')
    options = next(a['SetResearch']['options'] for a in before['available'] if isinstance(a, dict) and 'SetResearch' in a)
    tech = min(options, key=lambda t:t['cost'])
    tap(tech['name'] + ' • ', True); wait(before['_sequence'])
    unit_action('worker', 'Build Road')
    produced = researched = road = combat = False
    explored_before = len(state()['view']['visible_tiles'])
    for _ in range(100):
        if not combat: combat = explore_or_attack()
        current = end()
        produced |= any('UnitProduced' in e for e in current['result']['events'])
        researched |= tech['id'] in current['view']['researched_techs']
        road |= any(t['road_level'] for t in current['view']['visible_tiles'])
        print('Turn', current['view']['turn'], 'produced', produced, 'research', researched, 'road', road, 'combat', combat, flush=True)
        if produced and researched and road and combat: break
    assert produced and researched and road and combat
    assert len(state()['view']['visible_tiles']) > explored_before
    print('PASS: movement costs, city, production queue, research, Worker road, fog, AI encounter and combat', flush=True)
    save_and_reload()
    if args.imported: check_audio_settings()


if __name__ == '__main__': main()
