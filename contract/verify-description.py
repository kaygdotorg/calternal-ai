#!/usr/bin/env python3
"""Validates contract/fixtures/description-v2/*.json against contract/description-v2.schema.json.

A small validator for the keywords the schema uses (type, properties, required, additionalProperties, enum, const, pattern, minimum, maximum,
minItems, maxItems, uniqueItems, maxLength, maxProperties, oneOf, $ref), so the contract can be checked with the standard library alone.
Files named `invalid-*.json` must FAIL validation; every other file must pass. Run from the repository root: python3 contract/verify-description.py
"""
import json, os, re, sys, pathlib

root = pathlib.Path(__file__).resolve().parent
schema = json.loads((root / "description-v2.schema.json").read_text())

def load_catalogue():
    path = pathlib.Path(os.environ.get("CALTERNAL_HOST_CATALOGUE", root / "host-catalogue-v2.json"))
    return json.loads(path.read_text()) if path.exists() else None
CATALOGUE = load_catalogue()

def resolve(ref, ctx):
    """A `#/...` reference stays in the document it is read in; `host-catalogue-v2.json#/...` is the plugins lane's catalogue (the shapes of the activation protocol are theirs, cited here, never copied). Without the catalogue a cited shape matches nothing (fixtures named `-catalogue-` are skipped without it)."""
    if ref.startswith("host-catalogue-v2.json#") or ref.startswith("https://calternal.cloud/contract/host-catalogue-v2.json#"):
        if CATALOGUE is None: return {"const": "__host_catalogue_missing__"}, schema   # fails to match: fixtures that need the catalogue are skipped without it
        ctx, ref = CATALOGUE, ref.split("#", 1)[1]
    node = ctx
    try:
        for part in ref.removeprefix("#/").removeprefix("/").split("/"):
            node = node[part]
    except KeyError:
        return {"const": "__host_catalogue_def_missing__"}, schema   # a catalogue copy older than this schema: the cited shape is not there yet
    return node, ctx

def check(node, value, path, errors, ctx=None):
    ctx = ctx if ctx is not None else schema
    if "$ref" in node:
        target, ctx2 = resolve(node["$ref"], ctx)
        return check(target, value, path, errors, ctx2)
    if "oneOf" in node:
        passing = 0
        for branch in node["oneOf"]:
            sub = []
            check(branch, value, path, sub, ctx)
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
            for i, v in enumerate(value): check(node["items"], v, f"{path}[{i}]", errors, ctx)
    if isinstance(value, dict):
        for key in node.get("required", []):
            if key not in value: errors.append(f"{path}: missing {key}")
        props = node.get("properties", {})
        extra = node.get("additionalProperties", True)
        if "maxProperties" in node and len(value) > node["maxProperties"]: errors.append(f"{path}: more than {node['maxProperties']} properties")
        for key, v in value.items():
            if key in props: check(props[key], v, f"{path}.{key}", errors, ctx)
            elif extra is False: errors.append(f"{path}: unknown field {key}")
            elif isinstance(extra, dict): check(extra, v, f"{path}.{key}", errors, ctx)

def view_limits(doc, errors):
    """Limits a JSON schema cannot state: a view's content tree is at most 6 deep and 200 nodes (docs/description-contract.md, section 5.0)."""
    count = 0
    def walk(node, depth):
        nonlocal count
        count += 1
        if depth > 6: errors.append("$.root: deeper than 6 levels"); return
        for child in node.get("children", []) if isinstance(node, dict) else []: walk(child, depth + 1)
    if isinstance(doc, dict) and doc.get("kind") == "view" and isinstance(doc.get("root"), dict):
        walk(doc["root"], 1)
        if count > 200: errors.append(f"$.root: {count} nodes (at most 200)")


# ---- strict parsing -------------------------------------------------------------------------------------------------------------------------------------
class DuplicateKey(Exception):
    pass

def strict_pairs(pairs):
    keys = [k for k, _ in pairs]
    dup = {k for k in keys if keys.count(k) > 1}
    if dup:
        raise DuplicateKey(f"duplicate key {sorted(dup)[0]!r}")
    return dict(pairs)

def load_strict(text):
    """The host's parser must reject duplicate keys (a duplicate would let two readers disagree about a field); so does this one."""
    return json.loads(text, object_pairs_hook=strict_pairs)

# ---- semantic checks the schema cannot say ----------------------------------------------------------------------------------------------------------------
RESERVED_FOR_COMPONENTS = {"host.approval.resolve", "host.undo", "host.action.status", "host.panel.ask", "host.route.set", "host.session.end"}

def walk(node, path="$"):
    """Yields (path, node) for every dict and list in the document."""
    yield path, node
    if isinstance(node, dict):
        for k, v in node.items():
            yield from walk(v, f"{path}.{k}")
    elif isinstance(node, list):
        for i, v in enumerate(node):
            yield from walk(v, f"{path}[{i}]")

def semantic(doc, errors, catalogue):
    """Unique action ids per document; bind, form, draft and row source scope; fullSwipe placement; operation and field names against the host catalogue."""
    binds = {b.get("id") for b in doc.get("binds", []) if isinstance(b, dict)}
    forms, composers, action_ids = {}, {}, {}
    for path, node in walk(doc):
        if not isinstance(node, dict):
            continue
        if node.get("type") == "form" and "id" in node: forms[node["id"]] = path
        if node.get("type") == "composer" and "id" in node: composers[node["id"]] = path
    for path, node in walk(doc):
        if isinstance(node, dict) and "operation" in node and "id" in node and isinstance(node.get("operation"), str):
            if node["id"] in action_ids:
                errors.append(f"{path}: duplicate action id {node['id']!r} (first at {action_ids[node['id']]})")
            action_ids[node["id"]] = path
            op = node["operation"]
            if op in RESERVED_FOR_COMPONENTS and not (op == "host.panel.ask" and ".onQuote" in path):
                errors.append(f"{path}: {op} cannot be named by a generic invoke")
            if node.get("fullSwipe") and not re.search(r"(leadingActions|trailingActions)\[0\]$", path):
                errors.append(f"{path}: fullSwipe is allowed only on the first action of a swipe set")
            if catalogue is not None:
                ops = catalogue.get("operations", {})
                entry = ops.get(op)
                if op.startswith("host.") or op == "turns.start":
                    if entry is None:
                        errors.append(f"{path}: {op} is not in the host catalogue")
                    else:
                        in_surface = (op == "host.panel.ask" and ".onQuote" in path) or (op == "turns.start" and ".send" in path)
                        if not entry.get("generic_invoke", False) and not in_surface:
                            errors.append(f"{path}: {op} is not a generic invoke in the catalogue (surfaces: {entry.get('surfaces')})")
                        if node.get("fullSwipe") and not entry.get("full_swipe", False):
                            errors.append(f"{path}: {op} may not be a full swipe (catalogue full_swipe is false)")
                        schema_in = (entry.get("input_schema") or {}).get("properties", {})
                        given = node.get("inputs", {})
                        for name in given:
                            if schema_in and name not in schema_in: errors.append(f"{path}: {op} has no input {name!r}")
                        for name in (entry.get("input_schema") or {}).get("required", []):
                            if name not in given: errors.append(f"{path}: {op} needs input {name!r}")
                        for name in entry.get("target_inputs", []):
                            if isinstance(given.get(name), dict) and "literal" in given[name]:
                                errors.append(f"{path}: target input {name!r} of {op} cannot be a literal")
        if isinstance(node, dict):
            if "bind" in node and isinstance(node["bind"], str) and node["bind"] not in binds:
                errors.append(f"{path}: bind {node['bind']!r} is not declared in `binds`")
            if "row" in node and set(node) <= {"row", "field"} and ".row." not in path + "." and ".row" not in path:
                errors.append(f"{path}: a {{row}} source is only valid inside a bind's row template")
            if "form" in node and set(node) == {"form", "field"}:
                owner = forms.get(node["form"])
                if owner is None: errors.append(f"{path}: form {node['form']!r} does not exist")
                elif not path.startswith(owner + ".submit"): errors.append(f"{path}: a {{form}} source is only valid inside that form's own submit")
            if "draft" in node and set(node) == {"draft"} and isinstance(node["draft"], str):
                owner = composers.get(node["draft"])
                if owner is None: errors.append(f"{path}: composer {node['draft']!r} does not exist")
                elif not path.startswith(owner + ".config"): errors.append(f"{path}: a {{draft}} source is only valid inside that composer's own config")
            if catalogue is not None and node.get("kind") and isinstance(node.get("kind"), str) and path.endswith(".source") and "type" in node:
                kinds = catalogue.get("kinds", {})
                kind = kinds.get(node["kind"])
                bind = bind_at(doc, path)
                if kind is None:
                    if node["kind"] in {"notes", "threads", "collections", "requests"} or not plugin_kind(node["kind"]):
                        errors.append(f"{path}: kind {node['kind']!r} is not in the host catalogue")
                elif bind is not None:
                    known = set(kind.get("fields", {})) | {"id"}
                    used = list(bind.get("fields", [])) + [p_["field"] for g in ("all", "any") for p_ in bind.get("filter", {}).get(g, [])] + [x["field"] for x in bind.get("sort", [])]
                    for f in used:
                        if f not in known: errors.append(f"{path}: field {f!r} is not a field of kind {node['kind']!r}")
                    for g in ("all", "any"):
                        for pred in bind.get("filter", {}).get(g, []):
                            meta = kind.get("fields", {}).get(pred["field"], {})
                            if meta.get("audience") == "viewer_self" and not (pred.get("op") == "eq" and pred.get("value") == {"viewer": "person"}):
                                errors.append(f"{path}: {pred['field']!r} is viewer-only: it accepts only `eq` with {{viewer: person}}")

def bind_at(doc, path):
    m = re.match(r"^\$\.binds\[(\d+)\]\.source$", path)
    return doc["binds"][int(m.group(1))] if m else None

def plugin_kind(kind):
    return "-" in kind or "." in kind   # a plugin's own kind (reading-item); host kinds have no dash and are in the catalogue

failed = False
import os
catalogue_path = pathlib.Path(os.environ.get("CALTERNAL_HOST_CATALOGUE", root / "host-catalogue-v2.json"))
catalogue = json.loads(catalogue_path.read_text()) if catalogue_path.exists() else None
print("host catalogue: " + ("contract/host-catalogue-v2.json" if catalogue is not None else "not present yet (plugins lane); operation and kind names are not resolved"))
for file in sorted((root / "fixtures" / "description-v2").glob("*.json")):
    if catalogue is None and "-catalogue-" in file.name:
        print(f"skip {file.name}  (needs the host catalogue)")
        continue
    errors = []
    try:
        doc = load_strict(file.read_text())
    except (DuplicateKey, ValueError) as error:
        errors.append(str(error)); doc = None
    is_event = file.name.startswith(("event-", "invalid-event-"))
    if doc is not None:
        check({"$ref": "#/$defs/Event"} if is_event else schema, doc, "$", errors)
        if not is_event:
            view_limits(doc, errors)
            semantic(doc, errors, catalogue)
    expect_invalid = file.name.startswith("invalid-")
    ok = bool(errors) if expect_invalid else not errors
    print(("ok   " if ok else "FAIL ") + file.name + ("" if ok else "  " + "; ".join(errors[:4])))
    failed |= not ok
sys.exit(1 if failed else 0)
