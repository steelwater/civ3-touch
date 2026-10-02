#!/usr/bin/env python3
"""Run success and missing-rules JNI probes on one explicitly selected device."""
import argparse
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('serial', help='adb device serial; never select a device implicitly')
args = parser.parse_args()
package = 'org.civ3touch.spike'


def adb(*command):
    return subprocess.check_output(['adb', '-s', args.serial, *command], text=True)


for missing in (False, True, False):
    adb('shell', 'am', 'force-stop', package)
    adb('shell', 'am', 'start', '-W', '-n', package + '/.MainActivity', '--ez', 'missingRules', str(missing).lower())
    pid = adb('shell', 'pidof', package).strip()
    expected = 'CORE_SMOKE_FAIL IllegalStateException' if missing else 'CORE_SMOKE_PASS turn=2'
    for _ in range(40):
        output = adb('logcat', '-d', '--pid=' + pid, '-s', 'Civ3Touch:I', '*:S')
        if expected in output:
            print('PASS: missing rules reported' if missing else 'PASS: native rules/command/replay call')
            break
        time.sleep(0.25)
    else:
        raise SystemExit('Expected app result not observed: ' + expected)
