"""Sibling fixtures for scoped routes, renewals, search and host-owned fields."""

import json

from description_events import check_route_capacity
from description_input import Validation
from description_verification import CATALOGUE, parse_manifest, validate_reply


class ScopeCases:
    """Run scope fixtures through the normal package and reply verifier paths."""

    def test_every_composer_renewal_uses_the_latest_authority(self):
        self.stage("threads-app")
        fixture = self.fixtures / "events/threads-app/event-catalogue-composer-chain-changed-grant-and-lease.json"
        def capture():
            case = json.loads(fixture.read_bytes())
            renewal = case["renewals"][-1]
            return case, renewal["request"], renewal["result"]

        case, request, result = capture()
        request["result_versions"]["w"] = "result"
        result["retained_results"].append({"window_ref": "w", "result_version": "result"})
        self.event(case)

        for field, value in (("lease_ref", "lease"), ("grant_revision", "current"),
                             ("cache_partition", "another")):
            with self.subTest(request=field):
                case, request, _ = capture()
                request[field] = value
                self.event(case, "last accepted reply")

        for field, value in (("request_lease_ref", "lease"), ("renewal_id", "renewing-2"),
                             ("cache_partition", "another")):
            with self.subTest(response=field):
                case, _, result = capture()
                result[field] = value
                self.event(case, "last accepted reply")

        for name, reason in (
            ("reuse-id", "renewal ID was already used"),
            ("reply-lease-partition", "last accepted reply"),
            ("missing-intermediate-context", "missing issued context"),
            ("retired-id", "expected a composer action issued"),
            ("replaced-context", "must match the issued composer action context"),
            ("omitted-map", "expected a composer action issued"),
            ("unrequested-window", "requested exact result under the unchanged grant"),
            ("revoked-grant-window", "requested exact result under the unchanged grant"),
            ("reissued-token", "reused composer context from a different lease or grant"),
        ):
            with self.subTest(edge=name):
                case, request, result = capture()
                if name == "reuse-id":
                    request["renewal_id"] = case["renewals"][-2]["request"]["renewal_id"]
                elif name == "reply-lease-partition":
                    result["lease"]["cache_partition"] = "another"
                elif name == "missing-intermediate-context":
                    case["renewals"][2]["result"]["activation_contexts"].clear()
                elif name == "retired-id":
                    case["event"]["action_id"] = "composer.stop.5"
                elif name == "replaced-context":
                    case["event"]["activation_context"] = "stop-context-5"
                elif name == "omitted-map":
                    del result["composer_actions"]
                elif name == "reissued-token":
                    issue = case["event"]["action_id"]
                    result["activation_contexts"][issue] = "context-send"
                    case["event"]["activation_context"] = "context-send"
                elif name == "unrequested-window":
                    result["retained_results"].append({"window_ref": "w", "result_version": "result"})
                else:
                    renewal = case["renewals"][3]
                    renewal["request"]["result_versions"]["w"] = "result"
                    renewal["result"]["retained_results"].append({"window_ref": "w", "result_version": "result"})
                self.event(case, reason)

    def test_composer_context_has_one_binding_across_renewals(self):
        self.stage("threads-app")
        app = json.loads((self.package / self.template).read_bytes())
        panel = app["chrome"]["panel"]["components"]
        panel.append(next(component for component in panel if component["id"] == "panel-composer")
                     | {"id": "other-composer"})
        self.replace_template(app)
        fixture = self.fixtures / "events/threads-app/event-catalogue-composer-starting-stop-after-renewal.json"
        for component, purpose, action, failure in (
            ("panel-composer", "send", "composer.17", None),
            ("other-composer", "send", "composer.17", "rebound"),
            ("panel-composer", "stop", "composer.17", "rebound"),
            ("panel-composer", "send", "composer.changed", "rebound"),
            ("panel-composer", "stop", "composer.changed", "rebound"),
        ):
            with self.subTest(component=component, purpose=purpose, action=action):
                case = json.loads(fixture.read_bytes())
                result = case["renewals"][0]["result"]
                result["lease"] = case["view_open"]["lease"]
                result["composer_actions"] = {component: {purpose: action}}
                result["activation_contexts"] = {action: "context-send"}
                case["event"].update(action_id=action, activation_context="context-send")
                self.event(case, f"composer context was {failure}" if failure else None)

        case = json.loads(fixture.read_bytes())
        case["view_open"]["composer_actions"]["panel-composer"]["stop"] = "composer.added-stop"
        case["view_open"]["activation_contexts"]["composer.added-stop"] = "context-send"
        self.event(case, "composer context was rebound")

    def test_open_accepts_each_chrome_and_root_operand(self):
        doc = json.loads((self.package / self.template).read_bytes())
        chrome = doc["chrome"]
        share = chrome["header"]["trailing"][0]
        share["shell"]["item"]["route"] = "header_item"
        section = chrome["sidebar"]["sections"][2]
        for slot, route in (("actions", "section_item"), ("menu", "menu_item")):
            section[slot] = [share | {"id": slot, "shell": {"sheet": "share", "item": {"route": route}}}]
        chrome["footer"]["account"]["menu"].append(
            share | {"id": "footer-share", "shell": {"sheet": "share", "item": {"route": "footer_item"}}}
        )
        self.replace_template(doc)
        event = {"type": "view_open", "view_ref": "current"}
        self.event(event | {"route": {name: "ref" for name in
                   ("item", "collection", "header_item", "section_item", "menu_item", "footer_item")}})
        self.event(event | {"route": {"view": "notes"}}, "undeclared route operand 'view'")
        self.stage("threads-app")
        self.event(event | {"route": {"thread": "checked-thread"}})
        self.event(event | {"route": {"unrelated": "ref"}}, "undeclared route operand")

    def test_row_actions_consume_optional_operands_only_for_their_bind(self):
        doc = json.loads((self.package / self.template).read_bytes())
        bind = next(bind for bind in doc["binds"] if bind["id"] == "notes")
        bind["row"]["menu"].append({
            "id": "create-related", "title": "Create related", "operation": "host.item.create",
            "inputs": {"kind": {"literal": "notes"}, "collection": {"route": "destination"}},
        })
        self.replace_template(doc)
        route = {"collection": "scope", "destination": "action-scope"}
        self.event({"type": "window", "request": {"bind": "notes", "from": 0, "to": 30, "route": route}})
        selection = {"type": "selection", "bind": "notes", "row": "row"}
        self.event(selection | {"route": route})
        self.event(selection | {"route": {"collection": "scope"}})
        self.event(selection | {"route": route | {"another": "ref"}}, "undeclared route operand 'another'")
        self.event({"type": "view_open", "view_ref": "current", "route": {"destination": "ref"}},
                   "undeclared route operand 'destination'")
        self.event({"type": "window", "request": {"bind": "note", "from": 0, "to": 1,
                    "route": {"item": "ref"}}})

    def test_required_route_cap_counts_source_and_both_predicate_groups(self):
        doc = json.loads((self.package / self.template).read_bytes())
        bind = next(bind for bind in doc["binds"] if bind["id"] == "notes")
        def pred(name):
            return {"field": "title", "op": "eq", "value": {"route": name}}
        bind["filter"] = {"all": [pred(f"value_{index}") for index in range(4)],
                          "any": [pred(f"value_{index}") for index in range(4, 7)] + [pred("value_0")]}
        self.replace_template(doc)
        request = {"bind": "notes", "from": 0, "to": 30,
                   "route": {"collection": "scope", **{f"value_{index}": "x" for index in range(7)}}}
        self.event({"type": "window", "request": request})
        for name in ("collection", "value_0", "value_5"):
            with self.subTest(missing=name):
                missing = {key: value for key, value in request["route"].items() if key != name}
                self.event({"type": "window", "request": request | {"route": missing}},
                           f"missing required route operand {name!r}")
        bind["filter"]["any"][-1] = pred("overflow")
        self.replace_template(doc)
        self.verify(failure="needs more than 8 distinct route operands")

    def test_each_row_action_must_fit_alongside_the_query(self):
        original = (self.package / self.template).read_bytes()
        for slot in ("open", "menu", "leadingActions", "trailingActions"):
            with self.subTest(slot=slot):
                doc = json.loads(original)
                index, bind = next((i, b) for i, b in enumerate(doc["binds"]) if b["id"] == "notes")
                bind["filter"] = {
                    group: [{"field": "title", "op": "eq", "value": {"route": f"value_{i}"}} for i in items]
                    for group, items in (("all", range(4)), ("any", range(4, 7)))
                }
                action = {"id": "create-related", "title": "New related note", "operation": "host.item.create",
                          "inputs": {"kind": {"literal": "notes"}, "collection": {"route": "collection"}}}
                bind["row"][slot] = {"invoke": action} if slot == "open" else [action]
                self.replace_template(doc)
                self.verify()
                action["inputs"]["collection"]["route"] = "destination"
                self.replace_template(doc)
                suffix = ".invoke" if slot == "open" else "[0]"
                self.verify(failure=f"$.binds[{index}].row.{slot}{suffix}: needs more than 8 distinct route operands")

    def test_view_consumer_cap_covers_chrome_root_and_ignores_route_targets(self):
        def operation(names):
            return {"id": "act", "operation": "fixture.action", "inputs": {
                str(i): {"route": name} for i, name in enumerate(names)
            }, "then": {"route": {"name": "navigation_target"}}}
        for surface, build in (
            ("header", lambda action: {"trailing": [action]}),
            ("sidebar", lambda action: {"sections": [{"menu": [action], "actions": [action],
                                                       "rows": [{"open": {"invoke": action}}]}]}),
            ("footer", lambda action: {"account": {"menu": [action]}}),
            ("panel", lambda action: {"components": [{"type": "composer", "config": action["inputs"]}]}),
        ):
            with self.subTest(surface=surface):
                for count in (8, 9):
                    errors = Validation()
                    action = operation([f"value_{i}" for i in range(count)])
                    check_route_capacity({"chrome": {surface: build(action)}}, 8, errors)
                    self.assertEqual(bool(errors), count == 9, list(errors))
                    self.assertTrue(all(f"$.chrome.{surface}" in e and "needs more than 8" in e for e in errors))

        editors = [{"type": "item-editor", "config": {"item": {"route": f"item_{i}"}}} for i in range(8)]
        for action, reason in (
            (operation(["item_0"]), None),
            (operation(["extra"]), "needs more than 8"),
            ({"id": "share", "shell": {"sheet": "share", "item": {"route": "extra"}}}, "needs more than 8"),
        ):
            with self.subTest(action=action):
                errors = Validation()
                check_route_capacity({"view": {"root": {"children": [*editors, action]}}}, 8, errors)
                self.assertEqual(bool(errors), reason is not None, list(errors))
                if reason:
                    self.assertTrue(any(reason in e for e in errors), list(errors))
        errors = Validation()
        check_route_capacity({"chrome": {"header": {"trailing": [operation([
            f"value_{i}" for i in range(8)
        ]), {"id": "other", "shell": {"item": {"route": "separate"}}}]}}}, 8, errors)
        self.assertFalse(errors, list(errors))

    def test_content_only_open_requires_a_wire_representable_union(self):
        def editor(index):
            return {"type": "component", "component": {"type": "item-editor", "id": f"editor-{index}",
                    "config": {"item": {"route": f"item_{index}"}}}}
        children = [editor(index) for index in range(8)]
        doc = {"version": 2, "kind": "app", "revision": 1, "binds": [],
               "view": {"root": {"type": "stack", "children": children}}}
        self.replace_template(doc)
        self.event({"type": "view_open", "view_ref": "current",
                    "route": {f"item_{index}": "ref" for index in range(8)}})
        children.append(editor(8))
        self.replace_template(doc)
        self.verify(failure="$.view.root: needs more than 8 distinct route operands")

    def test_claimed_text_settle_scope_matrix(self):
        original = (self.package / self.template).read_bytes()
        for parent in (False, True):
            for collection in (None, {"literal": "collection-id"}, {"route": "collection"}):
                with self.subTest(parent=parent, collection=collection):
                    doc = json.loads(original)
                    notes = next(bind for bind in doc["binds"] if bind["id"] == "notes")
                    search = next(bind for bind in doc["binds"] if bind["id"] == "note-search")
                    search["source"].pop("collection")
                    scope = {}
                    if parent:
                        search["source"]["parent"] = True
                        notes["row"]["children"] = {"bind": "note-search"}
                        doc["chrome"]["footer"].pop("search")
                        scope["parent"] = "issued-child-edge"
                    if collection is not None:
                        search["source"]["collection"] = collection
                    if collection == {"route": "collection"}:
                        search["filter"] = {"all": [{"field": "title", "op": "eq", "value": {"route": "match_title"}}],
                                            "any": [{"field": "title", "op": "eq", "value": {"route": "other_title"}}]}
                        scope["route"] = {"collection": "checked", "match_title": "A", "other_title": "A"}
                    self.replace_template(doc)
                    current = {"bind": "note-search", "settle_generation": 4} | scope
                    settle = {"type": "search", "text": "test text"} | current
                    self.event(settle)
                    self.event({"type": "window", "request": current | {"from": 0, "to": 30}})
                    self.event({"type": "selection", "row": "row-in-scope"} | current)
                    self.event({"type": "selection", "row": "row-in-scope"} |
                               {key: value for key, value in current.items() if key != "settle_generation"},
                               "required for search Binds only")
                    bad = dict(settle)
                    if parent:
                        del bad["parent"]
                    else:
                        bad["parent"] = "unscoped"
                    self.event(bad, "parent scope must match source.parent")

    def test_conversation_search_parent_edges_and_selection_sets(self):
        self.stage("threads-nested-catalogue-app")
        doc = json.loads((self.package / self.template).read_bytes())
        for bind in doc["binds"]:
            if bind["id"] in ("children", "in-section"):
                bind["source"]["type"] = "search"
                bind["window"]["count"] = "none"
        self.replace_template(doc)
        for name in ("children", "in-section"):
            with self.subTest(edge=name):
                scope = {"bind": name, "settle_generation": 4, "parent": "issued-parent"}
                self.event({"type": "search", "text": "match"} | scope)
                self.event({"type": "window", "request": scope | {"from": 0, "to": 30}})
                self.event({"type": "selection", "row": "child"} | scope)
                self.event({"type": "search", "text": "match", "bind": name, "settle_generation": 4},
                           "parent scope must match source.parent")

        self.stage("notes-bulk-and-snooze-app")
        doc = json.loads((self.package / self.template).read_bytes())
        manifest = json.loads(self.manifest.read_bytes())
        manifest["kinds"][0]["searchable"] = True
        self.manifest.write_text(json.dumps(manifest), encoding="utf-8")
        bind = next(bind for bind in doc["binds"] if bind["id"] == "notes")
        bind.pop("persist", None)
        bind["source"]["type"] = "search"
        bind["window"]["count"] = "none"
        self.replace_template(doc)
        for as_component in (False, True):
            with self.subTest(component=as_component):
                request = {"type": "selection_set", "bind": "notes", "rows": ["row"]}
                def envelope(request):
                    return {"type": "component", "route": "selection.set",
                            "component": "notes", "request": request} if as_component else request
                self.event(envelope(request | {"settle_generation": 4}))
                self.event(envelope(request), "required for search Binds only")
                self.event(envelope(request | {"settle_generation": 4, "parent": "random"}), "parent scope")

    def test_every_extractor_output_type_rejects_manifest_defaults(self):
        original = (self.fixtures / "manifests" / "notes-tags-app.json").read_bytes()
        for kind, default in (("Text", ""), ("TextSet", []), ("Boolean", False),
                              ("Number", 0), ("Timestamp", "2026-11-20T00:00:00Z")):
            with self.subTest(frontmatter=kind):
                manifest = json.loads(original)
                declaration = manifest["kinds"][0]
                declaration["indexed_fields"]["extra"] = {
                    "type": kind, "audience": "member_item", "body_derived": True,
                }
                declaration["extractors"].append({"selector": "frontmatter", "key": "extra", "output": "extra"})
                errors = Validation()
                parse_manifest(json.dumps(manifest), errors)
                self.assertFalse(errors, list(errors))
                declaration["host_create"]["defaults"]["extra"] = default
                parse_manifest(json.dumps(manifest), errors)
                self.assertTrue(any(".host_create.defaults.extra: extractor-owned output" in e for e in errors),
                                list(errors))
        for selector in ("inline_tags", "frontmatter"):
            with self.subTest(selector=selector):
                manifest = json.loads(original)
                declaration = manifest["kinds"][0]
                declaration["extractors"] = [x for x in declaration["extractors"] if x["selector"] == selector]
                declaration["host_create"]["defaults"]["tags"] = ["work"]
                errors = Validation()
                parse_manifest(json.dumps(manifest), errors)
                self.assertTrue(any(".host_create.defaults.tags: extractor-owned output" in e for e in errors),
                                list(errors))

    def test_conversation_attempt_and_parent_are_distinct_projected_fields(self):
        origin = (
            {"id": "threads", "source": {"type": "list"}, "fields": ["active_execution", "parent"]},
            {"fields": CATALOGUE["conversation_capability"]["field_projections"]},
            CATALOGUE["generic_item_fields"],
        )
        reply = {
            "audience": "member", "bind": "threads", "state": "ready",
            "result_version": "v1", "offset": 0, "window_ref": "thread-window",
            "rows": [{"id": "conversation", "fields": {"parent": "parent-item", "active_execution": "attempt"}}],
        }
        for attempt in ("exact-admitted-attempt", None, {"kind": "intent", "ref": "intent"}):
            with self.subTest(attempt=attempt):
                errors = Validation()
                reply["rows"][0]["fields"]["active_execution"] = attempt
                validate_reply(reply, origin, errors)
                if isinstance(attempt, dict):
                    self.assertTrue(any("active_execution: matches none of the anyOf branches" in e for e in errors), list(errors))
                else:
                    self.assertFalse(errors, list(errors))
        reply["rows"][0]["fields"]["active_execution"] = "attempt"
        errors = Validation()
        fields = reply["rows"][0]["fields"]
        fields["active_turn"] = fields.pop("active_execution")
        validate_reply(reply, origin, errors)
        self.assertTrue(any("active_turn: field is not declared" in e for e in errors), list(errors))


    def test_every_catalogue_window_audience_against_projected_fields(self):
        fields_by_kind = {
            "conversation": CATALOGUE["conversation_capability"]["field_projections"],
            "item": CATALOGUE["generic_item_fields"],
        } | {name: kind["fields"] for name, kind in CATALOGUE["kinds"].items()}
        for name in ("notes-tags-app.json", "threads-app.json", "reading-list-manifest-v2-app.json"):
            manifest = json.loads((self.fixtures / "manifests" / name).read_bytes())
            for kind in manifest["kinds"]:
                fields_by_kind[f"{name}/{kind['id']}"] = kind.get("indexed_fields", {})
        values = {"Timestamp": "2026-11-20T00:00:00Z", "Boolean": False, "Number": 0,
                  "TextSet": [], "RowImageRef": "fixture-preview-image-ref"}
        audiences = CATALOGUE["$defs"]["WindowAudience"]["enum"]
        tested = set()
        for kind, fields in fields_by_kind.items():
            for field, meta in fields.items():
                policy = meta["audience"]
                tested.add(policy)
                for audience in audiences:
                    with self.subTest(kind=kind, field=field, audience=audience):
                        origin = ({"id": "rows", "source": {"type": "list"}, "fields": [field]},
                                  {"fields": fields}, {})
                        reply = {"audience": audience, "bind": "rows", "state": "ready",
                                 "result_version": "v1", "offset": 0, "window_ref": "window",
                                 "rows": [{"id": "ref", "fields": {field: values.get(meta["type"], "ref")}}]}
                        errors = Validation()
                        validate_reply(reply, origin, errors)
                        if policy == "shared_projection" or audience in ("member", "own_space_member"):
                            self.assertFalse(errors, list(errors))
                        else:
                            self.assertTrue(any(f"fields.{field}: {policy} is unavailable" in e
                                                for e in errors), list(errors))
        self.assertEqual(tested, set(CATALOGUE["$defs"]["FieldAudience"]["enum"]))

    def test_execution_field_needs_a_ready_reply_in_both_member_audiences(self):
        meta = CATALOGUE["conversation_capability"]["field_projections"]["active_execution"]
        origin = ({"id": "threads", "source": {"type": "search"}, "fields": ["active_execution"]},
                  {"fields": {"active_execution": meta}}, {})
        for audience in ("own_space_member", "member", "guest", "public"):
            with self.subTest(audience=audience):
                reply = {"audience": audience, "bind": "threads", "state": "refine_query",
                         "result_version": None, "offset": 0, "window_ref": None, "settle_generation": 1,
                         "rows": [{"id": "conversation", "fields": {"active_execution": "ref"}}]}
                errors = Validation()
                validate_reply(reply, origin, errors)
                self.assertTrue(any("active_execution: eligible_execution_actor is unavailable" in e
                                    for e in errors), list(errors))

    def test_empty_extractor_projections_keep_present_empty_sets_and_omit_absent_scalars(self):
        types = {"tags": "TextSet", "summary": "Text", "excerpt": "Text", "text": "Text",
                 "flag": "Boolean", "number": "Number", "date": "Timestamp"}
        kind = {"fields": {key: {"type": value, "audience": "member_item"} for key, value in types.items()}}
        origin = ({"id": "notes", "source": {"type": "list"}, "fields": list(types)}, kind, {})
        values = {"tags": [], "flag": False, "number": 0}
        reply = {"audience": "own_space_member", "bind": "notes", "state": "ready", "result_version": "v1",
                 "offset": 0, "window_ref": "window", "rows": [{"id": "row", "fields": values}]}
        errors = Validation()
        validate_reply(reply, origin, errors)
        self.assertFalse(errors, list(errors))
        for field in types:
            for bad in (None, ""):
                with self.subTest(field=field, value=bad):
                    errors = Validation()
                    reply["rows"][0]["fields"] = values | {field: bad}
                    validate_reply(reply, origin, errors)
                    self.assertTrue(any(f"fields.{field}:" in e for e in errors), list(errors))
