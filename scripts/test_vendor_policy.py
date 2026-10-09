#!/usr/bin/env python3
"""Synthetic checks that resource-patch provenance remains a fail-closed gate."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch

VERIFY = Path(__file__).with_name('verify-upstream.py').resolve()


class VendorPolicyTests(unittest.TestCase):
    def verify(self, change=None, addition=False, wrong_origin=False):
        original = hashlib.sha256(b'original').hexdigest()
        updated = hashlib.sha256(b'updated').hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'docs').mkdir()
            (root / 'vendor/freec3').mkdir(parents=True)
            (root / 'vendor/freec3/plain.rs').write_bytes(b'original')
            (root / 'vendor/freec3/patched.rs').write_bytes(b'updated')
            (root / 'docs/freec3-baseline.json').write_text(json.dumps({
                'revision': 'baseline', 'files': {'plain.rs': original, 'patched.rs': original}}))
            (root / 'docs/freec3-resource-patches.json').write_text(json.dumps({
                'baseline_revision': 'baseline', 'files': {'patched.rs': {
                    'original_sha256': 'wrong' if wrong_origin else original, 'sha256': updated}}}))
            if change:
                (root / 'vendor/freec3' / change).write_bytes(b'unreviewed edit')
            files = b'vendor/freec3/plain.rs\0vendor/freec3/patched.rs\0'
            if addition:
                files += b'vendor/freec3/unrecorded.rs\0'
            previous = Path.cwd()
            try:
                os.chdir(root)
                with patch('subprocess.check_output', return_value=files), contextlib.redirect_stdout(io.StringIO()):
                    runpy.run_path(str(VERIFY), run_name='__main__')
            finally:
                os.chdir(previous)

    def test_recorded_patch_and_unchanged_baseline_pass(self):
        self.verify()

    def test_unexpected_changes_to_either_kind_fail(self):
        for name in ['plain.rs', 'patched.rs']:
            with self.subTest(name=name), self.assertRaisesRegex(SystemExit, 'Unexpected vendor content'):
                self.verify(change=name)

    def test_unrecorded_source_addition_fails(self):
        with self.assertRaisesRegex(SystemExit, 'Unrecorded vendor addition'):
            self.verify(addition=True)

    def test_original_hash_chain_cannot_be_replaced(self):
        with self.assertRaisesRegex(SystemExit, 'original hash differs'):
            self.verify(wrong_origin=True)


if __name__ == '__main__':
    unittest.main()
