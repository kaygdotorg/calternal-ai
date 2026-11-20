"""Conformance fixtures for live and offline field provenance and action contexts."""

import json

from description_input import Validation
from description_verification import CATALOGUE, check, parse_manifest, validate_reply


class ProjectionCases:
    """Exercise the same install, event and reply checks used by the verifier."""

    def test_bind_state_actions_cannot_take_direct_or_prefill_route_inputs(self):
        original = (self.package / self.template).read_bytes()
        for state in ("empty", "loading", "error", "refine_query"):
            with self.subTest(state=state):
                doc = json.loads(original)
                bind = next(b for b in doc["binds"] if b["id"] == ("note-search" if state == "refine_query" else "notes"))
                action = {"id": "state-create", "operation": "host.item.create", "title": "New note",
                          "inputs": {"kind": {"literal": "notes"}}}
                bind["states"] = {state: {"title": "Message", "action": action}}
                self.replace_template(doc)
                self.verify()
                action["inputs"]["collection"] = {"route": "destination"}
                self.replace_template(doc)
                self.verify(failure=f"states.{state}.action: Bind-state actions cannot consume route operands")

        doc = json.loads(original)
        bind = next(b for b in doc["binds"] if b["id"] == "notes")
        action = json.loads(json.dumps(next(a for a in bind["row"]["menu"] if a["id"] == "rename")))
        action["id"] = "state-prefill"
        action["inputs"]["title"]["prompt"]["initial"] = {"route": "title"}
        bind["states"] = {"empty": {"title": "Nothing yet", "action": action}}
        self.replace_template(doc)
        self.verify(failure="states.empty.action: Bind-state actions cannot consume route operands")

    def test_title_install_cannot_relabel_protected_fields(self):
        original = (self.fixtures / "manifests" / "notes-tags-app.json").read_bytes()
        for title, meta, selector, expected in (
            ("title", {}, None, None),
            ("headline", {"type": "Text", "audience": "shared_projection"}, None, None),
            ("headline", {"type": "Text", "audience": "member_item"}, None, "requires an unrestricted metadata source"),
            ("headline", {"type": "Text", "audience": "viewer_self"}, None, "requires an unrestricted metadata source"),
            ("snippet", {}, None, "cannot use a body-derived source"),
            ("tags", {}, None, "cannot use a body-derived source"),
            ("thumbnail", {}, None, "cannot use a body-derived source"),
            ("headline", {"type": "Text", "audience": "shared_projection", "body_derived": True}, None,
             "cannot use a body-derived source"),
            ("headline", {"type": "Text", "audience": "shared_projection", "body_derived": True},
             {"output": "headline", "selector": "first_blocks", "blocks": 2, "max_utf8_bytes": 80},
             "cannot use a body-derived source"),
            ("headline", {"type": "Text", "audience": "shared_projection", "body_derived": True},
             {"output": "headline", "selector": "frontmatter", "key": "headline"}, "cannot use a body-derived source"),
            ("provider_icon", {}, None, "requires an unrestricted metadata source"),
        ):
            with self.subTest(title=title, meta=meta, selector=selector):
                manifest = json.loads(original)
                kind = manifest["kinds"][0]
                kind["title_field"] = title
                kind["schema"]["properties"][title] = {"type": "string"}
                if meta:
                    kind["indexed_fields"][title] = meta
                if selector:
                    kind["extractors"].append(selector)
                errors = Validation()
                parse_manifest(json.dumps(manifest), errors)
                if expected:
                    self.assertTrue(any(f".title_field: generic title {expected}" in msg for msg in errors),
                                    list(errors))
                else:
                    self.assertFalse(errors, list(errors))

    def test_title_sources_must_be_declared_metadata(self):
        original = self.manifest.read_bytes()
        for metadata, expected in ((None, "requires a canonical string declaration in schema.properties"),
                                   ({"type": "number"}, "requires a canonical string declaration in schema.properties"),
                                   ({"$ref": "#/definitions/headline"}, None)):
            with self.subTest(metadata=metadata):
                manifest = json.loads(original)
                kind = manifest["kinds"][0]
                kind["title_field"] = "heading"
                if metadata is not None:
                    kind["schema"]["properties"]["heading"] = metadata
                    kind["schema"]["definitions"] = {"headline": {"type": "string"}}
                errors = Validation()
                parse_manifest(json.dumps(manifest), errors)
                if expected:
                    self.assertTrue(any(f".title_field: generic title {expected}" in e for e in errors), list(errors))
                else:
                    self.assertFalse(errors, list(errors))

    def test_provider_marks_cannot_be_package_fields_or_body_backed_symbols(self):
        manifest = json.loads(self.manifest.read_bytes())
        manifest["kinds"][0]["indexed_fields"]["provider_icon"] = {"type": "Text", "audience": "shared_projection"}
        errors = Validation()
        parse_manifest(json.dumps(manifest), errors)
        self.assertTrue(any("provider_icon: reserved host generic field" in msg for msg in errors), list(errors))

        original = (self.package / self.template).read_bytes()
        for source in ("leading", "trailing icon", "trailing field"):
            with self.subTest(source=source):
                doc = json.loads(original)
                bind = next(b for b in doc["binds"] if b["id"] == "notes")
                field_icon = {"name": "notes", "field": "snippet"}
                if source == "leading":
                    bind["row"]["icon"] = field_icon
                    path = "row.icon"
                elif source == "trailing icon":
                    bind["row"]["marks"] = [{"kind": "icon", "icon": field_icon}]
                    path = "row.marks[0].icon"
                else:
                    bind["row"]["marks"] = [{"kind": "icon", "field": "snippet"}]
                    path = "row.marks[0].field"
                self.replace_template(doc)
                self.verify(failure=f"{path}: a symbol cannot use a body-derived field")

    def test_projection_grant_fixtures_for_live_text_image_and_query(self):
        guest = CATALOGUE["generic_item_fields"]["title"]["audience"]
        fields = {
            "restricted": {"type": "Text", "audience": "member_item"},
            "excerpt": {"type": "Text", "audience": guest, "source": "body_excerpt"},
            "excerpt_alt": {"type": "Text", "audience": guest, "body_derived": True},
            "preview": {"type": "RowImageRef", "audience": guest},
        }
        kind = {"fields": fields, "device_row_images": ["preview"]}
        projected = {"title": "Visible title"}
        reply = {"audience": "guest", "bind": "notes", "state": "ready", "result_version": "v1",
                 "offset": 0, "window_ref": "w", "scope_ref": "fixture-scope", "rows": [{"id": "row", "fields": projected}]}
        for use, field, source in (
            ("header title", "restricted", "item"),
            ("row title", "excerpt_alt", "list"),
            ("second line", "excerpt_alt", "list"),
            ("search excerpt", "excerpt", "search"),
            ("miniature fallback", "excerpt", "list"),
            ("row image", "preview", "list"),
        ):
            with self.subTest(use=use):
                bind = {"id": "notes", "source": {"type": source}, "fields": ["title", field],
                        "row": {"title": {"field": "title"}}}
                if use == "row title":
                    bind["row"]["title"]["field"] = field
                elif use == "second line":
                    bind["row"]["secondary"] = [{"field": field, "format": "text"}]
                elif use == "miniature fallback":
                    bind["row"]["icon"] = {"image": {"field": "preview", "fallback": field}}
                scoped = reply | ({"settle_generation": 1} if source == "search" else {})
                origin = (bind, kind, CATALOGUE["generic_item_fields"])
                errors = Validation()
                validate_reply(scoped, origin, errors, body_allowed=False)
                self.assertFalse(errors, list(errors))
                value = "image-preview-fixture-ref" if field == "preview" else "Protected text"
                delivered = scoped | {"rows": [{"id": "row", "fields": projected | {field: value}}]}
                errors = Validation()
                validate_reply(delivered, origin, errors, body_allowed=False)
                reason = "member_item is unavailable" if field == "restricted" else "requires current body authority"
                self.assertTrue(any(reason in msg for msg in errors), list(errors))
                if field != "restricted":
                    errors = Validation()
                    validate_reply(delivered, origin, errors, body_allowed=True)
                    self.assertFalse(errors, list(errors))

        for operand, field in ((operand, field) for operand in ("sort", "all", "any")
                               for field in ("excerpt", "restricted")):
            with self.subTest(operand=operand, field=field):
                bind = {"id": "notes", "source": {"type": "list"}, "fields": ["title"]}
                if operand == "sort":
                    bind["sort"] = [{"field": field, "direction": "asc"}]
                else:
                    bind["filter"] = {operand: [{"field": field, "op": "eq", "value": {"literal": "x"}}]}
                errors = Validation()
                validate_reply(reply, (bind, kind, CATALOGUE["generic_item_fields"]), errors, body_allowed=False)
                location = operand if operand == "sort" else f"filter.{operand}"
                self.assertTrue(any(f"$.bind.{location}.{field}:" in msg for msg in errors), list(errors))

    def test_cache_title_subtitle_and_date_use_only_eligible_live_sources(self):
        fixture = self.fixtures / "reply-catalogue-window-member-record-note-date.json"
        original = fixture.read_bytes()
        fields = {"snippet": {"type": "Text", "audience": "shared_projection", "source": "body_excerpt"},
                  "restricted_date": {"type": "Timestamp", "audience": "member_item"},
                  "plain": {"type": "Text", "audience": "shared_projection"}}
        base = {"id": "all-own-space-notes", "source": {"type": "list"}, "persist": True,
                "fields": ["title", "updated", "snippet", "restricted_date", "plain"]}
        for case in ("safe-date", "safe-subtitle", "body-title", "body-subtitle", "restricted-date", "invented-title"):
            with self.subTest(case=case):
                reply = json.loads(original)
                live = reply["rows"][0]["fields"]
                live["restricted_date"] = "2026-11-19T00:00:00Z"
                live["plain"] = "Safe metadata"
                cache = reply["member_cache_record"]["rows"][0]
                row = {"title": {"field": "title"},
                       "secondary": [{"field": "updated", "format": "relative_date"},
                                     {"field": "snippet", "format": "text"}]}
                reason = None
                if case == "safe-subtitle":
                    row["secondary"].insert(0, {"field": "plain", "format": "text"})
                    cache.pop("subtitle_timestamp")
                    cache["subtitle"] = live["plain"]
                elif case == "body-title":
                    row["title"]["field"] = "snippet"
                    cache["title"] = live["snippet"]
                    reason = "title: expected the offline-eligible live title"
                elif case == "body-subtitle":
                    cache.pop("subtitle_timestamp")
                    cache["subtitle"] = live["snippet"]
                    reason = "subtitle: expected only offline-eligible"
                elif case == "restricted-date":
                    row["secondary"][0]["field"] = "restricted_date"
                    cache["subtitle_timestamp"] = live["restricted_date"]
                    reason = "subtitle_timestamp: expected an offline-eligible"
                elif case == "invented-title":
                    cache["title"] = "Unpaired text"
                    reason = "title: expected the offline-eligible live title"
                errors = Validation()
                validate_reply(reply, (base | {"row": row}, {"fields": fields}, CATALOGUE["generic_item_fields"]),
                               errors, body_allowed=True)
                if reason:
                    self.assertTrue(any(reason in msg for msg in errors), list(errors))
                else:
                    self.assertFalse(errors, list(errors))

    def test_alarm_does_not_accept_host_notification_text(self):
        args = {"request_id": "alarm-request", "job": "reminder", "input": {},
                "at": "2026-11-20T00:00:00Z"}
        definition = CATALOGUE["$defs"]["AlarmSet"]
        errors = Validation()
        check(definition, args, "$", errors, CATALOGUE)
        self.assertFalse(errors, list(errors))
        for prop in ("title", "subtitle", "body", "notification_text", "snippet"):
            with self.subTest(prop=prop):
                errors = Validation()
                check(definition, args | {prop: {"field": "snippet"}}, "$", errors, CATALOGUE)
                self.assertEqual(list(errors), [f"$: unknown field {prop}"])
