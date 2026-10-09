#!/usr/bin/env python3
"""PR 3 regressions through real UI actions on a disposable debug emulator.

Uses the loop harness's serial/--imported options and save-slot warnings.
Default synthetic mode keeps the normal imported game's save slots untouched.
"""
import importlib.util
from pathlib import Path
import time

spec = importlib.util.spec_from_file_location(
    'loop', Path(__file__).with_name('android-loop-acceptance.py'))
loop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(loop)


def feedback_contains(messages):
    # Expand the collapsible status before checking complete Android feedback.
    info = next(n for n in loop.tree().iter('node')
                if n.get('class') == 'android.widget.Button' and n.get('text', '').startswith('Turn '))
    opened = info.get('text', '').endswith('▾')
    if opened: loop.tap_bounds(loop.bounds(info))
    for _ in range(12):
        text = '\n'.join(n.get('text', '') for n in loop.tree().iter('node'))
        if all(message in text for message in messages):
            if opened: loop.tap('Turn ' + str(loop.state()['view']['turn']), True)
            return
        time.sleep(.2)
    raise AssertionError(f'Missing completion feedback {messages!r}: {text}')


loop.adb('shell', 'am', 'force-stop', loop.PACKAGE)
loop.launch()
loop.tap('Play' if loop.args.imported else 'New Game')
loop.find('settler • Show actions')
loop.unit_action('settler', 'Build City')
loop.selected = None
loop.city_production('warrior')
loop.city_production('worker', queue=True)
before = loop.state()
loop.menu('Research')
options = next(a['SetResearch']['options'] for a in before['available']
               if isinstance(a, dict) and 'SetResearch' in a)
tech = min(options, key=lambda t: t['cost'])
loop.tap(tech['name'] + ' • ', True)
loop.wait(before['_sequence'])
loop.unit_action('worker', 'Build Road')

produced = researched = completed = overflow = False
for _ in range(100):
    before = loop.state()
    if not any(items for _, items in before['queues']):
        loop.city_production('worker', queue=True)
        before = loop.state()
    current = loop.end()
    messages = []
    for event in current['result']['events']:
        if 'ProductionComplete' in event:
            produced = True
            messages.append('Produced ' + event['ProductionComplete']['item_name'] + '.')
            old = before['view']['own_cities'][0]
            city = current['view']['own_cities'][0]
            expected = old['shield_stockpile'] + city['shields_per_turn'] - old['production_cost']
            assert city['producing'] == 'worker'
            assert city['shield_stockpile'] == expected, (city, expected)
            overflow |= expected > 0
        if 'TechResearched' in event:
            research = event['TechResearched']
            assert research['player'] == 0
            assert research['tech_id'] == tech['id']
            researched = True
            messages.append('Research complete: ' + tech['id'].replace('_', ' ') + '.')
        if 'ActionCompleted' in event:
            completed = True
            messages.append('Completed ' + event['ActionCompleted']['action_id'].replace('_', ' ') + '.')
        if 'TurnStarted' in event:
            assert event['TurnStarted']['player'] == 0
    if messages:
        feedback_contains(messages)
        print('PASS: visible feedback at turn', current['view']['turn'], messages, flush=True)
    if produced and researched and completed and overflow:
        break
assert produced and researched and completed and overflow
print('PASS: queued Worker retains positive overflow after auto-selected Warrior', flush=True)
loop.save_and_reload()
loop.tap('New Game')
loop.find('settler • Show actions')
assert loop.state()['view']['turn'] == 1
loop.save_and_reload()
print('PASS: New Game -> Save -> Load and continued play', flush=True)
