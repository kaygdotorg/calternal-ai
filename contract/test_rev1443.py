"""Schema probes for private item state and checked entry views."""

import json
import unittest

from description_input import Validation
from description_verification import CATALOGUE, check, field_meta, parse_manifest, schema


class EntryViewSchemas(unittest.TestCase):
    def assert_shape(self, document, name, payload, valid=True):
        errors = Validation()
        check(document["$defs"][name], payload, "$", errors, document)
        self.assertEqual(not bool(errors), valid, list(errors))

    def test_entry_contains_content_and_no_host_overlay(self):
        entry = {
            "type": "snapshot-entry",
            "anchor": "entry-1",
            "author": {"key": "author-1", "name": "Lee"},
            "timestamp": "2026-11-20T00:00:00Z",
            "children": [
                {"type": "snapshot-text", "markup": "inline", "text": "Hello"},
                {"type": "snapshot-quote", "source": {"item": "item-1", "revision": "rev-1", "span": "span-1"}},
            ],
        }
        self.assert_shape(schema, "SnapshotEntry", entry)
        self.assert_shape(schema, "SnapshotEntry", {**entry, "own": True}, False)
        self.assert_shape(schema, "SnapshotEntry", {**entry, "children": [
            {"type": "snapshot-quote", "source": entry["children"][1]["source"], "text": "unverified"}
        ]}, False)

    def test_checked_entry_anchors_and_position(self):
        request = {"type": "item_view", "item": "item-1", "slot": "body", "component": "contents", "live": True}
        self.assert_shape(CATALOGUE, "ItemViewRequest", {**request, "position": "unread"})
        self.assert_shape(CATALOGUE, "ItemViewRequest", {**request, "position": "unread", "anchor": "entry-1"}, False)
        self.assert_shape(CATALOGUE, "ItemViewRequest", {**request, "position": "unread", "cursor": "cursor-1"}, False)
        selection = {"type": "item_view_selection", "window_ref": "window-1", "first": "entry-1", "last": "entry-2"}
        self.assert_shape(CATALOGUE, "ItemViewSelectionRequest", selection)
        self.assert_shape(CATALOGUE, "ItemViewSelectionRequest", {**selection, "extra_items": ["other"]}, False)

    def test_private_state_has_no_actor_override(self):
        write = {"item": "item-1", "expected": "v1", "request_id": "req-1", "patch": {"muted": True}}
        self.assert_shape(CATALOGUE, "ViewerItemStateWrite", write)
        self.assert_shape(CATALOGUE, "ViewerItemStateWrite", {**write, "actor": "other"}, False)
        self.assert_shape(CATALOGUE, "ViewerItemStateWrite", {**write, "patch": {}}, False)

    def test_literal_options_and_form_operands(self):
        prompt = {"kind": "choice", "title": "Choose", "options": [{"value": 8, "label": "Eight hours"}]}
        self.assert_shape(schema, "ValueSource", {"prompt": prompt})
        self.assert_shape(schema, "ValueSource", {"prompt": {**prompt, "options": prompt["options"] * 13}}, False)
        field = {"id": "attachments", "label": "Files", "input": "file"}
        self.assert_shape(schema, "FormField", field)
        self.assert_shape(schema, "FormField", {**field, "initial": "unverified"}, False)

    def test_viewer_field_sources_and_names_are_checked_on_install(self):
        kind = {
            "id": "entries", "icon": "circle", "title_field": "title", "storage": {"hidden": True},
            "claimed_text": True, "schema": {"type": "object", "properties": {"title": {"type": "string"}}},
            "viewer_fields": {"unread": {"type": "Number", "source": "entry_unread"}},
        }
        manifest = {"api_version": 2, "id": "sample", "name": "Sample", "icon": "circle",
                    "version": "0.1.0", "kinds": [kind], "apps": [], "operations": [], "network": "none"}
        errors = Validation()
        normalized = parse_manifest(json.dumps(manifest), errors)
        self.assertFalse(errors, list(errors))
        projected = field_meta(normalized["kinds"]["entries"], "unread")
        self.assertEqual(projected, {"type": "Number", "source": "entry_unread", "audience": "viewer_self"})
        kind["viewer_fields"]["title"] = {"type": "Text"}
        errors = Validation()
        parse_manifest(json.dumps(manifest), errors)
        self.assertTrue(any("viewer_fields.title: collides" in message for message in errors), list(errors))

    def test_retained_page_cannot_carry_private_overlays(self):
        retained = {"state": "ready", "source_revision": "v1", "window_ref": "item-view",
                    "result_version": "v1", "view": {"content": {"type": "snapshot-text", "text": "Hello"}}}
        self.assert_shape(CATALOGUE, "ItemViewResult", retained)
        self.assert_shape(CATALOGUE, "ItemViewResult", {**retained, "reading": {"version": "private-v1"}}, False)
        self.assert_shape(CATALOGUE, "ItemViewResult", {
            **retained, "state": "live", "reading": {"version": "private-v1"}
        })
