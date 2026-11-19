"""Revision 14 review fixtures for metadata types, contexts and reserved namespaces."""

import json
from unittest.mock import patch

import description_verification as verification
from description_input import Validation


class ReviewCases:
    """Run revised checks through the package and manifest entry points."""

    def test_context_operands_need_a_real_row_not_a_bind_or_view(self):
        original = (self.package / self.template).read_bytes()
        for target in ({"context": "item"}, {"context": "placement"}, {"row": "id"},
                       {"row": "field", "field": "title"}):
            for surface in ("empty", "loading", "error", "refine_query", "header", "sidebar", "footer", "content"):
                with self.subTest(target=target, surface=surface):
                    doc = json.loads(original)
                    bind = next(b for b in doc["binds"]
                                if b["id"] == ("note-search" if surface == "refine_query" else "notes"))
                    action = {"id": "misplaced", "operation": "host.item.rename", "title": "Rename",
                              "inputs": {"item": target, "title": {"literal": "Renamed"}}}
                    chrome = doc["chrome"]
                    if surface == "header":
                        chrome["header"]["trailing"].append(action)
                    elif surface == "sidebar":
                        chrome["sidebar"]["sections"][1]["menu"] = [action]
                    elif surface == "footer":
                        chrome["footer"]["account"]["menu"].append(action)
                    elif surface == "content":
                        doc["view"]["root"] = {"type": "button", "invoke": action}
                    else:
                        bind["states"] = {surface: {"title": "Message", "action": action}}
                    self.replace_template(doc)
                    reason = ("is legal only inside the rows of a Section.repeat"
                              if target.get("context") == "placement" else "inside a bind's row template")
                    if surface == "header" and "row" in target:
                        reason = "matches a forbidden shape"
                    self.verify(failure=reason)

    def test_bind_state_prompt_prefills_have_no_row_context(self):
        original = (self.package / self.template).read_bytes()
        for target in ("item", "placement"):
            with self.subTest(target=target):
                doc = json.loads(original)
                bind = next(b for b in doc["binds"] if b["id"] == "notes")
                bind["states"] = {"empty": {"title": "Empty", "action": {
                    "id": "prompt-row-value", "title": "Rename", "operation": "host.item.rename",
                    "inputs": {"item": {"route": "item"}, "title": {"prompt": {
                        "title": "Title", "initial": {"context": target},
                    }}},
                }}}
                self.replace_template(doc)
                self.verify(failure=f"prompt.initial: {{context: {target}}} is legal only inside")

    def test_context_operands_work_in_real_item_and_placement_rows(self):
        notes = json.loads((self.package / self.template).read_bytes())
        bind = next(b for b in notes["binds"] if b["id"] == "notes")
        bind["row"]["menu"].append({
            "id": "rename-with-item", "operation": "host.item.rename", "title": "Rename",
            "inputs": {"item": {"context": "item"}, "title": {"literal": "Renamed"}},
        })
        self.replace_template(notes)
        self.verify()

        self.stage("threads-nested-catalogue-app")
        self.verify()  # Move up/down and Move to consume placement inside a Section.repeat.
        doc = json.loads((self.package / self.template).read_bytes())
        bind = next(b for b in doc["binds"] if b["id"] == "in-section")
        action = next(a for a in bind["row"]["menu"] if a["id"] == "move-up-in-section")
        bind["row"]["menu"].remove(action)
        bind["states"] = {"empty": {"title": "Empty", "action": action}}
        self.replace_template(doc)
        self.verify(failure="states.empty.action.inputs.entry: {context: placement} is legal only inside")

    def test_canonical_title_declaration(self):
        original = self.manifest.read_bytes()
        reason = "title_field: generic title requires a canonical string declaration in schema.properties"
        string = {"type": "string"}
        others = {"type": ["object", "array", "boolean", "null", "number"]}
        canonical = [
            string, {"type": ["string"]}, {"type": ["string"], "minLength": 1},
            {"$ref": "#/definitions/label"},
            {"type": "string", "oneOf": [{"const": "Title"}, {"pattern": "^Other"}]},
        ]
        negative = [
            {"$ref": "https://calternal.cloud/contract/host-catalogue-v2.json#/$defs/Text"},
            {"const": "Title"}, {"enum": ["A", "B"]},
            {"allOf": [{"type": ["string", "null"]}, {"not": {"type": "null"}}]},
            {"allOf": [{"type": ["string", "null"]}, {"not": {"const": None}}]},
            {"allOf": [{"type": ["string", "boolean"]}, {"not": {"enum": [True, False]}}]},
            {"anyOf": [string, {"$ref": "#/definitions/never"}]},
            {"anyOf": [string, {"const": "A"}]}, {"oneOf": [string, False]},
            {"oneOf": [True, others]}, {"not": others},
            {"if": string, "then": True, "else": False},
            {"if": others, "then": False},
            True, False, {"type": ["string", "null"]}, {"enum": ["A", False]},
            {"anyOf": [string, {"type": "boolean"}]},
            {"$ref": "#/definitions/number", "type": "string"},
            {"oneOf": [string, {"type": "null"}]},
            {"if": string, "then": True},
            {"allOf": [string, {"type": "number"}]},
        ]
        for shape, valid in [(shape, True) for shape in canonical] + [(shape, False) for shape in negative]:
            with self.subTest(shape=shape):
                manifest = json.loads(original)
                metadata = manifest["kinds"][0]["schema"]
                metadata["properties"]["title"] = shape
                metadata["definitions"] = {"label": {"type": ["string"]}, "number": {"type": "number"}, "never": False}
                errors = Validation()
                verification.parse_manifest(json.dumps(manifest), errors)
                if valid:
                    self.assertFalse(errors, list(errors))
                else:
                    self.assertTrue(any(reason in e for e in errors), list(errors))

        for wrapper in (
            {"properties": {"title": {}}, "patternProperties": {"^title$": string}},
            {"patternProperties": {"^title$": string}},
            {"properties": {"title": {}}, "additionalProperties": string},
            {"$ref": "#/definitions/metadata", "definitions": {"metadata": {"properties": {"title": string}}}},
            {"allOf": [{"properties": {"title": {"type": ["string", "null"]}}},
                       {"properties": {"title": {"not": {"type": "null"}}}}]},
            {"anyOf": [{"properties": {"title": string}}, {"properties": {"title": {"const": "Only"}}}]},
            {"type": "object", "required": ["title"], "oneOf": [
                {"properties": {"title": {"const": "First"}}}, {"properties": {"title": {"const": "Second"}}},
            ]},
        ):
            with self.subTest(wrapper=wrapper):
                manifest = json.loads(original)
                manifest["kinds"][0]["schema"] = wrapper
                errors = Validation()
                verification.parse_manifest(json.dumps(manifest), errors)
                self.assertTrue(any(reason in e for e in errors), list(errors))

        for scoped_type in ("string", "number"):
            with self.subTest(scoped_type=scoped_type):
                manifest = json.loads(original)
                manifest["kinds"][0]["schema"] = {
                    "properties": {"title": {"$ref": "scoped-title#/definitions/alias"}},
                    "definitions": {
                        "label": {"type": "number" if scoped_type == "string" else "string"},
                        "scoped": {"$id": "scoped-title", "type": "object",
                                   "definitions": {"label": {"type": scoped_type},
                                                   "alias": {"$ref": "#/definitions/label"}}},
                    },
                }
                errors = Validation()
                verification.parse_manifest(json.dumps(manifest), errors)
                if scoped_type == "string":
                    self.assertFalse(errors, list(errors))
                else:
                    self.assertTrue(any(reason in e for e in errors), list(errors))

    def test_index_types_are_declared_independently_of_metadata(self):
        self.stage("notes-tags-app")
        original = json.loads(self.manifest.read_bytes())
        kind = original["kinds"][0]
        kind["schema"]["properties"]["tags"] = {"type": "array", "items": {"type": "string"}}
        template = json.loads((self.package / self.template).read_bytes())
        bind = next(b for b in template["binds"] if b["id"] == "notes")
        self.manifest.write_text(json.dumps(original))
        self.replace_template(template)
        self.verify()

        # A metadata array cannot stand in for the declared extractor destination type.
        tags = kind["indexed_fields"].pop("tags")
        errors = Validation()
        verification.parse_manifest(json.dumps(original), errors)
        self.assertTrue(any("output 'tags' requires a declared indexed field type" in e for e in errors), list(errors))

        # With no extractor, a metadata array still cannot serve a contains operand.
        kind.pop("extractors")
        self.manifest.write_text(json.dumps(original))
        self.replace_template(template)
        self.verify(failure="contains requires a TextSet and one Text operand")

        # Even a declared set cannot sort. A canonical generic scalar can.
        kind["indexed_fields"]["tags"] = tags
        self.manifest.write_text(json.dumps(original))
        bind["sort"] = [{"field": "tags", "direction": "asc"}]
        self.replace_template(template)
        self.verify(failure="TextSet cannot be a sort key")
        bind["sort"] = [{"field": "updated", "direction": "desc"}]
        self.replace_template(template)
        self.verify()

        self.stage("threads-nested-catalogue-app")
        template = json.loads((self.package / self.template).read_bytes())
        bind = next(b for b in template["binds"] if b["id"] == "in-section")
        bind["sort"] = [{"field": "provider_icon", "direction": "asc"}]
        self.replace_template(template)
        self.verify(failure="sort field 'provider_icon' requires a declared indexed field type")

        self.stage("threads-app")
        template = json.loads((self.package / self.template).read_bytes())
        bind = next(b for b in template["binds"] if b["source"]["kind"] == "requests")
        bind["sort"] = [{"field": "title", "direction": "asc"}]
        self.replace_template(template)
        self.verify(failure="sort field 'title' requires a declared indexed field type")

    def test_offline_provider_label_requires_its_paired_text_source(self):
        reply = json.loads((self.fixtures / "reply-catalogue-window-member-record.json").read_bytes())
        app = json.loads((self.fixtures / "threads-nested-catalogue-app.json").read_bytes())
        bind = next(b for b in app["binds"] if b["id"] == reply["bind"])
        errors = Validation()
        owner = verification.parse_manifest(
            (self.fixtures / "manifests/rev14-threads-owner.json").read_text(), errors)
        self.assertFalse(errors, list(errors))
        kind = verification.project_kind(owner["kinds"]["threads"], verification.CATALOGUE)
        origin = (bind, kind, verification.CATALOGUE["generic_item_fields"])
        self.assertEqual(reply["member_cache_record"]["rows"][0]["subtitle"], "Claude Code")
        verification.validate_reply(reply, origin, errors)
        self.assertFalse(errors, list(errors))
        for mode in ("missing text", "substituted mark"):
            with self.subTest(mode=mode):
                changed = json.loads(json.dumps(reply))
                live = changed["rows"][0]["fields"]
                del live["provider_label"]
                if mode == "substituted mark":
                    live["provider_icon"] = "Claude Code"
                errors = Validation()
                verification.validate_reply(changed, origin, errors)
                self.assertTrue(any("subtitle: expected only offline-eligible live secondary text" in e
                                    for e in errors), list(errors))

    def test_reserved_operation_names_come_from_pinned_catalogue(self):
        fixture = self.fixtures / "manifests/reading-list-manifest-v2-app.json"
        manifest = json.loads(fixture.read_bytes())
        for operation, catalogue, reason in (
            ("host.summarize", verification.CATALOGUE, "reserved host operation namespace"),
            ("turns.summarize", verification.CATALOGUE, None),
            ("hosted.summarize", verification.CATALOGUE, None),
            ("extension.summarize", verification.CATALOGUE | {"reserved_operation_prefixes": ["extension."]},
             "reserved host operation namespace"),
        ):
            with self.subTest(operation=operation):
                manifest["id"] = operation.split(".")[0]
                manifest["operations"][0]["id"] = operation
                errors = Validation()
                with patch.object(verification, "CATALOGUE", catalogue):
                    verification.parse_manifest(json.dumps(manifest), errors)
                if reason:
                    self.assertTrue(any(reason in e for e in errors), list(errors))
                else:
                    self.assertFalse(errors, list(errors))
