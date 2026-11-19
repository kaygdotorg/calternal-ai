"""Sibling fixtures for scoped routes, renewals, search and host-owned fields."""

import json

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
        for audience in ("guest", "public"):
            with self.subTest(audience=audience):
                errors = Validation()
                validate_reply(reply | {"audience": audience}, origin, errors)
                self.assertTrue(any("execution actor fields require a ready member reply" in e for e in errors),
                                list(errors))
        errors = Validation()
        fields = reply["rows"][0]["fields"]
        fields["active_turn"] = fields.pop("active_execution")
        validate_reply(reply, origin, errors)
        self.assertTrue(any("active_turn: field is not declared" in e for e in errors), list(errors))
