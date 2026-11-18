#!/usr/bin/env python3
"""Fail when host runtime source names a package from the distribution seed."""
import json
import os
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SEED = ROOT / "crates/server/trusted-plugins.json"
SOURCE_ROOTS = (
    "crates/server/src",
    "crates/plugin-host/src",
    "crates/machine/src",
    "crates/core/src",
    "crates/client/src",
    "crates/cli/src",
    "apps/web/src",
    "apps/apple",
)
SOURCE_SUFFIXES = {".rs", ".swift", ".ts", ".js", ".svelte", ".mjs"}
NON_RUNTIME_DIRS = {"tests", "Tests", "UITests", "test", "fixtures", "node_modules", "dist", "target"}


def runtime_sources():
    for source_root in SOURCE_ROOTS:
        for directory, children, files in os.walk(ROOT / source_root):
            children[:] = sorted(
                child for child in children
                if not child.startswith(".") and child not in NON_RUNTIME_DIRS
            )
            for name in sorted(files):
                path = Path(directory) / name
                if path.suffix in SOURCE_SUFFIXES and not any(
                    qualifier in name for qualifier in (".test.", ".spec.")
                ):
                    yield path


def main():
    entries = json.loads(SEED.read_text(encoding="utf-8"))["plugins"]
    identifiers = "|".join(re.escape(entry["id"]) for entry in entries)
    pattern = re.compile(rf"(?<![A-Za-z0-9_-])(?:{identifiers})(?![A-Za-z0-9_-])")
    matches = 0
    for path in runtime_sources():
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            match = pattern.search(line)
            if match:
                matches += 1
                print(f"{path.relative_to(ROOT)}:{number}: distribution package ID {match[0]}")
    if matches:
        print(f"{matches} references to distribution package IDs in host runtime source.")
        return 1
    print(f"Host runtime source names none of the {len(entries)} distribution package IDs.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
