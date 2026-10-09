#!/usr/bin/env python3
"""Real SAF native-save export/import, cancellation and invalid-file checks.

Dedicated disposable emulator only. Uses synthetic saves and leaves clearly named
JSON test documents in Downloads. Never captures imported artwork.
"""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import time

spec = importlib.util.spec_from_file_location('qol', Path(__file__).with_name('android-qol-acceptance.py'))
qol = importlib.util.module_from_spec(spec); spec.loader.exec_module(qol)
loop = qol.loop


def downloads():
    hierarchy = loop.tree()
    if not any(n.get('text') == 'Downloads' for n in hierarchy.iter('node')):
        roots = next(n for n in hierarchy.iter('node') if n.get('content-desc') == 'Show roots')
        loop.tap_bounds(loop.bounds(roots))
    loop.tap('Downloads')


def document(importing):
    qol.menu('Saves and backups')
    loop.tap('Import native save…' if importing else 'Export native save…')
    if importing: loop.tap('Confirm')


def import_file(name, valid):
    document(True); downloads(); loop.tap(name)
    loop.find('Native save imported.' if valid else 'Could not complete action.', True)


def main():
    loop.adb('shell', 'am', 'force-stop', loop.PACKAGE); loop.launch()
    qol.menu('Resume recovery save'); loop.find('Saved game loaded.', True)
    original = loop.state()
    before = set(loop.adb('shell', 'ls', '/sdcard/Download').splitlines())
    document(False); downloads(); loop.tap('Save')
    loop.find('Native save exported.', True)
    created = set(loop.adb('shell', 'ls', '/sdcard/Download').splitlines()) - before
    assert len(created) == 1, created
    name = created.pop()
    exported = loop.adb('shell', 'cat', '/sdcard/Download/' + name)
    assert json.loads(exported) == json.loads(qol.saved('recovery'))
    loop.end()
    import_file(name, True)
    assert loop.state()['view'] == original['view'] and loop.state()['queues'] == original['queues']
    print('PASS: ACTION_CREATE_DOCUMENT export and ACTION_OPEN_DOCUMENT import restore matching native state', flush=True)
    for importing in [False, True]:
        current = loop.state(); recovery = qol.saved('recovery')
        document(importing); loop.adb('shell', 'input', 'keyevent', 'KEYCODE_BACK')
        loop.find('Save document operation cancelled.', True)
        assert loop.state() == current and qol.saved('recovery') == recovery
    print('PASS: cancelling either document picker preserves live state and recovery bytes', flush=True)
    with tempfile.TemporaryDirectory(prefix='civ3touch-documents-') as scratch:
        for label, content in [('malformed', 'not a native save'),
                               ('future-version', json.dumps(dict(json.loads(exported), version=999)))]:
            file = Path(scratch) / ('Civ3Touch-M7-' + label + '-' + str(time.time_ns()) + '.json')
            file.write_text(content)
            subprocess.run(['adb', '-s', loop.args.serial, 'push', str(file), '/sdcard/Download/' + file.name], check=True, stdout=subprocess.DEVNULL)
            current = loop.state(); recovery = qol.saved('recovery')
            import_file(file.name, False)
            assert loop.state() == current and qol.saved('recovery') == recovery
            print('PASS:', label, 'external document rejected without replacing live state or recovery', flush=True)
    print('Exported synthetic document:', name, flush=True)


if __name__ == '__main__': main()
