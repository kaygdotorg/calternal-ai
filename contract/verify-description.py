#!/usr/bin/env python3
"""Verify pinned plugin v2 fixtures or emitted package JSON with the standard library.

Use --package DIR or --manifest FILE. Use --event DESCRIPTION FILE for events
and --reply FILE for replies. See README.md for usage and input bounds. Run
without python -I so Python can import the modules beside this script.

JSON decoding is bounded by 4 MiB per artifact and 32 nested containers.
The visit budget meters inspection of decoded object pairs and the decoded tree.

Fixture events in events/<description>/ resolve that app's Bind, and every
invalid fixture must match the intended reason in its expected-errors.json.
"""
import argparse
import hashlib
import json
import pathlib
import re
import sys

from description_confirmation import (check_confirmation_exchange, check_confirmation_material,
                                      check_confirmation_table, check_request_card_material)
from description_intent_trace import check_attempt_fence_trace, check_discovery_audience, check_intent_trace
from description_input import PackageInput, Validation, check_unique_ids, lexical_path, read_bytes, read_json, resolved_path

from description_events import COMPONENT_TYPES, EventBindings, check_composer_actions, check_composer_invoke, check_event_binding, check_invoke, check_route_capacity, route_operands, validate_component_request, walk
import description_verification as verification

from description_verification import (
    DuplicateKey,
    check,
    field_meta,
    field_names,
    is_reply_fixture,
    load_fixture_manifest,
    load_reply_origins,
    load_strict,
    parse_manifest,
    root,
    schema,
    validate_reply,
)


def view_limits(doc, errors):
    """Enforce the aggregate content node cap across all branches of the tree."""
    root_node = (doc.get("view") or {}).get("root") if isinstance(doc, dict) else None
    if isinstance(root_node, dict):
        pending = [root_node]
        count = 0
        while pending and not errors.full:
            errors.visit()
            node = pending.pop()
            count += 1
            if isinstance(node, dict):
                pending.extend(errors.each(node.get("children", [])))
        if count > 200:
            errors.append(f"$.root: {count} nodes (at most 200)")


# ---- semantic checks the schema cannot say ----------------------------------------------------------------------------------------------------------------
def semantic(doc, errors, catalogue, manifest, installed_manifests=()):
    """Unique action ids per document; bind, form, draft and row source scope; fullSwipe placement and catalogue permission; operations, kinds and fields resolved against
    the host catalogue and fixture manifests; only the requester supplies reads and operations; a header title bound to an item bind."""
    if not isinstance(doc, dict): return
    binds_list = doc.get("binds", [])
    binds = {b.get("id"): b for b in errors.each(binds_list)}
    forms, composers = {}, {}
    cat_ops = (catalogue or {}).get("operations", {})
    cat_kinds = (catalogue or {}).get("kinds", {})
    man_ops = (manifest or {}).get("operations", {})
    composer_sends = verification.composer_send_operations(manifest or {}, errors)
    man_kinds = dict(errors.each((manifest or {}).get("kinds", {}).items()))
    for installed in errors.each(installed_manifests):
        for kind_id, kind in errors.each(installed.get("kinds", {}).items()):
            if kind_id in man_kinds:
                errors.append(f"$.binds: multiple installed owners for kind {kind_id!r}")
            else:
                man_kinds[kind_id] = kind
    man_kinds = {name: verification.project_kind(kind, catalogue)
                 for name, kind in errors.each(man_kinds.items())}
    children_edges = {b_id: ((b.get("row") or {}).get("children") or {}).get("bind") for b_id, b in errors.each(binds.items()) if ((b.get("row") or {}).get("children") or {}).get("bind")}
    repeat_rows = {n_["repeat"]["rows"] for _, n_ in walk(doc, errors) if isinstance(n_, dict) and isinstance(n_.get("repeat"), dict) and "id" in n_ and "rows" in n_["repeat"]}
    for path, node in walk(doc, errors, skip_literals=True):
        if isinstance(node, dict) and node.get("type") == "form" and "id" in node: forms[node["id"]] = (path, {field.get("id") for field in errors.each(node.get("fields", [])) if isinstance(field, dict)})
        if isinstance(node, dict) and node.get("type") == "composer" and "id" in node: composers[node["id"]] = path
    for path, node in walk(doc, errors, skip_literals=True):
        if not isinstance(node, dict): continue
        if "operation" in node and "id" in node and isinstance(node.get("operation"), str):
            op = node["operation"]
            if op in composer_sends:
                errors.append(f"{path}: prepared Send operation is reserved for the host composer")
            entry = cat_ops.get(op) if catalogue is not None else None
            if catalogue is not None:
                row_input = re.match(r"^\$\.binds\[(\d+)\]\.row\.", path)
                if row_input:
                    row_kind = (binds_list[int(row_input.group(1))].get("source") or {}).get("kind")
                    row_fields = cat_kinds.get(row_kind) or man_kinds.get(row_kind) or {}
                    generic = catalogue["generic_item_fields"]
                    for name, operand in errors.each(node.get("inputs", {}).items()):
                        if isinstance(operand, dict) and operand.get("row") == "field":
                            if field_meta(row_fields, operand.get("field"), generic).get("type") == "RowImageRef":
                                errors.append(f"{path}: RowImageRef cannot supply operation input {name!r}")
            if node.get("fullSwipe"):
                if not re.search(r"(leadingActions|trailingActions)\[0\]\Z", path):
                    errors.append(f"{path}: fullSwipe is allowed only on the first action of a swipe set")
                if catalogue is not None and not (entry and entry.get("full_swipe", False)):
                    errors.append(f"{path}: fullSwipe is rejected on {op}: the catalogue does not allow it (plugin operations never may)")
            if catalogue is not None:
                host_like = verification.reserved_operation(op, catalogue) or op in cat_ops
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
                            for branch in errors.each(branches):
                                props = {**base_props, **(branch.get("properties") or {})}
                                req = base_required + list(branch.get("required", []))
                                if all(r in given for r in errors.each(req)) and all(k in props for k in errors.each(given)): fits.append((props, req))
                            wants_one = "oneOf" in input_schema
                            if (wants_one and len(fits) != 1) or (not wants_one and not fits):
                                errors.append(f"{path}: {op} inputs {sorted(errors.each(given))} fit {len(fits)} of the input schema's branches")
                            if fits: schema_in, required = fits[0]
                        for name in errors.each(given):
                            if schema_in and name not in schema_in: errors.append(f"{path}: {op} has no input {name!r}")
                        for name in errors.each(required):
                            if name not in given: errors.append(f"{path}: {op} needs input {name!r}")
                        owner_bind = None
                        m_owner = re.match(r"^\$\.binds\[(\d+)\]\.row\.", path)
                        if m_owner: owner_bind = binds_list[int(m_owner.group(1))]
                        owner_kind = ((owner_bind or {}).get("source") or {}).get("kind")
                        owner_in_repeat = (owner_bind or {}).get("id") in repeat_rows
                        for name, source in errors.each(given.items()):
                            declared_type = ((schema_in.get(name) or {}).get("$ref") or "").rsplit("/", 1)[-1]
                            if op == "host.item.create" and name == "kind":
                                kind_id = source.get("literal") if isinstance(source, dict) else None
                                kind = man_kinds.get(kind_id) if isinstance(kind_id, str) else None
                                if not kind or not isinstance(kind.get("host_create"), dict):
                                    errors.append(f"{path}: host.item.create needs a literal kind with a host_create declaration")
                            if op == "host.item.create" and name == "collection":
                                if isinstance(source, dict) and source.keys() == {"literal"}:
                                    check(schema_in[name], source["literal"], f"{path}.inputs.collection", errors, catalogue)
                                elif not isinstance(source, dict) or source.keys() != {"route"}:
                                    errors.append(f"{path}: host.item.create collection must come from a route or literal")
                                continue
                            ctx_kind = source.get("context") if isinstance(source, dict) and source.keys() == {"context"} else None
                            layout_selection = isinstance(source, dict) and "selection" in source and not source.get("set") and source["selection"] in repeat_rows
                            layout_bare = owner_in_repeat and isinstance(source, dict) and source == {"row": "id"}
                            if declared_type == "LayoutEntryRef" and ctx_kind != "placement" and not layout_selection and not layout_bare: errors.append(f"{path}: input {name!r} of {op} is a LayoutEntryRef: it takes {{context: placement}}")
                            conversation_input = (entry.get("kind_capability") == "conversation" and name == "item") or (op, name) == ("host.layout.move_to", "conversation")
                            if conversation_input and owner_in_repeat and ctx_kind != "item":
                                errors.append(f"{path}: in the rows of a Section.repeat the row id is a placement (LayoutEntryRef): the ItemRef input {name!r} of {op} must be {{context: item}}, not the bare row id")
                            if conversation_input and owner_kind and (ctx_kind == "item" or source == {"row": "id"}):
                                if not man_kinds.get(owner_kind, {}).get("conversation"):
                                    errors.append(f"{path}: {{context: item}} requires a conversation kind, not {owner_kind!r}")
                            if ctx_kind == "placement" and declared_type not in ("LayoutEntryRef", ""): errors.append(f"{path}: {{context: placement}} fills a LayoutEntryRef, not {declared_type}")
                            if ctx_kind == "item" and declared_type == "LayoutEntryRef": errors.append(f"{path}: {{context: item}} cannot fill the LayoutEntryRef input {name!r}")
                            REF_TYPES = {k.get("row_ref") for k in errors.each(cat_kinds.values()) if k.get("row_ref")} | {"ItemRef", "LayoutEntryRef"}   # every ref a row can be or give
                            move_to_destination = (op, name) == ("host.layout.move_to", "section")   # a destination choice, checked below
                            row_ref_of_owner = (cat_kinds.get(owner_kind) or {}).get("row_ref") if owner_kind in cat_kinds else ("ItemRef" if owner_kind else None)
                            if declared_type in REF_TYPES and ctx_kind == "item" and declared_type != row_ref_of_owner and not move_to_destination:
                                errors.append(f"{path}: {{context: item}} gives the row's own {row_ref_of_owner} and cannot fill the {declared_type} input {name!r} of {op}")
                            strict_conversation = (op, name) == ("host.layout.move_to", "conversation")
                            if strict_conversation and not (ctx_kind == "item" or (isinstance(source, dict) and source == {"row": "id"})):
                                errors.append(f"{path}: the `conversation` of host.layout.move_to takes only {{context: item}} or the bare id of a conversation row, not {json.dumps(source, sort_keys=True)}")
                            if declared_type in REF_TYPES and ctx_kind is None and not move_to_destination:
                                bare_id = isinstance(source, dict) and source == {"row": "id"}
                                from_route = isinstance(source, dict) and source.keys() == {"route"}
                                row_ref = (cat_kinds.get(owner_kind) or {}).get("row_ref") if owner_kind in cat_kinds else ("ItemRef" if owner_kind else None)
                                prompt = source.get("prompt") if isinstance(source, dict) else None
                                choice_ok = False
                                if isinstance(prompt, dict) and prompt.get("kind") == "choice":
                                    options = prompt.get("options")
                                    option_bind = binds.get(options.get("bind")) if isinstance(options, dict) else None
                                    choice_ok = (cat_kinds.get(((option_bind or {}).get("source") or {}).get("kind")) or {}).get("row_ref") == declared_type
                                selection_ok = False
                                if isinstance(source, dict) and "selection" in source and not source.get("set"):
                                    sel_bind = binds.get(source["selection"])
                                    sel_kind = ((sel_bind or {}).get("source") or {}).get("kind")
                                    sel_ref = (cat_kinds.get(sel_kind) or {}).get("row_ref") if sel_kind in cat_kinds else ("ItemRef" if sel_kind else None)
                                    if source["selection"] in repeat_rows: sel_ref = "LayoutEntryRef"   # a Section.repeat.rows bind presents the placement as its row id
                                    selection_ok = sel_ref == declared_type
                                if strict_conversation: from_route = choice_ok = selection_ok = False
                                bare_ok = bare_id and ((owner_in_repeat and declared_type == "LayoutEntryRef") or (not owner_in_repeat and row_ref == declared_type))
                                if not (from_route or choice_ok or selection_ok or bare_ok):
                                    errors.append(f"{path}: the {declared_type} input {name!r} of {op} takes only a source of that ref type ({{context}}, a bare row id of a row of that type, or a route), not {json.dumps(source, sort_keys=True)}")
                            if declared_type in ("ChoiceRef", "LayoutSectionRef") and (declared_type == "ChoiceRef" or name == "section"):
                                prompt = source.get("prompt") if isinstance(source, dict) else None
                                if not (isinstance(prompt, dict) and prompt.get("kind") == "choice"):
                                    errors.append(f"{path}: input {name!r} of {op} is a destination choice: it must come from a `choice` prompt over the authorized options, not a literal or a row value")
                                else:
                                    options = prompt.get("options")
                                    option_bind = binds.get(options.get("bind")) if isinstance(options, dict) else None
                                    option_kind = cat_kinds.get(((option_bind or {}).get("source") or {}).get("kind"))
                                    want = "LayoutSectionRef" if (declared_type == "LayoutSectionRef" or name == "section") else None
                                    if want and (option_kind is None or option_kind.get("row_ref") != want):
                                        errors.append(f"{path}: the choice for {name!r} of {op} must range over a bind whose kind has row_ref {want} (layout_sections), not {((option_bind or {}).get('source') or {}).get('kind')!r}")
                        for name in errors.each(entry.get("target_inputs", [])):
                            if isinstance(given.get(name), dict) and "literal" in given[name]: errors.append(f"{path}: target input {name!r} of {op} cannot be a literal")
                elif manifest is not None and op not in man_ops:
                    errors.append(f"{path}: operation {op!r} is in neither the host catalogue nor an installed fixture manifest")
                elif manifest is not None:
                    declared = man_ops[op].get("input_schema") if isinstance(man_ops[op], dict) else None
                    if isinstance(declared, dict) and isinstance(declared.get("properties"), dict):
                        given = node.get("inputs", {})
                        for name in errors.each(given):
                            if name not in declared["properties"]: errors.append(f"{path}: {op} (manifest) has no input {name!r}")
                        for name in errors.each(declared.get("required", [])):
                            if name not in given: errors.append(f"{path}: {op} (manifest) needs input {name!r}")
        if "bind" in node and isinstance(node["bind"], str) and node["bind"] not in binds:
            errors.append(f"{path}: bind {node['bind']!r} is not declared in `binds`")
        if "bind" in node and "field" in node and node.keys() == {"bind", "field"} and ".config" not in path and ".header.title" not in path and ".row" not in path:
            errors.append(f"{path}: a {{bind, field}} source is legal only in a host component's config (and a header title)")
        row_owner = re.match(r"^\$\.binds\[(\d+)\]\.row\.", path)
        if "row" in node and node.keys() <= {"row", "field"} and not row_owner:
            errors.append(f"{path}: a {{row}} source is only valid inside a bind's row template")
        if node.keys() == {"context"}:
            if node["context"] == "placement":
                if not row_owner or binds_list[int(row_owner.group(1))]["id"] not in repeat_rows:
                    errors.append(f"{path}: {{context: placement}} is legal only inside the rows of a Section.repeat")
            elif node["context"] == "item" and not row_owner:
                errors.append(f"{path}: {{context: item}} is legal only inside a bind's row template")
        if "form" in node and node.keys() == {"form", "field"}:
            owner = forms.get(node["form"])
            if owner is None: errors.append(f"{path}: form {node['form']!r} does not exist")
            elif not path.startswith(owner[0] + ".submit"): errors.append(f"{path}: a {{form}} source is only valid inside that form's own submit")
            elif node["field"] not in owner[1]: errors.append(f"{path}: unknown form field {node['field']!r}")
        if "draft" in node and node.keys() == {"draft"} and isinstance(node["draft"], str):
            owner = composers.get(node["draft"])
            if owner is None: errors.append(f"{path}: composer {node['draft']!r} does not exist")
            elif not path.startswith(owner + ".config"): errors.append(f"{path}: a {{draft}} source is only valid inside that composer's own config")
    if manifest is not None:
        reads = set(errors.each(manifest.get("reads", [])))
        for index, bind in enumerate(errors.each(binds_list)):
            kind_id = (bind.get("source") or {}).get("kind")
            if isinstance(kind_id, str) and kind_id not in reads:
                errors.append(f"$.binds[{index}].source: kind {kind_id!r} is not in the manifest's `reads` (every bind needs it, including a package's own kinds)")
    for index, b in enumerate(errors.each(binds_list)):
        if not b.get("persist"): continue
        src = b.get("source") or {}
        for key in errors.each(("ref", "text", "parent")):
            if key in src: errors.append(f"$.binds[{index}]: persist needs INVARIANT query inputs: the source has `{key}`")
        if "collection" in src and not (isinstance(src["collection"], dict) and src["collection"].keys() == {"literal"}):
            errors.append(f"$.binds[{index}]: persist allows only a LITERAL collection restriction, not {json.dumps(src['collection'], sort_keys=True)}")
        if src.get("type") != "list": errors.append(f"$.binds[{index}]: persist needs a `list` source")
        for g in errors.each(("all", "any")):
            for pr in errors.each((b.get("filter") or {}).get(g, [])):
                value = pr.get("value")
                if value is not None and not (isinstance(value, dict) and (value.keys() == {"literal"} or value == {"viewer": "person"})):
                    errors.append(f"$.binds[{index}]: persist needs INVARIANT query inputs: the filter operand {json.dumps(value, sort_keys=True)} on {pr.get('field')!r} is not a literal or the viewer")
    parent_binds = {i for i, b in errors.each(binds.items()) if (b.get("source") or {}).get("parent")}
    children_edges = {}
    for bind_id, b in errors.each(binds.items()):
        child = ((b.get("row") or {}).get("children") or {}).get("bind")
        if child is not None:
            if child not in parent_binds: errors.append(f"$.binds {bind_id}: children bind {child!r} must declare `source.parent: true`")
            children_edges[bind_id] = child
    for sec_path, sec in errors.each([(p_, n_) for p_, n_ in walk(doc, errors) if isinstance(n_, dict) and "repeat" in n_ and "id" in n_]):
        rep = sec["repeat"]
        for key in errors.each(("over", "rows")):
            if rep.get(key) not in binds: errors.append(f"{sec_path}: repeat.{key} {rep.get(key)!r} is not a declared bind")
        if rep.get("rows") in binds and rep["rows"] not in parent_binds: errors.append(f"{sec_path}: repeat.rows must be a bind with `source.parent: true`")
        if rep.get("over") in parent_binds: errors.append(f"{sec_path}: repeat.over must be a top-level bind, not a per-parent one")
        if "bind" in sec or sec.get("rows"): errors.append(f"{sec_path}: a repeated section has no `bind` or literal `rows` of its own")
    referenced = set(children_edges.values())
    for sec_path, sec in errors.each([(p_, n_) for p_, n_ in walk(doc, errors) if isinstance(n_, dict) and "repeat" in n_ and "id" in n_]):
        rep = sec["repeat"]
        if rep.get("rows") in binds: referenced.add(rep["rows"])
        over = binds.get(rep.get("over"))
        if over is not None and (over.get("source") or {}).get("kind") != "layout_sections":
            errors.append(f"{sec_path}: repeat.over must be a bind that reads `layout_sections` (the per-person layout the host stores)")
    for path_, node_ in walk(doc, errors):
        if isinstance(node_, dict) and isinstance(node_.get("bind"), str) and node_["bind"] in parent_binds:
            if not path_.endswith(".children"):
                errors.append(f"{path_}: the per-parent bind {node_['bind']!r} is consumed only through its declared edge (children or repeat.rows), not by an ordinary section, list, count or header")
    for bind_id in errors.each(sorted(parent_binds - referenced)):
        errors.append(f"$.binds {bind_id}: a bind with `source.parent: true` needs a declared edge (a RowTemplate.children or a Section.repeat that references it)")
    for bind_id in errors.each(sorted(repeat_rows & set(children_edges.values()))):
        errors.append(f"$.binds {bind_id}: a bind is a section's rows OR a children target, never both (its row ref would be a placement in one and the kind's own ref in the other)")
    def depth(bind_id, seen=()):
        if errors.full:
            return 0
        errors.visit()
        if bind_id in seen: errors.append(f"$.binds: children cycle through {bind_id!r}"); return 0
        nxt = children_edges.get(bind_id)
        return 1 + depth(nxt, seen + (bind_id,)) if nxt else 1
    for bind_id in errors.each(binds):
        if depth(bind_id) > 3: errors.append(f"$.binds {bind_id}: nested rows deeper than 3 levels")
    for index, b in enumerate(errors.each(binds_list)):
        uses_parent = any(isinstance(pr.get("value"), dict) and "parent" in pr["value"] for g in errors.each(("all", "any")) for pr in errors.each((b.get("filter") or {}).get(g, [])))
        if uses_parent and not (b.get("source") or {}).get("parent"): errors.append(f"$.binds[{index}]: {{parent}} is legal only in a bind whose source has `parent: true`")
    header_title = (doc.get("chrome") or {}).get("header", {}).get("title")
    if isinstance(header_title, dict) and header_title.get("bind") in binds and binds[header_title["bind"]].get("source", {}).get("type") != "item":
        errors.append("$.chrome.header.title: a header title takes its text from an `item` bind (the open item), not from a list's first row")
    for index, bind in enumerate(errors.each(binds_list)):
        if catalogue is None: break
        source = bind.get("source", {})
        kind_id = source.get("kind")
        path = f"$.binds[{index}].source"
        if not isinstance(kind_id, str): continue
        kind = cat_kinds.get(kind_id)
        plugin = man_kinds.get(kind_id)
        if source.get("type") == "search":
            conversation_search = plugin and plugin.get("conversation") is True and plugin.get("searchable") is True
            claimed_text_search = kind is None and plugin and plugin.get("claimed_text") is True and plugin.get("searchable") is True
            if not (conversation_search or claimed_text_search):
                errors.append(f"{path}: {kind_id!r} needs claimed_text and searchable in its kind manifest to support search")
            elif "field_filter" in source:
                search = catalogue["query"]["search"]
                profile = search["conversation" if conversation_search else "claimed_text"]
                check(profile["field_filter_schema"], source["field_filter"], f"{path}.field_filter", errors, catalogue)
        if catalogue is not None and kind is None and plugin is None:
            if manifest is not None or kind_id in ("notes", "threads", "collections", "requests"):
                errors.append(f"{path}: kind {kind_id!r} is in neither the host catalogue nor an installed fixture manifest")
            continue
        entry = kind or plugin
        if entry is None: continue
        generic = catalogue["generic_item_fields"] if entry.get("includes_generic_item_fields", kind is None) else {}
        known = field_names(entry, errors, generic) | {"id"}
        predicates = [pred for group in errors.each(("all", "any")) for pred in errors.each(bind.get("filter", {}).get(group, []))]
        sorts = bind.get("sort", [])
        used = list(errors.each(bind.get("fields", []))) + [part["field"] for part in errors.each(predicates + sorts)]
        row = bind.get("row") or {}
        for part in errors.each(row.get("secondary", [])):
            field = part["field"]
            expected_type = "Text" if part["format"] == "text" else "Timestamp"
            if field not in bind.get("fields", []) or field_meta(entry, field, generic).get("type") != expected_type:
                errors.append(f"$.binds[{index}].row.secondary: {field!r} must project {expected_type} for {part['format']}")
        row_icon = row.get("icon")
        image = row_icon.get("image") if isinstance(row_icon, dict) else None
        if isinstance(image, dict):
            field = image.get("field")
            if isinstance(field, str):
                if field not in bind.get("fields", []) or field_meta(entry, field).get("type") != "RowImageRef":
                    errors.append(f"$.binds[{index}].row.icon: {field!r} must project a RowImageRef")
            fallback = image.get("fallback")
            if isinstance(fallback, str):
                meta = field_meta(entry, fallback, generic)
                if fallback not in bind.get("fields", []) or meta.get("type") != "Text" or meta.get("source") != "body_excerpt":
                    errors.append(f"$.binds[{index}].row.icon: fallback {fallback!r} must project a body_excerpt Text field")
        for f in errors.each(used):
            if f not in known: errors.append(f"{path}: field {f!r} is not a field of kind {kind_id!r}")
        for sort in errors.each(sorts):
            field = sort["field"]
            meta = field_meta(entry, field, generic)
            field_type = meta.get("type")
            if field_type in ("RowImageRef", "TextSet"):
                errors.append(f"{path}: {field_type} cannot be a sort key")
            elif field in known and (field_type is None or ("index_owner" in meta and meta["index_owner"] is None)):
                errors.append(f"{path}: sort field {field!r} requires a declared indexed field type")
        for pred in errors.each(predicates):
            field_type = field_meta(entry, pred["field"]).get("type")
            operation, operand = pred["op"], pred.get("value", {})
            if field_type == "RowImageRef":
                errors.append(f"$.binds[{index}].filter: RowImageRef cannot be a predicate operand")
            if operation == "contains":
                # Route parameters resolve to the consuming operand's type; an
                # account or parent source retains its reference type.
                literal = operand.get("literal")
                member = (catalogue or {}).get("$defs", {}).get("TextSet", {}).get("items")
                if field_type != "TextSet" or ("route" not in operand and not isinstance(literal, str)):
                    errors.append(f"$.binds[{index}].filter: contains requires a TextSet and one Text operand")
                elif "literal" in operand:
                    check(member, literal, f"$.binds[{index}].filter.value.literal", errors, catalogue)
            if field_type == "TextSet" and operation not in ("contains", "exists"):
                errors.append(f"$.binds[{index}].filter: TextSet supports contains and exists")
        for relative_path, desc in walk(bind, errors, skip_literals=True):
            if not isinstance(desc, dict):
                continue
            icon = desc.get("icon")
            if isinstance(icon, dict) and "field" in icon:
                field = icon["field"]
                if verification.body_derived_field(entry, field, field_meta(entry, field, generic)):
                    errors.append(f"$.binds[{index}]{relative_path[1:]}.icon: a symbol cannot use a body-derived field")
            if desc.get("kind") == "icon" and "field" in desc:
                field = desc["field"]
                if verification.body_derived_field(entry, field, field_meta(entry, field, generic)):
                    errors.append(f"$.binds[{index}]{relative_path[1:]}.field: a symbol cannot use a body-derived field")
            for key in errors.each(("when", "disabled")):
                operand = desc.get(key)
                if isinstance(operand, dict) and field_meta(entry, operand.get("field")).get("type") == "RowImageRef":
                    errors.append(f"$.binds[{index}]{relative_path[1:]}.{key}: RowImageRef cannot be a predicate operand")
        for pred in errors.each(predicates):
            meta = field_meta(entry, pred["field"])
            if meta.get("audience") == "viewer_self" and not (pred.get("op") == "eq" and pred.get("value") == {"viewer": "person"}):
                errors.append(f"{path}: {pred['field']!r} is viewer-only: it accepts only `eq` with {{viewer: person}}")


def provider_mark_fixture_symbol(case, icon):
    """Paper-check the row fallback using publisher and ownership facts from install."""
    mark = case["claimed_mark"]
    publisher = case["verified_publisher"]
    if publisher is not None and case["trusted_mark_owners"].get(mark) == publisher:
        return mark
    return icon["name"]


def expected_errors(fixtures):
    """Read each fixture directory's expected errors and require exact coverage."""
    expected = {}
    for directory in sorted({file.parent for file in fixtures if file.name.startswith("invalid-")}):
        for name, fragment in read_json(directory / "expected-errors.json").items():
            if pathlib.Path(name).name != name or not isinstance(fragment, str) or not fragment.strip():
                raise ValueError(f"{directory}: invalid expected error for {name!r}")
            expected[directory / name] = fragment
    negatives = {file for file in fixtures if file.name.startswith("invalid-")}
    if missing := negatives - expected.keys():
        raise ValueError("Missing expected errors: " + ", ".join(str(file) for file in sorted(missing)))
    if stale := expected.keys() - negatives:
        raise ValueError("Expected errors without negative fixtures: " + ", ".join(str(file) for file in sorted(stale)))
    return expected


def report_fixture(file, errors, expected):
    """A negative fixture must fail a named rule, even if other rules also reject it."""
    fragment = expected.get(file)
    ok = any(fragment in error for error in errors) if fragment is not None else not errors
    if ok:
        print(f"ok   {file.name}")
    else:
        wanted = f"expected error {fragment!r}. " if fragment is not None else ""
        print(f"FAIL {file.name}  {wanted}{'; '.join(errors) or 'no errors'}")
    return ok


def check_description_ids(doc, errors):
    """Reject list collisions and repeated Invoke or shell-action IDs before binding."""
    ids = {"action": {}, "form": {}, "component": {}}
    for path, node in walk(doc, errors):
        if not isinstance(node, dict) or not isinstance(node.get("id"), str):
            continue
        kind = node.get("type")
        if isinstance(node.get("operation"), str) or isinstance(node.get("shell"), dict):
            kind = "action"
        if isinstance(kind, str) and kind in COMPONENT_TYPES:
            kind = "component"
        if not isinstance(kind, str) or kind not in ids:
            continue
        ident = node["id"]
        if ident in ids[kind]:
            errors.append(f"{path}: duplicate {kind} id {ident!r} (first at {ids[kind][ident]})")
        else:
            ids[kind][ident] = path
    check_unique_ids(doc, errors, skip={"literal"})


def validate_description(doc, manifest, installed, errors):
    """Check the entire template structure before indexing IDs or checking relations."""
    check(schema, doc, "$", errors)
    if errors:
        return
    check_description_ids(doc, errors)
    if errors:
        return
    view_limits(doc, errors)
    if verification.CATALOGUE is not None:
        limit = verification.CATALOGUE["$defs"]["RouteOperands"]["maxProperties"]
        check_route_capacity(doc, limit, errors)
    for index, bind in enumerate(errors.each(doc.get("binds", []))):
        for state, declaration in errors.each(bind.get("states", {}).items()):
            if route_operands(declaration.get("action", {}), errors):
                errors.append(f"$.binds[{index}].states.{state}.action: Bind-state actions cannot consume route operands")
    semantic(doc, errors, verification.CATALOGUE, manifest, installed)


def validate_activation_capture(capture, bindings, errors):
    """Check a bounded chain of accepted replies before resolving an invoke."""
    base = {"view_open", "event"}
    if (not base <= capture.keys() <= base | {"renewals", "windows", "resolution", "confirmation", "checked_facts", "checked_inputs"}
            or not bindings or verification.CATALOGUE is None):
        errors.append("$: composer capture requires an originating description, view_open and event")
        return
    if "confirmation" in capture and "resolution" not in capture:
        errors.append("$.confirmation: requires the originating resolution")
        return
    catalogue = verification.CATALOGUE
    check({"type": "array", "maxItems": 16, "uniqueItems": True, "items": {"type": "string"}},
          capture.get("checked_facts", []), "$.checked_facts", errors, catalogue)
    capture_fields = {"view_open": "ViewOpenResult", "event": "ResolveActivation"}
    for name, definition in errors.each(capture_fields.items()):
        check({"$ref": f"#/$defs/{definition}"}, capture[name], f"$.{name}", errors, catalogue)
    pair = {
        "type": "object", "additionalProperties": False, "required": ["request", "result"],
        "properties": {"request": {"$ref": "#/$defs/LeaseRenewRequest"},
                       "result": {"$ref": "#/$defs/LeaseRenewResult"}},
    }
    renewals = capture.get("renewals", [])
    check({"type": "array", "maxItems": 16, "items": pair}, renewals, "$.renewals", errors, catalogue)
    window_pair = {
        "type": "object", "additionalProperties": False, "required": ["lease", "request", "result"],
        "properties": {"lease": {"$ref": "#/$defs/SubscriptionLease"},
                       "request": {"$ref": "#/$defs/WindowRequest"},
                       "result": {"$ref": "#/$defs/WindowResult"}},
    }
    windows = capture.get("windows", [])
    check({"type": "array", "maxItems": 16, "items": window_pair}, windows, "$.windows", errors, catalogue)
    if errors:
        return
    origin = bindings()
    if errors:
        return
    policy = catalogue["activation_capture"]
    contexts = {}
    audience = capture["view_open"]["audience"]

    def issued_actions(reply, path):
        issued = check_composer_actions(origin, reply, audience, errors)
        if errors:
            return issued
        lease = reply["lease"]
        authority = tuple(lease[field] for field in policy["context_authority_fields"])
        def remember(action, context, purpose, scope):
            binding = (authority, purpose, action, scope)
            previous = contexts.setdefault(context, binding)
            if previous == binding or (purpose is None and previous[1] is None):
                return
            if purpose is None or previous[1] is None:
                errors.append(f"{path}.activation_contexts: composer context aliases a non-composer action")
            elif previous[0] != authority:
                errors.append(f"{path}.activation_contexts: reused composer context from a different lease or grant")
            else:
                errors.append(f"{path}.activation_contexts: composer context was rebound to a different component, purpose or action ID")

        for action, context in errors.each(reply["activation_contexts"].items()):
            remember(action, context, issued.get(action), "view")
        for window in errors.each(reply.get("windows", [])):
            for row in errors.each(window.get("activation_contexts", [])):
                for action, context in errors.each(row["contexts"].items()):
                    remember(action, context, None, "row")
        return issued

    reply = capture["view_open"]
    issued = issued_actions(reply, "$.view_open")
    seen = set()
    for index, renewal in enumerate(errors.each(renewals)):
        request, result = renewal["request"], renewal["result"]
        lease = reply["lease"]
        path = f"$.renewals[{index}]"
        if request["renewal_id"] in seen:
            errors.append(f"{path}.request: renewal ID was already used")
            return
        seen.add(request["renewal_id"])
        sources = {"request": request, "current": lease, "result": result, "next": result["lease"]}
        if any(len({sources[source][field] for source, field in
                    (entry.split(".", 1) for entry in fields)}) != 1
               for fields in policy["renewal_matches"]):
            errors.append(f"{path}.result: must match the originating lease, partition and outstanding renewal from the last accepted reply")
            return
        for retained in errors.each(result["retained_results"]):
            if (lease["grant_revision"] != result["lease"]["grant_revision"]
                    or request["result_versions"].get(retained["window_ref"]) != retained["result_version"]):
                errors.append(f"{path}.result.retained_results: must name a requested exact result under the unchanged grant")
        reply = result
        issued = issued_actions(reply, f"{path}.result")
    if errors:
        return
    accepted_windows = {window["window_ref"]: window for window in errors.each(reply.get("windows", []))}
    captured_refs = set()
    for index, window in enumerate(errors.each(windows)):
        path = f"$.windows[{index}]"
        request, result = window["request"], window["result"]
        if window["lease"] != reply["lease"] or result["audience"] != audience:
            errors.append(f"{path}: must match the last accepted view lease, grant, partition and audience")
            continue
        validate_event(request, lambda: origin, errors)
        payload = request["request"]
        if (any(payload.get(field) != result.get(field)
                for field in policy["window_scope_fields"])
                or any(payload.get(field, default) != result.get(field, default)
                       for field, default in policy["window_scope_defaults"].items())):
            errors.append(f"{path}: result must match the originating window Bind and scope")
        if result["state"] != "ready":
            errors.append(f"{path}: activation contexts require a ready window")
        ref = result["window_ref"]
        if ref in captured_refs:
            errors.append(f"{path}: capture must supply one current result per window ref")
        captured_refs.add(ref)
        accepted_windows[ref] = result
    if errors:
        return
    # Ordinary windows and renewal windows share the same context alias checks.
    issued = issued_actions({**reply, "windows": list(accepted_windows.values())}, "$.windows")
    if errors:
        return
    event = capture["event"]
    action_id = event["action_id"]
    if action_id in issued or action_id not in origin.actions:
        check_composer_invoke(event, reply, issued, errors)
        if errors:
            return
        purpose = issued[action_id][1]
        kind = policy["composer_operations"][purpose]
        facts = [f"has_{name}" for name, value in event.get("interaction", {}).get("draft", {}).items()
                 if name in ("attachments", "mentions", "quote") and value]
        if capture.get("checked_facts"):
            errors.append("$.checked_facts: composer material comes from the originating invoke")
    else:
        check_invoke(event, origin, errors)
        token = event["activation_context"]
        row_bind = origin.row_action_binds.get(action_id)
        if row_bind is None:
            matches = reply["activation_contexts"].get(action_id) == token
        else:
            issued_rows = set()
            for window in errors.each(accepted_windows.values()):
                if window["audience"] != audience:
                    continue
                row_ids = {value["id"] for value in errors.each(window["rows"])}
                for row in errors.each(window.get("activation_contexts", [])):
                    if row["row"] in row_ids and row["contexts"].get(action_id) == token:
                        issued_rows.add((window["bind"], row["row"]))
            matches = len(issued_rows) == 1 and next(iter(issued_rows))[0] == row_bind
        if not matches:
            errors.append("$.activation_context: must match the issued host action context")
        kind = origin.actions[action_id]["operation"]
        facts = capture.get("checked_facts", ())
    if not errors and "resolution" in capture:
        material = {}
        if kind not in catalogue["operations"] and kind != "composer.send":
            operation = origin.operations.get(kind)
            if operation is None:
                errors.append("$.event.action_id: operation is not declared by the originating manifest")
                return
            if "checked_inputs" not in capture:
                errors.append("$.checked_inputs: authored confirmation requires checked operation input")
                return
            values = capture["checked_inputs"]
            check(operation["input_schema"], values, "$.checked_inputs", errors, operation["input_schema"])
            if errors:
                return
            material = {key: (label, values[key]) for key, label in operation.get("material_inputs", {}).items()
                        if isinstance(values, dict) and key in values}
            kind = "plugin_operation"
        elif "checked_inputs" in capture:
            errors.append("$.checked_inputs: only authored confirmation uses manifest inputs")
            return
        check_confirmation_exchange(capture, kind, facts, errors, catalogue, material=material)


def validate_event(doc, bindings, errors):
    """Check event shape and contextual Bind and window range requirements."""
    if isinstance(doc, dict) and "view_open" in doc:
        validate_activation_capture(doc, bindings, errors)
        return
    check({"$ref": "#/$defs/Event"}, doc, "$", errors)
    if errors:
        return
    if doc["type"] == "component":
        validate_component_request(doc, errors, verification.CATALOGUE)
    if verification.CATALOGUE is not None and doc["type"] == "window":
        request = doc["request"]
        start, end = request["from"], request["to"]
        limit = verification.CATALOGUE["query"]["window"]["max_request_span"]
        if start >= end:
            errors.append(f"$.request: from ({start}) must be below to ({end})")
        if end - start > limit:
            errors.append(f"$.request: the span {end - start} exceeds the catalogue's max_request_span ({limit})")
    if not errors:
        checked_origin = bindings() if bindings else None
        if not errors:
            check_event_binding(doc, checked_origin, errors, verification.CATALOGUE)


def fixture_scope(file, manifests_dir, engine_owner, errors):
    """Supply the installed conversation owner for local conformance fixtures."""
    manifest, installed = load_fixture_manifest(file, manifests_dir, errors)
    if engine_owner and "threads" not in (manifest or {}).get("kinds", {}) and not any("threads" in owner.get("kinds", {}) for owner in installed):
        installed += (engine_owner,)
    return manifest, installed


def fixture_origins(fixtures, fixtures_dir, manifests_dir, engine_owner=None):
    reply_files = {file.name for file in fixtures if is_reply_fixture(file)}
    if verification.CATALOGUE is None or not reply_files:
        return {}
    errors = Validation()
    origins = read_json(fixtures_dir / "replies" / "origins.json", errors)
    verification.check_reply_origins_shape(origins, reply_files, errors)
    sources = {}
    for file in errors.each(fixtures):
        if file.parent == fixtures_dir and file.name in origins:
            app = read_json(file, errors)
            check(schema, app, "$", errors)
            if errors:
                raise ValueError(f"{file}: {'; '.join(errors)}")
            sources[file.resolve()] = app, *fixture_scope(file, manifests_dir, engine_owner, errors)
    return load_reply_origins(origins, fixtures_dir, sources, verification.CATALOGUE, reply_files, errors)


def fixture_event_origin(file, manifests_dir, errors):
    """Index a fixture origin only after its entire template passes structural checks."""
    app = read_json(file, errors)
    check(schema, app, "$", errors)
    manifest, _ = load_fixture_manifest(file, manifests_dir, errors)
    return EventBindings(app, errors, manifest) if not errors else None


def run_fixtures():
    """Run the checked-in positive, negative and paper verification cases."""
    fixtures_dir = root / "fixtures" / "description-v2"
    manifests_dir = fixtures_dir / "manifests"
    fixtures = sorted(file for file in [*fixtures_dir.glob("*.json"), *fixtures_dir.glob("events/*/*.json")]
                      if file.name != "expected-errors.json")
    install_fixtures = sorted(file for file in (manifests_dir / "install").glob("*.json")
                              if file.name != "expected-errors.json")
    expected = expected_errors(fixtures + install_fixtures)
    print("host catalogue: " + ("loaded" if verification.CATALOGUE is not None else "absent (catalogue cases are skipped)"))
    engine_owner = verification.load_checked_manifest(manifests_dir / "rev14-threads-owner.json", Validation()) if verification.CATALOGUE is not None else None
    origins = fixture_origins(fixtures, fixtures_dir, manifests_dir, engine_owner)
    failed = False
    for file in fixtures:
        manifest_path = manifests_dir / file.name.removeprefix("invalid-")
        if verification.CATALOGUE is None and ("-catalogue-" in file.name or manifest_path.exists()):
            print(f"skip {file.name}  (needs the host catalogue)")
            continue
        errors = Validation()
        try:
            doc = read_json(file, errors)
            if file.name.startswith(("event-", "invalid-event-")):
                app = None
                if file.parent.parent == fixtures_dir / "events":
                    app = lambda: fixture_event_origin(fixtures_dir / f"{file.parent.name}.json", manifests_dir, errors)
                validate_event(doc, app, errors)
            elif is_reply_fixture(file):
                validate_reply(doc, origins.get(file.name), errors)
            else:
                if verification.CATALOGUE is None and any(
                    path.endswith(".source") and isinstance(source, dict) and source.get("type") == "search"
                    for path, source in walk(doc, errors)
                ):
                    print(f"skip {file.name}  (search qualification needs the host catalogue)")
                    continue
                manifest, installed = fixture_scope(file, manifests_dir, engine_owner, errors)
                validate_description(doc, manifest, installed, errors)
        except (DuplicateKey, ValueError) as error:
            errors.append(str(error))
        failed |= not report_fixture(file, errors, expected)
    for file in install_fixtures:
        if verification.CATALOGUE is None:
            print(f"skip {file.name}  (install checks need the host catalogue)")
            continue
        errors = Validation()
        try:
            parse_manifest(read_bytes(file).decode("utf-8"), errors)
        except (DuplicateKey, ValueError) as error:
            errors.append(str(error))
        failed |= not report_fixture(file, errors, expected)
    if verification.CATALOGUE is not None:
        errors = Validation()
        check_confirmation_table(errors, verification.CATALOGUE)
        if errors:
            print(f"FAIL confirmation material table  {list(errors)!r}")
            failed = True
    for file in sorted((fixtures_dir / "engines").glob("*.json")):
        if verification.CATALOGUE is None:
            continue
        errors = Validation()
        case = read_json(file, errors)
        if not errors:
            check({"$ref": f"#/$defs/{case['definition']}"}, case["value"], "$", errors, verification.CATALOGUE)
        if not errors and case["definition"] in ("ActivationResolution", "ResumeResolution"):
            kind = "host.execution.resume" if case["definition"] == "ResumeResolution" else case.get("challenge_kind")
            if case["value"].get("confirm") == "challenge" or kind is not None:
                check_confirmation_material(case["value"], kind, case.get("checked_facts", ()),
                                            errors, verification.CATALOGUE, inputs=case.get("material_inputs", ()))
        if not errors and case["definition"] == "RequestCardRecord":
            check_request_card_material(case["value"], errors, verification.CATALOGUE,
                                        checked=case.get("checked_effect"))
        if not errors and case["definition"] in ("TranscriptPage", "TranscriptUpdate"):
            records_key = "records" if case["definition"] == "TranscriptPage" else "upserts"
            for record in errors.each(case["value"][records_key]):
                refs = [attachment["ref"] for attachment in errors.each(record.get("attachments", []))]
                if len(set(refs)) != len(refs):
                    errors.append(f"$.{records_key}: attachment refs must be distinct within the record")
                if "attachment_projection" in case and refs != case["attachment_projection"].get(record["record_id"], []):
                    errors.append(f"$.{records_key}: attachment projection differs from checked record authority")
        if not errors and case["definition"] == "HostWorkerInvokeFrame":
            manifest = verification.load_checked_manifest(manifests_dir / case["manifest"], errors)
            verification.check_worker_invocation(case["value"], manifest, case.get("job"), errors)
        want = case.get("error") if case else None
        passed = any(want in error for error in errors) if want else not errors
        print(("ok   " if passed else "FAIL ") + file.name + ("" if passed else f"  {list(errors)!r}"))
        failed |= not passed
    for file in sorted((fixtures_dir / "intent-races").glob("*.json")):
        if verification.CATALOGUE is None:
            continue
        errors = Validation()
        case = read_json(file, errors)
        if not errors:
            check_intent_trace(case, errors, verification.CATALOGUE)
        want = case.get("error") if case else None
        passed = any(want in error for error in errors) if want else not errors
        print(("ok   " if passed else "FAIL ") + file.name + ("" if passed else f"  {list(errors)!r}"))
        failed |= not passed
    for file in sorted((fixtures_dir / "attempt-discovery").glob("*.json")):
        if verification.CATALOGUE is None:
            continue
        errors = Validation()
        case = read_json(file, errors)
        if not errors:
            check_discovery_audience(case, errors, verification.CATALOGUE)
        want = case.get("error") if case else None
        passed = any(want in error for error in errors) if want else not errors
        print(("ok   " if passed else "FAIL ") + file.name + ("" if passed else f"  {list(errors)!r}"))
        failed |= not passed
    if verification.CATALOGUE is not None:
        covered = set()
        for file in sorted((fixtures_dir / "attempt-fences").glob("*.json")):
            errors = Validation()
            case = read_json(file, errors)
            if not errors:
                check_attempt_fence_trace(case, errors, verification.CATALOGUE)
            want = case.get("error") if case else None
            passed = any(want in error for error in errors) if want else not errors
            if passed and want is None:
                covered.add(case["action"])
            print(("ok   " if passed else "FAIL ") + file.name + ("" if passed else f"  {list(errors)!r}"))
            failed |= not passed
        policy = verification.CATALOGUE["attempt_authority_end"]
        required = set(policy["actions"])
        listed = {action for group in policy["proposal_inventory"].values() for action in group}
        if listed != required:
            print("FAIL attempt fence inventory does not match the catalogue actions")
            failed = True
        if required != covered:
            print(f"FAIL attempt fences missing passing crash traces: {sorted(required - covered)}")
            failed = True
    threads = read_json(fixtures_dir / "threads-app.json")
    for file in sorted((fixtures_dir / "provider-marks").glob("*.json")):
        errors = Validation()
        check(schema, threads, "$", errors)
        if not errors:
            case = read_json(file, errors)
            icon = next(bind["row"]["icon"] for bind in errors.each(threads["binds"]) if bind["id"] == case["threads_bind"])
            actual = provider_mark_fixture_symbol(case, icon)
            if actual != case["expected_symbol"]:
                errors.append(f"expected icon {case['expected_symbol']!r}, got {actual!r}")
        failed |= not report_fixture(file, errors, expected)
    return int(failed)


def load_package_templates(manifest, package_dir, explicit_apps, inputs, errors):
    """Resolve declared app paths and verify each template's pinned bytes."""
    templates = {}
    for app in errors.each(manifest["apps"]):
        asset = app["description"]["template"]
        path = package_dir / asset["path"]
        sha256 = asset["sha256"]
        if templates.get(path, sha256) != sha256:
            raise ValueError(f"{path}: conflicting template digests")
        templates[path] = sha256
    if explicit_apps:
        supplied = {inputs.identify(file) for file in errors.each(explicit_apps)}
        if len(supplied) != len(explicit_apps):
            raise ValueError("Duplicate --app path")
    else:
        supplied = {inputs.identify(path) for path in errors.each(templates)}
    implicit_root = None if explicit_apps else package_dir
    assets = {file: inputs.bytes(file, implicit_root=implicit_root)[1] for file in errors.each(sorted(supplied))}
    digests = {file: hashlib.sha256(data).hexdigest() for file, data in errors.each(assets.items())}
    used = set()
    for declared, digest in errors.each(templates.items()):
        chosen = inputs.identify(declared)
        if chosen not in assets and explicit_apps:
            names = [file for file in errors.each(assets) if file.name == declared.name and digests[file] == digest]
            if not names:
                names = [file for file in errors.each(assets) if digests[file] == digest]
            if len(names) == 1:
                chosen = names[0]
        if digests.get(chosen) != digest:
            raise ValueError(f"{declared}: missing app template or SHA-256 mismatch")
        used.add(chosen)
        yield declared, chosen, load_strict(assets[chosen].decode("utf-8"), errors)
    if extra := supplied - used:
        raise ValueError(f"Undeclared app templates: {', '.join(str(file) for file in errors.each(sorted(extra)))}")


def package_manifest(path, inputs, errors):
    """Read and validate an emitted manifest, including its declared schemas."""
    _, data = inputs.bytes(path)
    return parse_manifest(data.decode("utf-8"), errors)


def check_package_owners(owners, errors):
    """Reject duplicate package and kind identities before linking across manifests."""
    packages = set()
    kinds = set(verification.CATALOGUE["kinds"])
    providers = set()
    for owner in errors.each(owners):
        if owner["id"] in packages:
            errors.append(f"Duplicate installed package ID {owner['id']!r}")
        packages.add(owner["id"])
        for kind in errors.each(owner["kinds"]):
            if kind in kinds:
                errors.append(f"Duplicate installed kind ID {kind!r}")
            kinds.add(kind)
        for provider in errors.each(owner.get("providers", [])):
            ident = provider["id"]
            if ident in providers:
                errors.append(f"Duplicate installed provider ID {ident!r}")
            providers.add(ident)


def run_package(args):
    """Verify emitted JSON artifacts directly, independent of the fixture tree."""
    if verification.CATALOGUE is None:
        raise ValueError("Package verification requires the pinned host catalogue")
    if len(args.app) > 16 or len(args.installed_manifest) > 32 or len(args.event) > 64 or len(args.reply) > 64:
        raise ValueError("Package argument caps are 16 apps, 32 installed manifests, 64 events and 64 replies")
    boundary = lexical_path(args.package) if args.package else None
    path = boundary / "manifest.json" if boundary is not None else resolved_path(args.manifest)
    with PackageInput(boundary, Validation()) as inputs:
        return verify_package(args, path, inputs)


def verify_package(args, path, inputs):
    """Validate package artifacts while the directory descriptor stays open."""
    package_dir = path.parent
    errors = inputs.errors
    explicit_origins = inputs.json(args.origins) if args.origins else None
    owners = []
    for file in errors.each((path, *args.installed_manifest)):
        owner = package_manifest(file, inputs, errors)
        if errors:
            report_fixture(file, errors, {})
            return 1
        owners.append(owner)
    manifest, *installed = owners
    check_package_owners(owners, errors)
    by_package = {package["id"]: package for package in owners}
    for contribution in errors.each(manifest.get("contributions", [])):
        target = contribution["app"]
        owner = by_package.get(target["package"])
        if owner is None:
            continue  # The installer must resolve targets absent from an offline package check.
        app = next((entry for entry in errors.each(owner["apps"]) if entry["id"] == target["app"]), None)
        point = next((entry for entry in errors.each((app or {}).get("extension_points", []))
                      if entry["id"] == contribution["point"]), None)
        if not point or (point["kind"], point["version"]) != (contribution["kind"], contribution["version"]):
            errors.append(f"$.contributions.{contribution['id']}.app: no matching qualified extension point")
    if args.package:
        for contribution in errors.each(manifest.get("contributions", [])):
            entry = contribution.get("harness", {}).get("entry")
            if entry is None:
                continue
            entry_path = package_dir / entry["path"]
            _, data = inputs.bytes(entry_path, implicit_root=package_dir)
            if hashlib.sha256(data).hexdigest() != entry["sha256"]:
                errors.append(f"{entry_path}: harness entry SHA-256 mismatch")
    if not report_fixture(path, errors, {}):
        return 1

    sources = {}
    for declared, chosen, doc in load_package_templates(manifest, package_dir, args.app, inputs, errors):
        validate_description(doc, manifest, installed, errors)
        if not report_fixture(chosen, errors, {}):
            return 1
        sources[inputs.identify(declared)] = sources[chosen] = (doc, manifest, installed)

    event_bindings = {}

    def index_origin(source):
        doc, owner, _ = source
        if id(doc) not in event_bindings:
            event_bindings[id(doc)] = EventBindings(doc, errors, owner)
        return event_bindings[id(doc)]

    for description_path, file in errors.each(args.event):
        declared = inputs.identify(package_dir / description_path)
        source = sources.get(declared)
        if source is None:
            raise ValueError(f"{file}: unknown event origin description {description_path}")
        validate_event(inputs.json(file), lambda: index_origin(source), errors)
        if not report_fixture(file, errors, {}):
            return 1

    replies = args.reply
    if inputs.root is not None and not replies:
        replies = inputs.reply_files(package_dir / "replies")
    if not replies:
        return 0
    if len(replies) > 64:
        raise ValueError("Package exceeds 64 reply artifacts")

    if args.origins is None:
        try:
            explicit_origins = inputs.json(package_dir / "replies" / "origins.json")
        except FileNotFoundError as error:
            raise ValueError("Reply verification requires --origins or replies/origins.json") from error
    names = {file.name for file in errors.each(replies)}
    if len(names) != len(replies):
        raise ValueError("Reply filenames must be unique")
    origins = load_reply_origins(explicit_origins, package_dir, sources, verification.CATALOGUE, names, errors,
                                 identify=inputs.identify)
    for file in errors.each(replies):
        validate_reply(inputs.json(file), origins[file.name], errors)
        if not report_fixture(file, errors, {}):
            return 1
    return 0


def main():
    parser = argparse.ArgumentParser(description="Verify pinned description fixtures or emitted package artifacts.")
    parser.add_argument("--version", action="version", version="Calternal v2 conformance tool 1.1.0")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--package", type=pathlib.Path, metavar="DIR", help="read manifest.json, declared templates and optional replies/*.json from DIR")
    mode.add_argument("--fixture-testing", action="store_true", help="allow CALTERNAL_HOST_CATALOGUE for fixture diagnostics")
    mode.add_argument("--manifest", type=pathlib.Path, metavar="FILE", help="read a manifest at an explicit path")
    parser.add_argument("--app", type=pathlib.Path, action="append", default=[], metavar="FILE", help="an explicit declared app template; repeat for each")
    parser.add_argument("--event", nargs=2, type=pathlib.Path, action="append", default=[], metavar=("DESCRIPTION", "FILE"),
                        help="verify an event against its declared app template; repeat as needed")
    parser.add_argument("--reply", type=pathlib.Path, action="append", default=[], metavar="FILE", help="an optional WindowReply artifact; repeat for each")
    parser.add_argument("--origins", type=pathlib.Path, metavar="FILE", help="reply origins keyed by description path and Bind (default replies/origins.json)")
    parser.add_argument("--installed-manifest", type=pathlib.Path, action="append", default=[], metavar="FILE",
                        help="an installed manifest owning a referenced kind; repeat as needed")
    args = parser.parse_args()
    if not (args.manifest or args.package) and (args.app or args.event or args.reply or args.origins or args.installed_manifest):
        parser.error("explicit artifacts require --manifest or --package")
    try:
        if args.fixture_testing:
            verification.use_fixture_catalogue()
        return run_package(args) if args.manifest or args.package else run_fixtures()
    except (DuplicateKey, ValueError, OSError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
