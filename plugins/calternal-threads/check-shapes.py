"""Check pinned schema shapes and asset pins independently of install semantics."""

import hashlib
import json
import pathlib
import sys

sys.path.insert(0, sys.argv[1])
from description_input import Validation, read_json
from description_verification import CATALOGUE, check, manifest_schema, schema


def validate(shape, value, root, path):
    errors = Validation()
    check(shape, value, str(path), errors, root)
    if errors:
        raise SystemExit("\n".join(errors))


def pinned(directory, asset):
    path = directory / asset["path"]
    if not path.resolve().is_relative_to(directory.resolve()):
        raise SystemExit(f"Asset outside package: {path}")
    if hashlib.sha256(path.read_bytes()).hexdigest() != asset["sha256"]:
        raise SystemExit(f"SHA-256 mismatch: {path}")
    return path


for directory in map(pathlib.Path, sys.argv[2:]):
    manifest = read_json(directory / "manifest.json")
    validate(manifest_schema, manifest, manifest_schema, directory / "manifest.json")
    pinned(directory, manifest["worker"])
    for app in manifest["apps"]:
        path = pinned(directory, app["description"]["template"])
        validate({"$ref": "#/$defs/AppDescription"}, read_json(path), schema, path)
    for contribution in manifest.get("contributions", []):
        if contribution["kind"] == "harness":
            pinned(directory, contribution["harness"]["entry"])
    print(f"PASS {manifest['id']} draft schema shapes and asset pins")

if len(sys.argv) == 2:
    for name, value in json.load(sys.stdin):
        validate({"$ref": "#/$defs/" + name}, value, CATALOGUE, name)
