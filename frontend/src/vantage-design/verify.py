#!/usr/bin/env python3
"""Validate a portable VANTAGE design snapshot without any sibling checkout."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import sys

REQUIRED = {"tokens.css", "design-system.css", "components.css", "runtime.js", "tailwind3.css", "verify.py"}


def verify(root: Path) -> str:
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != 1:
        raise ValueError("unsupported design manifest schema")
    version = manifest.get("version")
    files = manifest.get("files")
    if not isinstance(version, str) or not version or not isinstance(files, dict):
        raise ValueError("design manifest requires a version and file digests")
    missing = REQUIRED - files.keys()
    if missing:
        raise ValueError("design manifest omits required assets: " + ", ".join(sorted(missing)))
    fonts = [name for name in files if name.startswith("fonts/") and name.endswith(".woff2")]
    licenses = [name for name in files if name.startswith("fonts/") and ("license" in name.lower() or "ofl" in name.lower())]
    if len(fonts) < 2 or len(licenses) < 2:
        raise ValueError("both local font families and their licenses must be included")
    for name, expected in files.items():
        if not isinstance(name, str) or not isinstance(expected, str):
            raise ValueError("invalid design manifest entry")
        path = root / name
        if Path(name).is_absolute() or ".." in Path(name).parts or path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError(f"unsafe design asset path: {name}")
        if not path.is_file():
            raise ValueError(f"missing design asset: {name}")
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f"design asset differs from recorded release: {name}")
    return version


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="validate the recorded snapshot (default)")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    try:
        version = verify(args.root)
    except (ValueError, OSError) as exc:
        print(f"VANTAGE design check failed: {exc}", file=sys.stderr)
        return 1
    print(f"VANTAGE design {version}: local assets and licenses verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
