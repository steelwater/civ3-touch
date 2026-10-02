#!/usr/bin/env python3
"""Emit paths/size metadata only; never emit or copy proprietary file contents."""
import argparse
import collections
import json
from pathlib import Path

REQUIRED = [f"Art/Terrain/{name}.pcx" for name in ("xggc", "xtgc", "xdgc", "xdgp", "xdpc", "xpgc")] + [
    "Art/Units/Settler/settler.ini", "Art/Units/Settler/settDefault.flc", "Art/Units/Settler/settRun.flc"]


def inventory(root):
    root = root.resolve(strict=True)
    files = sorted(p for p in root.rglob("*") if p.is_file())
    rows = []
    names = {}
    for path in files:
        if not path.resolve().is_relative_to(root):
            raise ValueError("file escapes the installation root")
        relative = path.relative_to(root).as_posix()
        names.setdefault(relative.casefold(), []).append(relative)
        if relative in REQUIRED:
            category = "required-for-image-probe"
        elif path.suffix.lower() in (".exe", ".dll", ".pdf", ".hlp"):
            category = "runtime-unneeded-for-spike"
        elif relative.startswith("Art/") and path.suffix.lower() in (".pcx", ".flc", ".wav", ".amb", ".mp3"):
            category = "optional-for-later-prototype"
        else:
            category = "unknown-or-deferred"
        rows.append({"path": relative, "bytes": path.stat().st_size, "classification": category})
    missing = [name for name in REQUIRED if name not in {row["path"] for row in rows}]
    collisions = [values for values in names.values() if len(values) > 1]
    if missing or collisions:
        raise ValueError(f"missing exact paths: {missing}; case collisions: {collisions}")
    return {"schema": 1, "scope": "Extracted app directory; metadata only. Categories are M0-specific, not a full game requirement claim.",
            "file_count": len(rows), "total_bytes": sum(r["bytes"] for r in rows),
            "classification_counts": dict(collections.Counter(r["classification"] for r in rows)), "files": rows}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    print(json.dumps(inventory(args.root), indent=2))
