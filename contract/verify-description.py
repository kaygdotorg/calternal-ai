#!/usr/bin/env python3
"""Validates contract/fixtures/description-v2/*.json against contract/description-v2.schema.json.

A small validator for the keywords the schema uses (type, properties, required, additionalProperties, enum, const, pattern, minimum, maximum,
minItems, maxItems, uniqueItems, maxLength, maxProperties, oneOf, $ref), so the contract can be checked with the standard library alone.
Files named `invalid-*.json` must FAIL validation; every other file must pass. Run from the repository root: python3 contract/verify-description.py
"""
import json, re, sys, pathlib

root = pathlib.Path(__file__).resolve().parent
schema = json.loads((root / "description-v2.schema.json").read_text())

def resolve(ref):
    node = schema
    for part in ref.removeprefix("#/").split("/"):
        node = node[part]
    return node

def check(node, value, path, errors):
    if "$ref" in node:
        return check(resolve(node["$ref"]), value, path, errors)
    if "oneOf" in node:
        passing = 0
        for branch in node["oneOf"]:
            sub = []
            check(branch, value, path, sub)
            if not sub: passing += 1
        if passing != 1: errors.append(f"{path}: matches {passing} of the oneOf branches (needs exactly 1)")
        return
    t = node.get("type")
    kinds = {"object": dict, "array": list, "string": str, "boolean": bool, "null": type(None)}
    if t == "integer":
        if not (isinstance(value, int) and not isinstance(value, bool)): errors.append(f"{path}: expected integer"); return
    elif t == "number":
        if not (isinstance(value, (int, float)) and not isinstance(value, bool)): errors.append(f"{path}: expected number"); return
    elif t in kinds:
        if not isinstance(value, kinds[t]) or (t != "boolean" and isinstance(value, bool)): errors.append(f"{path}: expected {t}"); return
    if "const" in node and value != node["const"]: errors.append(f"{path}: must be {node['const']!r}")
    if "enum" in node and value not in node["enum"]: errors.append(f"{path}: {value!r} not in {node['enum']}")
    if isinstance(value, str):
        if "pattern" in node and not re.search(node["pattern"], value): errors.append(f"{path}: {value!r} does not match {node['pattern']}")
        if "maxLength" in node and len(value.encode()) > node["maxLength"]: errors.append(f"{path}: longer than {node['maxLength']} bytes")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in node and value < node["minimum"]: errors.append(f"{path}: below {node['minimum']}")
        if "maximum" in node and value > node["maximum"]: errors.append(f"{path}: above {node['maximum']}")
    if isinstance(value, list):
        if "minItems" in node and len(value) < node["minItems"]: errors.append(f"{path}: fewer than {node['minItems']} items")
        if "maxItems" in node and len(value) > node["maxItems"]: errors.append(f"{path}: more than {node['maxItems']} items")
        if node.get("uniqueItems") and len({json.dumps(v, sort_keys=True) for v in value}) != len(value): errors.append(f"{path}: items not unique")
        if "items" in node:
            for i, v in enumerate(value): check(node["items"], v, f"{path}[{i}]", errors)
    if isinstance(value, dict):
        for key in node.get("required", []):
            if key not in value: errors.append(f"{path}: missing {key}")
        props = node.get("properties", {})
        extra = node.get("additionalProperties", True)
        if "maxProperties" in node and len(value) > node["maxProperties"]: errors.append(f"{path}: more than {node['maxProperties']} properties")
        for key, v in value.items():
            if key in props: check(props[key], v, f"{path}.{key}", errors)
            elif extra is False: errors.append(f"{path}: unknown field {key}")
            elif isinstance(extra, dict): check(extra, v, f"{path}.{key}", errors)

failed = False
for file in sorted((root / "fixtures" / "description-v2").glob("*.json")):
    errors = []
    check(schema, json.loads(file.read_text()), "$", errors)
    expect_invalid = file.name.startswith("invalid-")
    ok = bool(errors) if expect_invalid else not errors
    print(("ok   " if ok else "FAIL ") + file.name + ("" if ok else "  " + "; ".join(errors[:4])))
    failed |= not ok
sys.exit(1 if failed else 0)
