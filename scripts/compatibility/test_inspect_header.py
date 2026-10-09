"""Entirely invented headers, not extracted game fixtures."""

import hashlib
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

from inspect_header import HEADER_BYTES, MAX_FILE_BYTES, ProbeError, inspect_file, inspect_header


def rules(magic=b"BICX", major=12, minor=8):
    data = bytearray(HEADER_BYTES)
    data[:8] = magic + b"VER#"
    struct.pack_into("<ii", data, 8, 1, 720)
    struct.pack_into("<ii", data, 24, major, minor)
    return data


class HeaderTests(unittest.TestCase):
    def test_observed_rules_variants_report_metadata_without_claiming_body_support(self):
        for magic, major, minor in [(b"BIC ", 4, 1), (b"BICX", 11, 18),
                                     (b"BICX", 12, 7), (b"BICX", 12, 8)]:
            data = rules(magic, major, minor)
            result = inspect_header(data, len(data))
            self.assertEqual(result["status"], "metadata_only")
            self.assertEqual((result["major"], result["minor"]), (major, minor))
            self.assertIn("body and integrity not validated", result["scope"])

    def test_every_truncated_rules_header_is_rejected(self):
        data = rules()
        for size in range(HEADER_BYTES):
            with self.subTest(size=size), self.assertRaises(ProbeError):
                inspect_header(data[:size], size)

    def test_unknown_versions_and_reference_only_signature_are_not_assumed_compatible(self):
        for data in [rules(major=13), rules(minor=99), rules(major=-1), rules(b"BICQ")]:
            self.assertEqual(inspect_header(data, len(data))["status"], "unverified_version")

    def test_bad_header_record_counts_lengths_and_tags_fail(self):
        for offset, value in [(8, 0), (8, 2147483647), (12, -1), (12, 721)]:
            data = rules()
            struct.pack_into("<i", data, offset, value)
            with self.assertRaises(ProbeError):
                inspect_header(data, len(data))
        data[4:8] = b"NOPE"
        with self.assertRaises(ProbeError):
            inspect_header(data, len(data))

    def test_compression_prefix_is_only_a_candidate_even_without_a_body(self):
        for dictionary in (4, 5, 6):
            result = inspect_header(bytes([0, dictionary]), 2)
            self.assertEqual(result["status"], "compressed_candidate")
            self.assertIn("unknown", result["diagnostic"])
        for data in (b"\x00\x03", b"\x01\x06", b"PK\x03\x04", b"{}"):
            with self.assertRaises(ProbeError):
                inspect_header(data, len(data))

    def test_save_header_reports_raw_metadata_without_interpreting_the_version(self):
        data = b"CIV3" + struct.pack("<hii", 123, 99, 456)
        result = inspect_header(data, len(data))
        self.assertEqual(result["status"], "unverified_sav")
        self.assertEqual((result["marker"], result["major"], result["minor"]), (123, 99, 456))
        for size in range(4, 14):
            with self.assertRaises(ProbeError):
                inspect_header(data[:size], size)

    def test_size_limit_and_short_reads_fail_before_parsing(self):
        for data, size in [(rules(), MAX_FILE_BYTES + 1), (b"CIV3", 100), (rules(), -1)]:
            with self.assertRaises(ProbeError):
                inspect_header(data, size)

    def test_file_inspection_preserves_bytes_and_never_resolves_embedded_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.data"
            data = rules()
            data[32:51] = b"../../escape/secret"
            path.write_bytes(data)
            before = hashlib.sha256(path.read_bytes()).digest()
            self.assertEqual(inspect_file(path)["status"], "metadata_only")
            self.assertEqual(hashlib.sha256(path.read_bytes()).digest(), before)
            link = Path(directory) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                inspect_file(link)
            fifo = Path(directory) / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(ProbeError):
                inspect_file(fifo)

    def test_oversized_sparse_file_is_rejected_and_cli_errors_are_actionable(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "oversized"
            with path.open("wb") as output:
                output.truncate(MAX_FILE_BYTES + 1)
            with self.assertRaisesRegex(ProbeError, "oversized"):
                inspect_file(path)
            command = [sys.executable, str(Path(__file__).with_name("inspect_header.py")), str(path)]
            result = subprocess.run(command, capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 2)
            self.assertIn("oversized", result.stderr)
            self.assertNotIn(str(path), result.stderr)


if __name__ == "__main__":
    unittest.main()
