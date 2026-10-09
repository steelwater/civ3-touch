#!/usr/bin/env python3
"""Research-only metadata probe. Never loads gameplay or decompresses input.

Offsets: OpenCiv3 QueryCiv3.cs at 6a8db067dc2c07ba28b5e648b4c6b0a0b771c769.
Evidence and limits: docs/compatibility/README.md. No upstream code copied.
"""

import argparse
import json
import os
import stat
import struct
import sys

MAX_FILE_BYTES = 32 * 1024 * 1024
HEADER_BYTES = 736
OBSERVED = {
    (b"BIC ", 4, 1): "vanilla-rules",
    (b"BICX", 11, 18): "ptw-rules",
    (b"BICX", 12, 7): "conquests-rules",
    (b"BICX", 12, 8): "conquests-rules",
}


class ProbeError(ValueError):
    """An input cannot be inspected within this probe's bounds."""


def inspect_header(header, size):
    if size > MAX_FILE_BYTES:
        raise ProbeError("oversized: research limit is 32 MiB")
    if size < 0 or len(header) > size:
        raise ProbeError("invalid: inconsistent input size")
    if len(header) < min(size, HEADER_BYTES):
        raise ProbeError("truncated: incomplete header read")
    if len(header) < 2:
        raise ProbeError("truncated: need at least two signature bytes")
    result = {"bytes": size, "scope": "header only; body and integrity not validated"}
    # This is a heuristic from QueryCiv3.Util, not proof of a valid DCL stream.
    if header[0] == 0 and header[1] in (4, 5, 6):
        return dict(result, status="compressed_candidate", container="possible-pkware-dcl",
                    diagnostic="Decompression required; inner format/version unknown")
    magic = bytes(header[:4])
    if magic == b"CIV3":
        if size < 14:
            raise ProbeError("truncated: CIV3 metadata requires 14 bytes")
        marker, major, minor = struct.unpack_from("<hii", header, 4)
        return dict(result, status="unverified_sav", signature="CIV3", marker=marker,
                    major=major, minor=minor,
                    diagnostic="Candidate save header; no real SAV sample verified or body parsed")
    if magic not in (b"BIC ", b"BICX", b"BICQ"):
        raise ProbeError("unknown_signature: no supported research header detected")
    if size < HEADER_BYTES:
        raise ProbeError("truncated: rules metadata requires a 736-byte header")
    if header[4:8] != b"VER#":
        raise ProbeError("invalid: expected VER# at offset 4")
    count, length = struct.unpack_from("<ii", header, 8)
    if count != 1 or length != 720:
        raise ProbeError("unsupported_layout: expected one 720-byte VER# record")
    major, minor = struct.unpack_from("<ii", header, 24)
    family = OBSERVED.get((magic, major, minor))
    return dict(result, status="metadata_only" if family else "unverified_version",
                signature=magic.decode("ascii"), major=major, minor=minor,
                family=family,
                diagnostic="Observed header variant; gameplay unsupported" if family else
                "Version/signature pair not locally verified; do not interpret sections")


def inspect_file(path):
    # No content-supplied paths are followed. O_NONBLOCK also avoids a FIFO hang
    # before fstat can reject non-regular files. Final symlinks are rejected.
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    with os.fdopen(fd, "rb") as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ProbeError("invalid: input must be a regular file")
        if before.st_size > MAX_FILE_BYTES:
            raise ProbeError("oversized: research limit is 32 MiB")
        header = source.read(HEADER_BYTES)
        after = os.fstat(source.fileno())
        if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ProbeError("changed: source changed during inspection; retry on a stable copy")
    return inspect_header(header, before.st_size)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", help="explicit local file; never modified")
    args = parser.parse_args()
    try:
        result = inspect_file(args.path)
    except (OSError, ProbeError) as error:
        # Avoid echoing a private path or file content in a shareable diagnostic.
        message = str(error) if isinstance(error, ProbeError) else "io_error: cannot read regular input"
        print(json.dumps({"status": "error", "diagnostic": message}), file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
