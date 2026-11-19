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

def matches(node, value, ctx):
    sub = []
    check(node, value, "$", sub, ctx)
    return not sub

def check(node, value, path, errors, ctx=None):
    """Validates `value` against `node`: the JSON Schema keywords the contract and the host catalogue use, including boolean schemas, not, if/then/else, allOf,
    anyOf, oneOf and dependentRequired (the catalogue's ActionOutcome and ActivationResolution rely on them)."""
    ctx = ctx if ctx is not None else schema
    if node is True: return
    if node is False: errors.append(f"{path}: not allowed here"); return
    if "$ref" in node:
        target, ctx2 = resolve(node["$ref"], ctx)
        check(target, value, path, errors, ctx2)
    for branch in node.get("allOf", []): check(branch, value, path, errors, ctx)
    if "anyOf" in node and not any(matches(b, value, ctx) for b in node["anyOf"]): errors.append(f"{path}: matches none of the anyOf branches")
    if "oneOf" in node:
        results = []
        for b in node["oneOf"]:
            sub = []
            check(b, value, path, sub, ctx)
            results.append(sub)
        passing = sum(1 for r in results if not r)
        if passing != 1:
            closest = min(results, key=len) if passing == 0 else []
            errors.append(f"{path}: matches {passing} of the oneOf branches (needs exactly 1)" + (f"; closest branch: {'; '.join(closest[:3])}" if closest else ""))
    if "not" in node and matches(node["not"], value, ctx): errors.append(f"{path}: matches a forbidden shape")
    if "if" in node:
        branch = node.get("then") if matches(node["if"], value, ctx) else node.get("else")
        if branch is not None: check(branch, value, path, errors, ctx)
    t = node.get("type")
    kinds = {"object": dict, "array": list, "string": str, "boolean": bool, "null": type(None)}
    for one in (t if isinstance(t, list) else [t]) if t else []:
        pass
    if t:
        ok_type = False
        for one in (t if isinstance(t, list) else [t]):
            if one == "integer": ok_type |= isinstance(value, int) and not isinstance(value, bool)
            elif one == "number": ok_type |= isinstance(value, (int, float)) and not isinstance(value, bool)
            elif one in kinds: ok_type |= isinstance(value, kinds[one]) and (one == "boolean" or not isinstance(value, bool))
        if not ok_type: errors.append(f"{path}: expected {t}"); return
    if "const" in node and value != node["const"]: errors.append(f"{path}: must be {node['const']!r}")
    if "enum" in node and value not in node["enum"]: errors.append(f"{path}: {value!r} not in {node['enum']}")
    if isinstance(value, str):
        if "pattern" in node and not re.search(node["pattern"], value): errors.append(f"{path}: {value!r} does not match {node['pattern']}")
        if "maxLength" in node and len(value.encode()) > node["maxLength"]: errors.append(f"{path}: longer than {node['maxLength']} bytes")
        if "minLength" in node and len(value) < node["minLength"]: errors.append(f"{path}: shorter than {node['minLength']}")
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
        for key, needs in node.get("dependentRequired", {}).items():
            if key in value:
                for other in needs:
                    if other not in value: errors.append(f"{path}: {key} needs {other}")
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
    root_node = (doc.get("view") or {}).get("root") if isinstance(doc, dict) else None
    if isinstance(root_node, dict):
        walk(root_node, 1)
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
def walk(node, path="$"):
    """Yields (path, node) for every dict and list in the document."""
    yield path, node
    if isinstance(node, dict):
        for k, v in node.items():
            yield from walk(v, f"{path}.{k}")
    elif isinstance(node, list):
        for i, v in enumerate(node):
            yield from walk(v, f"{path}[{i}]")

def field_names(kind_entry, generic=()):
    fields = kind_entry.get("fields") or kind_entry.get("indexed_fields") or {}
    names = set(fields if isinstance(fields, dict) else [f.get("name") if isinstance(f, dict) else f for f in fields])
    return names | set(generic)

def normalize_manifest(m):
    """A fixture manifest is either a real plugin manifest v2 (`kinds` and `operations` are ARRAYS with `id`; `indexed_fields` maps a field to {type, audience} or to a bare type;
    `reads` lists the host kinds the package may bind; an operation's `input_schema` may list `properties` and `required`) or the older small map form (`kinds` and `operations` keyed
    by id with `fields`). Both are normalized to the keyed form the checks use: `kinds[id].fields`, `operations[id]`, `reads`."""
    if not isinstance(m, dict): return m
    out = dict(m)
    kinds = m.get("kinds", {})
    if isinstance(kinds, list):
        out["kinds"] = {k.get("id"): {**{x: y for x, y in k.items() if x not in ("indexed_fields",)},
                                       "fields": {f: (v if isinstance(v, dict) else {"type": v}) for f, v in (k.get("indexed_fields") or {}).items()}} for k in kinds if isinstance(k, dict)}
    ops = m.get("operations", {})
    if isinstance(ops, list):
        out["operations"] = {o.get("id"): o for o in ops if isinstance(o, dict)}
    out["_real"] = isinstance(kinds, list) or "api_version" in m
    return out

def semantic(doc, errors, catalogue, manifest):
    """Unique action ids per document; bind, form, draft and row source scope; fullSwipe placement and catalogue permission; operations, kinds and fields resolved against
    the host catalogue and the fixture's own manifest (a plugin's kinds and operations); a header title bound to an item bind."""
    if not isinstance(doc, dict): return
    binds_list = [b for b in doc.get("binds", []) if isinstance(b, dict)]
    binds = {b.get("id"): b for b in binds_list}
    forms, composers, action_ids = {}, {}, {}
    cat_ops = (catalogue or {}).get("operations", {})
    cat_kinds = (catalogue or {}).get("kinds", {})
    man_ops = (manifest or {}).get("operations", {})
    man_kinds = (manifest or {}).get("kinds", {})
    for path, node in walk(doc):
        if isinstance(node, dict) and node.get("type") == "form" and "id" in node: forms[node["id"]] = path
        if isinstance(node, dict) and node.get("type") == "composer" and "id" in node: composers[node["id"]] = path
    for path, node in walk(doc):
        if not isinstance(node, dict): continue
        if "operation" in node and "id" in node and isinstance(node.get("operation"), str):
            if node["id"] in action_ids: errors.append(f"{path}: duplicate action id {node['id']!r} (first at {action_ids[node['id']]})")
            action_ids[node["id"]] = path
            op = node["operation"]
            entry = cat_ops.get(op) if catalogue is not None else None
            if node.get("fullSwipe"):
                if not re.search(r"(leadingActions|trailingActions)\[0\]$", path):
                    errors.append(f"{path}: fullSwipe is allowed only on the first action of a swipe set")
                if catalogue is not None and not (entry and entry.get("full_swipe", False)):
                    errors.append(f"{path}: fullSwipe is rejected on {op}: the catalogue does not allow it (plugin operations never may)")
            if catalogue is not None:
                host_like = op.startswith("host.") or op in ("turns.start",) or op in cat_ops
                if host_like:
                    if entry is None: errors.append(f"{path}: {op} is not in the host catalogue")
                    else:
                        if not entry.get("generic_invoke", False): errors.append(f"{path}: {op} is not a generic invoke in the catalogue (surfaces: {entry.get('surfaces')})")
                        schema_in = (entry.get("input_schema") or {}).get("properties", {})
                        given = node.get("inputs", {})
                        for name in given:
                            if schema_in and name not in schema_in: errors.append(f"{path}: {op} has no input {name!r}")
                        for name in (entry.get("input_schema") or {}).get("required", []):
                            if name not in given: errors.append(f"{path}: {op} needs input {name!r}")
                        for name in entry.get("target_inputs", []):
                            if isinstance(given.get(name), dict) and "literal" in given[name]: errors.append(f"{path}: target input {name!r} of {op} cannot be a literal")
                elif manifest is not None and op not in man_ops:
                    errors.append(f"{path}: operation {op!r} is in neither the host catalogue nor the fixture manifest")
                elif manifest is not None:
                    declared = man_ops[op].get("input_schema") if isinstance(man_ops[op], dict) else None
                    if isinstance(declared, dict) and isinstance(declared.get("properties"), dict):
                        given = node.get("inputs", {})
                        for name in given:
                            if name not in declared["properties"]: errors.append(f"{path}: {op} (manifest) has no input {name!r}")
                        for name in declared.get("required", []):
                            if name not in given: errors.append(f"{path}: {op} (manifest) needs input {name!r}")
        if "bind" in node and isinstance(node["bind"], str) and node["bind"] not in binds:
            errors.append(f"{path}: bind {node['bind']!r} is not declared in `binds`")
        if "bind" in node and "field" in node and set(node) == {"bind", "field"} and ".config" not in path and ".header.title" not in path and ".row" not in path:
            errors.append(f"{path}: a {{bind, field}} source is legal only in a host component's config (and a header title)")
        if "row" in node and set(node) <= {"row", "field"} and ".row" not in path:
            errors.append(f"{path}: a {{row}} source is only valid inside a bind's row template")
        if "form" in node and set(node) == {"form", "field"}:
            owner = forms.get(node["form"])
            if owner is None: errors.append(f"{path}: form {node['form']!r} does not exist")
            elif not path.startswith(owner + ".submit"): errors.append(f"{path}: a {{form}} source is only valid inside that form's own submit")
        if "draft" in node and set(node) == {"draft"} and isinstance(node["draft"], str):
            owner = composers.get(node["draft"])
            if owner is None: errors.append(f"{path}: composer {node['draft']!r} does not exist")
            elif not path.startswith(owner + ".config"): errors.append(f"{path}: a {{draft}} source is only valid inside that composer's own config")
    if manifest is not None:
        reads = set(manifest.get("reads", []) or [])
        for index, bind in enumerate(binds_list):
            kind_id = (bind.get("source") or {}).get("kind")
            if isinstance(kind_id, str) and kind_id not in reads and (manifest.get("_real") or "reads" in manifest):
                errors.append(f"$.binds[{index}].source: kind {kind_id!r} is not in the manifest's `reads` (every bind needs it, including a package's own kinds)")
    parent_binds = {i for i, b in binds.items() if (b.get("source") or {}).get("parent")}
    children_edges = {}
    for bind_id, b in binds.items():
        child = ((b.get("row") or {}).get("children") or {}).get("bind")
        if child is not None:
            if child not in parent_binds: errors.append(f"$.binds {bind_id}: children bind {child!r} must declare `source.parent: true`")
            children_edges[bind_id] = child
    for sec_path, sec in [(p_, n_) for p_, n_ in walk(doc) if isinstance(n_, dict) and "repeat" in n_ and "id" in n_]:
        rep = sec["repeat"]
        for key in ("over", "rows"):
            if rep.get(key) not in binds: errors.append(f"{sec_path}: repeat.{key} {rep.get(key)!r} is not a declared bind")
        if rep.get("rows") in binds and rep["rows"] not in parent_binds: errors.append(f"{sec_path}: repeat.rows must be a bind with `source.parent: true`")
        if rep.get("over") in parent_binds: errors.append(f"{sec_path}: repeat.over must be a top-level bind, not a per-parent one")
        if "bind" in sec or sec.get("rows"): errors.append(f"{sec_path}: a repeated section has no `bind` or literal `rows` of its own")
    def depth(bind_id, seen=()):
        if bind_id in seen: errors.append(f"$.binds: children cycle through {bind_id!r}"); return 0
        nxt = children_edges.get(bind_id)
        return 1 + depth(nxt, seen + (bind_id,)) if nxt else 1
    for bind_id in binds:
        if depth(bind_id) > 3: errors.append(f"$.binds {bind_id}: nested rows deeper than 3 levels")
    for index, b in enumerate(binds_list):
        uses_parent = any(isinstance(pr.get("value"), dict) and "parent" in pr["value"] for g in ("all", "any") for pr in (b.get("filter") or {}).get(g, []))
        if uses_parent and not (b.get("source") or {}).get("parent"): errors.append(f"$.binds[{index}]: {{parent}} is legal only in a bind whose source has `parent: true`")
    header_title = (doc.get("chrome") or {}).get("header", {}).get("title")
    if isinstance(header_title, dict) and header_title.get("bind") in binds and binds[header_title["bind"]].get("source", {}).get("type") != "item":
        errors.append("$.chrome.header.title: a header title takes its text from an `item` bind (the open item), not from a list's first row")
    for index, bind in enumerate(binds_list):
        if catalogue is None: break
        source = bind.get("source", {})
        kind_id = source.get("kind")
        path = f"$.binds[{index}].source"
        if not isinstance(kind_id, str): continue
        kind = cat_kinds.get(kind_id) if catalogue is not None else None
        plugin = man_kinds.get(kind_id) if manifest is not None else None
        if catalogue is not None and kind is None and plugin is None:
            if manifest is not None or kind_id in ("notes", "threads", "collections", "requests"):
                errors.append(f"{path}: kind {kind_id!r} is in neither the host catalogue nor the fixture manifest")
            continue
        entry = kind or plugin
        if entry is None: continue
        generic = (catalogue or {}).get("generic_item_fields", {})
        known = field_names(entry, generic) | {"id"}
        used = list(bind.get("fields", [])) + [x["field"] for g in ("all", "any") for x in bind.get("filter", {}).get(g, [])] + [x["field"] for x in bind.get("sort", [])]
        for f in used:
            if f not in known: errors.append(f"{path}: field {f!r} is not a field of kind {kind_id!r}")
        for g in ("all", "any"):
            for pred in bind.get("filter", {}).get(g, []):
                meta = (entry.get("fields") or {}).get(pred["field"], {}) if isinstance(entry.get("fields"), dict) else {}
                if isinstance(meta, dict) and meta.get("audience") == "viewer_self" and not (pred.get("op") == "eq" and pred.get("value") == {"viewer": "person"}):
                    errors.append(f"{path}: {pred['field']!r} is viewer-only: it accepts only `eq` with {{viewer: person}}")

failed = False
manifests_dir = root / "fixtures" / "description-v2" / "manifests"
print("host catalogue: " + ("loaded" if CATALOGUE is not None else "not present yet (plugins lane); operation, kind and field names are not resolved"))
for file in sorted((root / "fixtures" / "description-v2").glob("*.json")):
    if CATALOGUE is None and "-catalogue-" in file.name:
        print(f"skip {file.name}  (needs the host catalogue)")
        continue
    errors = []
    try:
        doc = load_strict(file.read_text())
    except (DuplicateKey, ValueError) as error:
        errors.append(str(error)); doc = None
    manifest = None
    manifest_file = manifests_dir / file.name.replace("invalid-", "")
    if manifest_file.exists():
        manifest = normalize_manifest(load_strict(manifest_file.read_text()))
    is_event = file.name.startswith(("event-", "invalid-event-"))
    if doc is not None:
        check({"$ref": "#/$defs/Event"} if is_event else schema, doc, "$", errors)
        if not is_event:
            view_limits(doc, errors)
            semantic(doc, errors, CATALOGUE, manifest)
    expect_invalid = file.name.startswith("invalid-")
    ok = bool(errors) if expect_invalid else not errors
    print(("ok   " if ok else "FAIL ") + file.name + ("" if ok else "  " + "; ".join(errors[:4])))
    failed |= not ok
sys.exit(1 if failed else 0)
