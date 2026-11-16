#!/usr/bin/env python3
"""Portable VANTAGE product versions. Canonical helper: UUUUI/scripts/version.py.

Copies travel with each product so standalone checkouts need only Python 3.
Package-manager, protocol and dependency versions are independent of this registry.
"""
from __future__ import annotations

import argparse
import datetime as dt
import fcntl
import json
import os
from pathlib import Path
import pwd
import re
import stat
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
PATTERN = re.compile(r"10\.([1-9][0-9]*)\.([0-9]{12})\Z")


def timestamp(value: str) -> str:
    if len(value) != 12 or not value.isascii() or not value.isdigit():
        raise ValueError("timestamp must be YYYYMMDDHHMM")
    dt.datetime.strptime(value, "%Y%m%d%H%M")
    return value


def load(root: Path) -> tuple[dict, dict[str, str]]:
    config = json.loads((root / "versioning.json").read_text())
    components = config["components"]
    if (not components or len(set(components)) != len(components)
            or config["headline"] not in components
            or any(not re.fullmatch(r"[a-z][a-z0-9-]*", name) for name in components)):
        raise ValueError("invalid versioning components")
    versions = {}
    for line in (root / "VERSION").read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2 or parts[0] in versions or not PATTERN.fullmatch(parts[1]):
            raise ValueError(f"invalid VERSION row: {line}")
        timestamp(parts[1].rsplit(".", 1)[1])
        versions[parts[0]] = parts[1]
    if set(versions) != set(components):
        raise ValueError("VERSION components differ from versioning.json")
    return config, {name: versions[name] for name in components}


def output_path(root: Path, name: str) -> Path:
    path = root / name
    if Path(name).is_absolute() or ".." in Path(name).parts or path.is_symlink():
        raise ValueError(f"invalid generated path: {name}")
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"generated path escapes the product: {name}")
    return path


def outputs(root: Path, config: dict, versions: dict[str, str]) -> dict[Path, str]:
    value = versions[config["headline"]]
    manifest = {"format": 1, "product": config["product"], "version": value, "components": versions}
    encoded = json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"
    result = {root / "version.json": encoded}
    for target in config.get("generated", []):
        path = output_path(root, target["path"])
        kind = target["kind"]
        if kind == "json":
            content = encoded
        elif kind == "js":
            content = ("// Generated from VERSION by scripts/version.sh sync.\n"
                       f"const release = {encoded.rstrip()};\n"
                       "export const { version, components } = release;\nexport default release;\n")
        elif kind == "go":
            content = ("// Copyright (c) 2026 Andrea Bodei — www.andeeabodei.com\n"
                       "// Generated from VERSION by scripts/version.sh sync.\n"
                       "package version\n\n// Value may be overridden by an explicit release build.\n"
                       f"var Value = {json.dumps(value)}\n")
        elif kind == "python":
            content = ('"""Generated from VERSION by scripts/version.sh sync."""\n'
                       f"__version__ = {value!r}\nCOMPONENT_VERSIONS = {versions!r}\n")
        else:
            raise ValueError(f"unknown generated kind: {kind}")
        if path in result:
            raise ValueError(f"duplicate generated path: {path}")
        result[path] = content
    return result


def owner(root: Path, previous: os.stat_result | None) -> tuple[int, int]:
    if os.geteuid() == 0:
        try:
            account = pwd.getpwnam("spabam")
            return account.pw_uid, account.pw_gid
        except KeyError:
            pass
    source = previous or root.stat()
    return source.st_uid, source.st_gid


def write(root: Path, path: Path, content: str) -> None:
    if path.is_file() and path.read_text() == content:
        return
    previous = path.stat() if path.exists() else None
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=".version-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as output:
            output.write(content)
            output.flush()
            os.fchmod(output.fileno(), stat.S_IMODE(previous.st_mode) if previous else 0o644)
            if os.geteuid() == 0:
                os.fchown(output.fileno(), *owner(root, previous))
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def registry(config: dict, versions: dict[str, str]) -> str:
    return ("# VANTAGE product/component versions: 10.RELEASE.YYYYMMDDHHMM\n"
            "# Every source change bumps its component and the product headline.\n"
            "# Use scripts/version.sh bump [COMPONENT...]; see VERSIONING.md.\n"
            + "".join(f"{name} {versions[name]}\n" for name in config["components"]))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("show", "value", "check", "sync", "bump"))
    parser.add_argument("components", nargs="*")
    parser.add_argument("--at", type=timestamp, help="explicit YYYYMMDDHHMM for reproducible release preparation")
    args = parser.parse_args(argv)
    if args.at and args.command != "bump":
        parser.error("--at is only valid for bump")
    if args.command not in ("value", "bump") and args.components:
        parser.error("this command takes no component names")
    if args.command in ("bump", "sync"):
        with (ROOT / ".version.lock").open("a+") as lock:
            if os.geteuid() == 0:
                os.fchown(lock.fileno(), *owner(ROOT, None))
            fcntl.flock(lock, fcntl.LOCK_EX)
            config, versions = load(ROOT)
            selected = set(args.components) | {config["headline"]}
            unknown = selected - set(versions)
            if unknown:
                raise ValueError("unknown component(s): " + ", ".join(sorted(unknown)))
            if args.command == "bump":
                now = args.at or dt.datetime.now().strftime("%Y%m%d%H%M")
                for name in selected:
                    major, release, previous = versions[name].split(".")
                    if now < previous:
                        raise ValueError("a version timestamp cannot move backwards")
                    versions[name] = f"{major}.{int(release) + (now[:8] != previous[:8])}.{now}"
            generated = outputs(ROOT, config, versions)
            if args.command == "bump":
                write(ROOT, ROOT / "VERSION", registry(config, versions))
            for path, content in generated.items():
                write(ROOT, path, content)
            print(f"{config['product']} {versions[config['headline']]} ({args.command})")
        return 0
    config, versions = load(ROOT)
    if args.command == "show":
        for name, value in versions.items():
            print(f"{name:<16} {value}")
    elif args.command == "value":
        if len(args.components) > 1:
            raise ValueError("value accepts at most one component")
        name = args.components[0] if args.components else config["headline"]
        if name not in versions:
            raise ValueError(f"unknown component: {name}")
        print(versions[name])
    else:
        stale = [str(path.relative_to(ROOT)) for path, content in outputs(ROOT, config, versions).items()
                 if not path.is_file() or path.read_text() != content]
        if stale:
            raise ValueError("stale generated versions: " + ", ".join(stale) + "; run scripts/version.sh sync")
        print(f"{config['product']} {versions[config['headline']]} (verified)")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, KeyError) as error:
        print(f"version.sh: {error}", file=sys.stderr)
        raise SystemExit(1)
