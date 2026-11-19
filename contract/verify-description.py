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
    children_edges = {b_id: ((b.get("row") or {}).get("children") or {}).get("bind") for b_id, b in binds.items() if ((b.get("row") or {}).get("children") or {}).get("bind")}
    repeat_rows = {n_["repeat"]["rows"] for _, n_ in walk(doc) if isinstance(n_, dict) and isinstance(n_.get("repeat"), dict) and "id" in n_ and "rows" in n_["repeat"]}
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
                        input_schema = entry.get("input_schema") or {}
                        base_props = input_schema.get("properties", {})
                        base_required = list(input_schema.get("required", []))
                        given = node.get("inputs", {})
                        branches = input_schema.get("oneOf") or input_schema.get("anyOf")
                        schema_in, required = dict(base_props), base_required
                        if branches:
                            # the branch is selected by the inputs given: its required keys must all be present and every given key must be allowed by it
                            fits = []
                            for branch in branches:
                                props = {**base_props, **(branch.get("properties") or {})}
                                req = base_required + list(branch.get("required", []))
                                if all(r in given for r in req) and all(k in props for k in given): fits.append((props, req))
                            wants_one = "oneOf" in input_schema
                            if (wants_one and len(fits) != 1) or (not wants_one and not fits):
                                errors.append(f"{path}: {op} inputs {sorted(given)} fit {len(fits)} of the input schema's branches")
                            if fits: schema_in, required = fits[0]
                        for name in given:
                            if schema_in and name not in schema_in: errors.append(f"{path}: {op} has no input {name!r}")
                        for name in required:
                            if name not in given: errors.append(f"{path}: {op} needs input {name!r}")
                        owner_bind = None
                        m_owner = re.match(r"^\$\.binds\[(\d+)\]", path)
                        if m_owner: owner_bind = binds_list[int(m_owner.group(1))]
                        owner_kind = ((owner_bind or {}).get("source") or {}).get("kind")
                        owner_in_repeat = (owner_bind or {}).get("id") in repeat_rows
                        for name, source in given.items():
                            declared_type = ((schema_in.get(name) or {}).get("$ref") or "").rsplit("/", 1)[-1]
                            ctx_kind = source.get("context") if isinstance(source, dict) and set(source) == {"context"} else None
                            if declared_type == "LayoutEntryRef" and ctx_kind != "placement": errors.append(f"{path}: input {name!r} of {op} is a LayoutEntryRef: it takes {{context: placement}}")
                            if declared_type == "ThreadRef":
                                if ctx_kind == "placement": errors.append(f"{path}: input {name!r} of {op} is a ThreadRef: it cannot take {{context: placement}}")
                                if owner_in_repeat and ctx_kind != "item": errors.append(f"{path}: in the rows of a Section.repeat the row id is a placement (LayoutEntryRef): the ThreadRef input {name!r} of {op} must be {{context: item}}, not the bare row id")
                                if ctx_kind == "item" and owner_kind != "threads": errors.append(f"{path}: {{context: item}} fills a ThreadRef only from a row of kind `threads`, not {owner_kind!r}")
                            if ctx_kind == "placement" and declared_type not in ("LayoutEntryRef", ""): errors.append(f"{path}: {{context: placement}} fills a LayoutEntryRef, not {declared_type}")
                            if ctx_kind == "item" and declared_type == "LayoutEntryRef": errors.append(f"{path}: {{context: item}} cannot fill the LayoutEntryRef input {name!r}")
                            REF_TYPES = {k.get("row_ref") for k in cat_kinds.values() if k.get("row_ref")} | {"ItemRef", "LayoutEntryRef"}   # every ref a row can be or give
                            move_to_destination = (op, name) == ("host.layout.move_to", "section")   # a destination choice, checked below
                            row_ref_of_owner = (cat_kinds.get(owner_kind) or {}).get("row_ref") if owner_kind in cat_kinds else ("ItemRef" if owner_kind else None)
                            if declared_type in REF_TYPES and ctx_kind == "item" and declared_type != row_ref_of_owner and not move_to_destination:
                                errors.append(f"{path}: {{context: item}} gives the row's own {row_ref_of_owner} and cannot fill the {declared_type} input {name!r} of {op}")
                            strict_thread = (op, name) == ("host.layout.move_to", "thread")
                            if strict_thread and not (ctx_kind == "item" or (isinstance(source, dict) and source == {"row": "id"})):
                                errors.append(f"{path}: the `thread` of host.layout.move_to takes only {{context: item}} or the bare id of a threads row, not {json.dumps(source, sort_keys=True)}")
                            if declared_type in REF_TYPES and ctx_kind is None and not move_to_destination:
                                bare_id = isinstance(source, dict) and source == {"row": "id"}
                                from_route = isinstance(source, dict) and set(source) == {"route"}
                                row_ref = (cat_kinds.get(owner_kind) or {}).get("row_ref") if owner_kind in cat_kinds else ("ItemRef" if owner_kind else None)
                                prompt = source.get("prompt") if isinstance(source, dict) else None
                                choice_ok = False
                                if isinstance(prompt, dict) and prompt.get("kind") == "choice":
                                    option_bind = binds.get(((prompt.get("options") or {}).get("bind")))
                                    choice_ok = (cat_kinds.get(((option_bind or {}).get("source") or {}).get("kind")) or {}).get("row_ref") == declared_type
                                if strict_thread: from_route = choice_ok = False
                                if not (from_route or choice_ok or (bare_id and row_ref == declared_type and not owner_in_repeat)):
                                    errors.append(f"{path}: the {declared_type} input {name!r} of {op} takes only a source of that ref type ({{context}}, a bare row id of a row of that type, or a route), not {json.dumps(source, sort_keys=True)}")
                            if declared_type in ("ChoiceRef", "LayoutSectionRef") and (declared_type == "ChoiceRef" or name == "section"):
                                prompt = source.get("prompt") if isinstance(source, dict) else None
                                if not (isinstance(prompt, dict) and prompt.get("kind") == "choice"):
                                    errors.append(f"{path}: input {name!r} of {op} is a destination choice: it must come from a `choice` prompt over the authorized options, not a literal or a row value")
                                else:
                                    option_bind = binds.get(((prompt.get("options") or {}).get("bind")))
                                    option_kind = cat_kinds.get(((option_bind or {}).get("source") or {}).get("kind"))
                                    want = "LayoutSectionRef" if (declared_type == "LayoutSectionRef" or name == "section") else None
                                    if want and (option_kind is None or option_kind.get("row_ref") != want):
                                        errors.append(f"{path}: the choice for {name!r} of {op} must range over a bind whose kind has row_ref {want} (layout_sections), not {((option_bind or {}).get('source') or {}).get('kind')!r}")
                            if declared_type == "ThreadRef" and isinstance(source, dict) and source.get("row") == "id" and not owner_in_repeat:
                                if (cat_kinds.get(owner_kind) or {}).get("row_ref") != "ThreadRef":
                                    errors.append(f"{path}: the bare row id fills a ThreadRef only in a bind whose kind has row_ref ThreadRef (threads), not {owner_kind!r}")
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
    for index, b in enumerate(binds_list):
        if not b.get("persist"): continue
        src = b.get("source") or {}
        for key in ("ref", "text", "parent"):
            if key in src: errors.append(f"$.binds[{index}]: persist needs INVARIANT query inputs: the source has `{key}`")
        if "collection" in src and not (isinstance(src["collection"], dict) and set(src["collection"]) == {"literal"}):
            errors.append(f"$.binds[{index}]: persist allows only a LITERAL collection restriction, not {json.dumps(src['collection'], sort_keys=True)}")
        if src.get("type") != "list": errors.append(f"$.binds[{index}]: persist needs a `list` source")
        for g in ("all", "any"):
            for pr in (b.get("filter") or {}).get(g, []):
                value = pr.get("value")
                if value is not None and not (isinstance(value, dict) and (set(value) == {"literal"} or value == {"viewer": "person"})):
                    errors.append(f"$.binds[{index}]: persist needs INVARIANT query inputs: the filter operand {json.dumps(value, sort_keys=True)} on {pr.get('field')!r} is not a literal or the viewer")
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
    referenced = set(children_edges.values())
    for sec_path, sec in [(p_, n_) for p_, n_ in walk(doc) if isinstance(n_, dict) and "repeat" in n_ and "id" in n_]:
        rep = sec["repeat"]
        if rep.get("rows") in binds: referenced.add(rep["rows"])
        over = binds.get(rep.get("over"))
        if over is not None and (over.get("source") or {}).get("kind") != "layout_sections":
            errors.append(f"{sec_path}: repeat.over must be a bind that reads `layout_sections` (the per-person layout the host stores)")
    for path_, node_ in walk(doc):
        if isinstance(node_, dict) and isinstance(node_.get("bind"), str) and node_["bind"] in parent_binds:
            if not path_.endswith(".children"):
                errors.append(f"{path_}: the per-parent bind {node_['bind']!r} is consumed only through its declared edge (children or repeat.rows), not by an ordinary section, list, count or header")
    for bind_id in sorted(parent_binds - referenced):
        errors.append(f"$.binds {bind_id}: a bind with `source.parent: true` needs a declared edge (a RowTemplate.children or a Section.repeat that references it)")
    for path, node in walk(doc):
        if isinstance(node, dict) and node.get("context") == "placement" and set(node) == {"context"}:
            m = re.match(r"^\$\.binds\[(\d+)\]", path)
            owner = binds_list[int(m.group(1))].get("id") if m else None
            if owner not in repeat_rows: errors.append(f"{path}: {{context: placement}} is legal only inside the rows of a Section.repeat")
            elif owner in set(children_edges.values()): errors.append(f"{path}: this bind is both a section's rows and a child-rows bind: a placement operand would be usable from the child edge, where the row is no placement")
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
    is_reply = file.name.startswith(("reply-", "invalid-reply-"))
    if doc is not None:
        check({"$ref": "#/$defs/Event"} if is_event else {"$ref": "#/$defs/WindowReply"} if is_reply else schema, doc, "$", errors)
        if is_event and isinstance(doc, dict) and doc.get("type") == "window" and isinstance(doc.get("request"), dict) and CATALOGUE is not None:
            req = doc["request"]; limit = ((CATALOGUE.get("query") or {}).get("window") or {}).get("max_request_span")
            if isinstance(req.get("from"), int) and isinstance(req.get("to"), int):
                if not req["from"] < req["to"]: errors.append(f"$.request: from ({req['from']}) must be below to ({req['to']})")
                if limit is not None and req["to"] - req["from"] > limit: errors.append(f"$.request: the span {req['to'] - req['from']} exceeds the catalogue's max_request_span ({limit})")
        if not is_event and not is_reply:
            view_limits(doc, errors)
            semantic(doc, errors, CATALOGUE, manifest)
    expect_invalid = file.name.startswith("invalid-")
    ok = bool(errors) if expect_invalid else not errors
    print(("ok   " if ok else "FAIL ") + file.name + ("" if ok else "  " + "; ".join(errors[:4])))
    failed |= not ok
sys.exit(1 if failed else 0)
