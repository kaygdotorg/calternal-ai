"""Regression tests for the pinned verifier and its package input boundaries."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from textwrap import dedent
import unittest

from description_events import EventBindings, check_event_binding, walk
from description_projection_cases import ProjectionCases
from description_scope_cases import ScopeCases
from description_input import MAX_ARTIFACT_BYTES, PackageInput, Validation, VisitLimit, check_unique_ids, load_strict
from description_verification import CATALOGUE, check, check_window_rows, compile_embedded, load_reply_origins, manifest_schema, parse_manifest, schema

CONTRACT = Path(__file__).resolve().parent
FIXTURES = CONTRACT / "fixtures/description-v2"


def write_json(path, value):
    path.write_text(json.dumps(value), encoding="utf-8")


def invoke(action, **interaction):
    return {"type": "invoke", "action_id": action, "activation_id": "case",
            "activation_context": "opaque", "interaction": interaction}


def verifier_cli():
    spec = importlib.util.spec_from_file_location("verifier_cli", CONTRACT / "verify-description.py")
    verifier = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(verifier)
    return verifier


class PackageTests(ProjectionCases, ScopeCases, unittest.TestCase):
    fixtures = FIXTURES

    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.directory = Path(temp.name)
        self.package = self.directory / "package"
        self.package.mkdir()
        self.manifest = self.package / "manifest.json"
        self.event_file = self.package / "event.json"
        self.stage("notes-app")

    def stage(self, name):
        self.template = name + ".json"
        self.installed = []
        if name == "threads-nested-catalogue-app":
            owner = self.package / "installed-conversation-owner.json"
            shutil.copyfile(FIXTURES / "manifests" / "rev14-threads-owner.json", owner)
            self.installed = ["--installed-manifest", owner]
        shutil.copyfile(FIXTURES / self.template, self.package / self.template)
        shutil.copyfile(FIXTURES / "manifests" / self.template, self.manifest)

    def replace_template(self, doc):
        self.replace_template_bytes(json.dumps(doc).encode("utf-8"))

    def replace_template_bytes(self, data):
        path = self.package / self.template
        path.write_bytes(data)
        manifest = json.loads(self.manifest.read_bytes())
        manifest["apps"][0]["description"]["template"]["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        write_json(self.manifest, manifest)

    def cli(self, *args, env=None):
        return subprocess.run([sys.executable, CONTRACT / "verify-description.py", *args],
                              env=env, capture_output=True, text=True, timeout=15)

    def verify(self, *args, failure=None, env=None):
        result = self.cli("--package", self.package, *self.installed, *args, env=env)
        if failure is None:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertNotIn("Traceback", result.stderr)
            self.assertIn(failure, result.stdout + result.stderr)

    def event(self, value, failure=None, origin=None):
        write_json(self.event_file, value)
        self.verify("--event", origin or self.template, self.event_file, failure=failure)

    def test_invoke_checks_action_and_contextual_input_ids(self):
        self.event(invoke("bogus"), "unknown action")
        self.event(invoke("rename", prompt={"title": "New title"}))
        self.event(invoke("rename", prompt={"unlisted": "New title"}), "unknown prompt slot")
        self.event(invoke("rename", choices={"title": "opaque"}), "unknown choice slot")
        self.event(invoke("create", rows=["row-1"]), "row selection")
        self.stage("reading-list-app")
        self.event(invoke("add", form={"title": "A book", "url": "https://example.org"}))
        self.event(invoke("add", form={"unlisted": "text"}), "unknown form field")
        self.event(invoke("create-blank", form={"title": "text"}), "unknown form field")

    def test_window_checks_bind_and_parent_scope(self):
        self.event({"type": "window", "request": {"bind": "notes", "from": 0, "to": 10,
                                                   "route": {"collection": "opaque"}}})
        self.event({"type": "window", "request": {"bind": "bogus", "from": 0, "to": 10}}, "active description")
        self.event({"type": "window", "request": {"bind": "notes", "parent": "opaque", "from": 0, "to": 10}}, "parent scope")
        self.event({"type": "window", "request": {"bind": "notes", "from": 0, "to": 201}}, "max_request_span")
        self.stage("threads-nested-catalogue-app")
        scoped = json.loads((FIXTURES / "event-catalogue-window-parent.json").read_bytes())
        self.event(scoped)
        del scoped["request"]["parent"]
        self.event(scoped, "parent scope")

    def test_selection_checks_bind_and_row_scope(self):
        self.event({"type": "selection", "bind": "notes", "row": "opaque", "route": {"collection": "opaque"}})
        self.event({"type": "selection", "bind": "bogus", "row": "opaque"}, "active description")
        self.event({"type": "selection_set", "bind": "bogus", "rows": []}, "active description")
        self.event({"type": "selection_set", "bind": "notes", "rows": []}, "multiple-selection Bind")
        self.stage("notes-bulk-and-snooze-app")
        self.event({"type": "selection_set", "bind": "notes", "rows": ["opaque"]})
        self.event(invoke("trash-selected", rows=["opaque"]))

    def test_search_checks_bind_type(self):
        self.event({"type": "search", "bind": "bogus", "text": "words", "settle_generation": 1}, "active description")
        self.event({"type": "search", "bind": "notes", "text": "words", "settle_generation": 1}, "not a search bind")

    def test_installed_provider_id_is_unique_across_packages(self):
        self.stage("independent-runner-app")
        manifest = json.loads(self.manifest.read_bytes())
        provider = manifest["providers"][0]
        other = {
            "api_version": 2, "id": "another-runner", "version": "1.0.0",
            "name": "Another runner", "icon": "puzzle", "network": "none",
            "apps": [], "kinds": [], "operations": [], "providers": [provider],
        }
        installed = self.package / "installed-provider.json"
        self.installed = ["--installed-manifest", installed]
        write_json(installed, other)
        self.verify(failure=f"Duplicate installed provider ID {provider['id']!r}")
        provider["id"] = "distinct-provider"
        write_json(installed, other)
        self.verify()

    def test_standalone_card_requires_its_editor_route_on_open(self):
        self.replace_template({
            "version": 2, "kind": "app", "revision": 1, "binds": [],
            "view": {"root": {"type": "component", "component": {
                "type": "item-editor", "id": "editor", "config": {"item": {"route": "item"}}
            }}}
        })
        event = {"type": "view_open", "view_ref": "opaque"}
        self.event(event | {"route": {"item": "note-1"}})
        self.event(event, "missing required route operand 'item'")

    def test_selection_set_checks_the_same_bind_route_as_windows(self):
        self.stage("notes-bulk-and-snooze-app")
        app = json.loads((self.package / self.template).read_bytes())
        bind = next(bind for bind in app["binds"] if bind["id"] == "notes")
        bind["source"]["collection"] = {"route": "collection"}
        self.replace_template(app)
        event = {"type": "selection_set", "bind": "notes", "rows": ["opaque"]}
        self.event(event | {"route": {"collection": "opaque"}})
        self.event(event, "missing required route operand 'collection'")

    def test_route_checks_name(self):
        self.event({"type": "route", "id": "item", "row": "opaque"})
        self.event({"type": "route", "id": "bogus", "row": "opaque"}, "unknown route")

    def test_route_checks_selector_options(self):
        self.stage("threads-app")
        self.event({"type": "route", "id": "view", "row": "inbox"})
        self.event({"type": "route", "id": "view", "row": "bogus"}, "no matching row scope")

    def test_shortcut_route_accepts_its_literal_and_rejects_other_values(self):
        path = self.package / self.template
        doc = json.loads(path.read_bytes())
        doc["chrome"]["sidebar"]["sections"].append({
            "id": "more", "rows": [{"id": "trash-shortcut", "title": "Trash",
                                    "open": {"route": {"name": "destination", "value": {"literal": "trash"}}}}]
        })
        self.replace_template(doc)
        self.event({"type": "route", "id": "destination", "row": "trash"})
        self.event({"type": "route", "id": "destination", "row": "other"}, "no matching row scope")

    def test_oversize_bind_work_is_refused(self):
        self.replace_template({"version": 2, "kind": "app", "revision": 1, "binds": [None] * 300_000})
        self.verify(failure="Validation exceeds 200000 visits")

    def test_byte_limit_null_array_decodes_with_bounded_resources_then_hits_visit_budget(self):
        max_seconds = 5
        max_mib = 128
        array = b"[" + b"null," * (MAX_ARTIFACT_BYTES // 5 - 1) + b"null]"
        data = array.ljust(MAX_ARTIFACT_BYTES, b" ")
        self.assertEqual(len(data), MAX_ARTIFACT_BYTES)
        self.replace_template_bytes(data)

        # An isolated process keeps its peak RSS independent of this test runner.
        probe = dedent("""
            import json, resource, runpy, sys, time
            sys.argv = sys.argv[1:]
            start = time.monotonic()
            try:
                runpy.run_path(sys.argv[0], run_name="__main__")
            finally:
                rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
                rss_mib = rss / (1024 ** (2 if sys.platform == "darwin" else 1))
                print("decode-probe", json.dumps([time.monotonic() - start, rss_mib]))
        """)
        result = subprocess.run(
            [sys.executable, "-c", probe, CONTRACT / "verify-description.py", "--package", self.package],
            cwd=CONTRACT, text=True, capture_output=True, timeout=max_seconds,
        )
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("Validation exceeds 200000 visits", result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        sample = next(line.removeprefix("decode-probe ") for line in result.stdout.splitlines()
                      if line.startswith("decode-probe "))
        elapsed, peak_rss = json.loads(sample)
        self.assertLess(elapsed, max_seconds)
        self.assertLess(peak_rss, max_mib)
        print(f"4 MiB null-array refusal: {elapsed:.3f} s, {peak_rss:.1f} MiB peak RSS")

    def test_section_checks_sidebar_declaration(self):
        self.event({"type": "section", "id": "bogus", "collapsed": True}, "unknown section")

    def test_sidebar_checks_surface(self):
        self.event({"type": "sidebar", "open": True})
        doc = json.loads((self.package / self.template).read_bytes())
        del doc["chrome"]["sidebar"]
        self.replace_template(doc)
        self.event({"type": "sidebar", "open": True}, "no sidebar")

    def test_layout_checks_surface(self):
        self.event({"type": "layout", "id": "sidebar", "width": 320})
        self.event({"type": "layout", "id": "bogus", "width": 320}, "unknown layout")

    def test_app_checks_footer(self):
        self.event({"type": "app", "id": "threads"})
        self.event({"type": "app", "id": "bogus"}, "unknown app")

    def test_refresh_checks_optional_bind(self):
        self.event({"type": "refresh"})
        self.event({"type": "refresh", "bind": "bogus"}, "active description")

    def test_component_checks_route_and_request_shape(self):
        self.stage("threads-app")
        event = json.loads((FIXTURES / "event-catalogue-transcript-page.json").read_bytes())
        self.event(event)
        self.event({**event, "component": "bogus"}, "unknown component")
        self.event({**event, "component": "composer"}, "cannot use route")
        self.event({**event, "request": {}}, "missing conversation")

    def test_composer_options_requires_declared_control(self):
        self.stage("threads-app")
        path = self.package / self.template
        doc = json.loads(path.read_bytes())
        next(component for component in doc["chrome"]["panel"]["components"]
             if component["type"] == "composer")["config"]["controls"] = ["model"]
        self.replace_template(doc)
        event = {"type": "component", "component": "panel-composer", "route": "composer.options",
                 "request": {"conversation": "opaque", "control": "model"}}
        self.event(event)
        event["request"]["control"] = "machine"
        self.event(event, "control 'machine' is not configured")

    def test_empty_shortcut_route_fails_structure_before_event_indexing(self):
        path = self.package / self.template
        doc = json.loads(path.read_bytes())
        doc["chrome"]["sidebar"]["sections"].append({
            "id": "short", "rows": [{"id": "open", "title": "Open", "open": {"route": {}}}]
        })
        self.replace_template(doc)
        self.event({"type": "refresh"}, "open.route: missing name")

    def test_package_ownership_diagnostics_are_capped_at_32(self):
        manifest = json.loads(self.manifest.read_bytes())
        kind = manifest["kinds"][0]
        manifest["kinds"] = [{**kind, "id": f"kind-{index}"} for index in range(64)]
        write_json(self.manifest, manifest)
        installed = self.package / "installed.json"
        write_json(installed, {**manifest, "id": "other"})
        run = self.cli("--package", self.package, "--installed-manifest", installed)
        self.assertEqual(run.returncode, 1, run.stdout + run.stderr)
        self.assertEqual(run.stdout.count("Duplicate installed kind ID"), 32, run.stdout)

    def test_structural_fixture_corpus_fails_cleanly_in_package_mode(self):
        origins = self.package / "origins.json"
        write_json(origins, {self.template: {"notes": [self.event_file.name]}})
        failures = 0
        for fixture in sorted(FIXTURES.rglob("invalid-*.json")):
            name = fixture.name
            if fixture.is_relative_to(FIXTURES / "provider-marks"):
                continue  # Paper cases are not wire artifacts.
            data = fixture.read_bytes()
            errors = Validation()
            kind = ("manifest" if fixture.is_relative_to(FIXTURES / "manifests")
                    else "event" if name.startswith("invalid-event-")
                    else "reply" if name.startswith("invalid-reply-")
                    else "description")
            shapes = {"manifest": manifest_schema, "event": {"$ref": "#/$defs/Event"},
                      "reply": {"$ref": "#/$defs/WindowReply"}, "description": schema}
            try:
                doc = load_strict(data.decode("utf-8"))
                check(shapes[kind], doc, "$", errors, manifest_schema if kind == "manifest" else None)
            except ValueError as error:
                errors.append(str(error))
            if not errors:
                continue
            failures += 1
            with self.subTest(fixture=str(fixture.relative_to(FIXTURES))):
                self.stage("notes-app")
                if kind == "manifest":
                    self.manifest.write_bytes(data)
                    self.verify(failure="FAIL")
                elif kind == "event":
                    self.event_file.write_bytes(data)
                    self.verify("--event", self.template, self.event_file, failure="FAIL")
                elif kind == "reply":
                    self.event_file.write_bytes(data)
                    self.verify("--reply", self.event_file, "--origins", origins, failure="FAIL")
                else:
                    self.replace_template_bytes(data)
                    self.verify("--event", self.template, self.event_file, failure="FAIL")
        self.assertGreater(failures, 0)
        print(f"Package structural rejection corpus: {failures} fixtures")

    def test_choice_options_checks_slot(self):
        event = {"type": "choice_options", "activation_context": "opaque", "slot": "collection"}
        self.event(event)
        self.event({**event, "slot": "bogus"}, "unknown choice slot")

    def test_events_with_only_host_refs_validate_wire_shape(self):
        # These events have no template IDs. Use one valid case and corrupt its required field.
        for suffix, field in (("confirm", "resolution_ref"), ("dispatch", "resolution_ref"),
                              ("lease-renew", "lease_ref"), ("view-open", "view_ref"),
                              ("undo", "undo_ref"), ("status", "status_ref")):
            with self.subTest(event=suffix):
                event = json.loads((FIXTURES / f"event-catalogue-{suffix}.json").read_bytes())
                self.event(event)
                del event[field]
                self.event(event, f"missing {field}")

    def test_origin_must_name_verified_template(self):
        self.event({"type": "refresh"}, "unknown event origin", origin="other.json")

    def test_package_and_normal_fixture_run_ignore_override(self):
        env = {**os.environ, "CALTERNAL_HOST_CATALOGUE": str(self.directory / "missing.json")}
        self.verify(env=env)
        normal = self.cli(env=env)
        self.assertEqual(normal.returncode, 0, normal.stdout + normal.stderr)
        self.assertIn("host catalogue: loaded", normal.stdout)
        testing = self.cli("--fixture-testing", env=env)
        self.assertEqual(testing.returncode, 0, testing.stdout + testing.stderr)
        self.assertIn("host catalogue: absent", testing.stdout)

    def test_fixture_testing_cannot_override_package_mode(self):
        result = self.cli("--package", self.package, "--fixture-testing")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn("not allowed with argument --package", result.stderr)

    def test_catalogue_eligibility_rejects_unavailable_swipe(self):
        self.stage("threads-app")
        fixture = FIXTURES / "invalid-catalogue-archive-fullswipe-before-writer.json"
        self.replace_template(json.loads(fixture.read_bytes()))
        self.verify(failure="fullSwipe is rejected on host.item.archive")

    def test_structure_and_tree_wide_action_ids(self):
        fixture = FIXTURES / "invalid-duplicate-cross-list-action-id.json"
        self.replace_template(json.loads(fixture.read_bytes()))
        self.verify(failure="duplicate action id")
        self.replace_template({"version": 2, "kind": "app", "revision": "first"})
        self.verify(failure="$.revision: expected integer")

    def test_scope_rejects_undeclared_reads(self):
        manifest = json.loads(self.manifest.read_bytes())
        manifest["reads"].remove("notes")
        write_json(self.manifest, manifest)
        self.verify(failure="kind 'notes' is not in the manifest's")

    def test_template_digest_and_path_bound(self):
        with (self.package / self.template).open("ab") as stream:
            stream.write(b" ")
        self.verify(failure="SHA-256 mismatch")
        self.verify("--origins", "x" * 4097, failure="Artifact path exceeds")

    def test_argument_count_bound(self):
        self.verify(*(["--event", self.template, self.event_file] * 65), failure="Package argument caps")

    def test_installed_ownership_is_unique(self):
        other = self.package / "installed.json"
        shutil.copyfile(self.manifest, other)
        self.verify("--installed-manifest", other, failure="Duplicate installed package ID")
        doc = json.loads(other.read_bytes())
        doc["id"] = "other"
        write_json(other, doc)
        self.verify("--installed-manifest", other, failure="Duplicate installed kind ID")

    def test_reply_origin_and_projection_type(self):
        replies = self.package / "replies"
        replies.mkdir()
        doc = json.loads((FIXTURES / "reply-catalogue-window-ready.json").read_bytes())
        reply = replies / "window.json"
        write_json(reply, doc)
        self.verify(failure="requires --origins")
        origins = replies / "origins.json"
        write_json(origins, {self.template: {"notes": [reply.name]}})
        self.verify()
        doc["rows"][0]["fields"]["title"] = 42
        write_json(reply, doc)
        self.verify(failure="fields.title: expected string")
        write_json(origins, {self.template: {"bogus": [reply.name]}})
        self.verify(failure="unknown reply origin Bind")

    def test_artifact_symlink_is_refused(self):
        outside = self.directory / "manifest.json"
        self.manifest.rename(outside)
        self.manifest.symlink_to(outside)
        self.verify(failure="symlink")
        self.manifest.unlink()
        outside.rename(self.manifest)
        template = self.package / self.template
        inner = self.package / "inner.json"
        template.rename(inner)
        template.symlink_to(inner)
        self.verify(failure="symlink")

    def test_root_descriptor_survives_ancestor_replacement(self):
        parent = self.directory / "parent"
        parent.mkdir()
        root = parent / "pkg"
        root.mkdir()
        (root / "artifact").write_bytes(b"original")
        outside = self.directory / "outside"
        (outside / "pkg").mkdir(parents=True)
        (outside / "pkg" / "artifact").write_bytes(b"untrusted")
        with PackageInput(root) as inputs:
            parent.rename(self.directory / "moved")
            parent.symlink_to(outside)
            self.assertEqual(inputs.bytes(root / "artifact")[1], b"original")
        with self.assertRaises((OSError, ValueError)):
            with PackageInput(root):
                self.fail("opened through an ancestor symlink")


class InputTests(unittest.TestCase):
    def test_component_request_shape_precedes_indexing_for_every_route(self):
        verifier = verifier_cli()
        for route in schema["$defs"]["EventComponent"]["properties"]["route"]["enum"]:
            with self.subTest(route=route):
                errors = Validation()
                event = {"type": "component", "component": "unused", "route": route, "request": {}}
                verifier.validate_event(event, lambda: self.fail("indexed before request validation"), errors)
                self.assertTrue(any(message.startswith("$.request: missing ") for message in errors), list(errors))
        self.assertTrue(CATALOGUE["component_routes"])

    def test_budgeted_iteration_stops_without_pulling_after_full(self):
        errors = Validation()
        pulls = []

        def elements():
            for index in range(100):
                pulls.append(index)
                yield index

        for _ in errors.each(elements()):
            errors.append("one invalid element")
        self.assertEqual((len(pulls), errors.visits, len(errors)), (32, 32, 32))

    def test_schema_array_and_object_loops_stop_after_diagnostics_fill(self):
        for shape, make_data in (
            ({"items": False}, lambda length: [None] * length),
            ({"prefixItems": [False] * 100}, lambda length: [None] * length),
            ({"additionalProperties": False}, lambda length: {f"extra-{i}": None for i in range(length)}),
            ({"required": [f"missing-{i}" for i in range(100)],
              "additionalProperties": False}, lambda length: {f"extra-{i}": None for i in range(length)}),
        ):
            with self.subTest(shape=next(iter(shape))):
                short = Validation()
                check(shape, make_data(32), "$", short)
                for length in (32, 100, 1000):
                    extended = Validation(visit_budget=short.visits + 2000)
                    check(shape, make_data(length), "$", extended)
                    self.assertEqual(len(extended), 32)
                    self.assertEqual(extended.visits, short.visits)
                with self.assertRaises(VisitLimit):
                    check(shape, make_data(100), "$", Validation(visit_budget=50))

    def test_unique_and_contains_loops_charge_each_element(self):
        for shape in ({"uniqueItems": True}, {"contains": {"const": True}}):
            with self.subTest(shape=shape):
                with self.assertRaises(VisitLimit):
                    check(shape, [None] * 100, "$", Validation(visit_budget=150))

    def test_tree_and_duplicate_id_walks_stop_and_charge(self):
        for length in (33, 100):
            errors = Validation()
            check_unique_ids([{"id": "duplicate"}] * length, errors)
            self.assertEqual((len(errors), errors.visits), (32, 130))
            values = Validation()
            for _, node in walk([None] * length, values):
                if node is None:
                    values.append("bad node")
            self.assertEqual((len(values), values.visits), (32, 65))
        with self.assertRaises(VisitLimit):
            check_unique_ids([{"id": str(i)} for i in range(50)], Validation(visit_budget=100))
        with self.assertRaises(VisitLimit):
            list(walk([None] * 50, Validation(visit_budget=100)))

    def test_ownership_loop_stops_after_32_collisions(self):
        verifier = verifier_cli()
        owners = [{"id": "first", "kinds": {f"kind-{i}": {} for i in range(64)}},
                  {"id": "second", "kinds": {f"kind-{i}": {} for i in range(64)}}]
        errors = Validation()
        verifier.check_package_owners(owners, errors)
        self.assertEqual((len(errors), errors.visits), (32, 98))
        with self.assertRaises(VisitLimit):
            verifier.check_package_owners(owners, Validation(visit_budget=90))

    def test_invoke_and_reply_semantic_loops_stop_after_full(self):
        view = {"view": {"root": {"type": "button", "invoke": {"id": "apply", "operation": "a.run"}}}}
        bindings = EventBindings(view, Validation())
        for extra in (0, 8):
            errors = Validation()
            event = invoke("apply", form={f"field-{i}": "invalid" for i in range(32)},
                           choices={f"choice-{i}": "invalid" for i in range(extra)})
            check_event_binding(event, bindings, errors, CATALOGUE)
            self.assertEqual((len(errors), errors.visits), (32, 34))
        origin = ({"id": "all", "fields": ["title"]}, {}, CATALOGUE["generic_item_fields"])
        small = Validation()
        rows = [{"fields": {"title": False}}] * 32
        check_window_rows({"rows": rows}, origin, CATALOGUE, small)
        large = Validation()
        check_window_rows({"rows": rows * 6}, origin, CATALOGUE, large)
        self.assertEqual((len(large), large.visits), (32, small.visits))

    def test_origin_and_json_collection_loops_spend_visits(self):
        origins = {"app.json": {"all": [f"reply-{i}.json" for i in range(64)]}}
        files = set(origins["app.json"]["all"])
        with self.assertRaises(VisitLimit):
            load_reply_origins(origins, CONTRACT, {}, CATALOGUE, files, Validation(visit_budget=32))
        with self.assertRaises(VisitLimit):
            load_strict(json.dumps([{"key": i} for i in range(64)]), Validation(visit_budget=32))

    def test_encoding_depth_keys_and_numbers(self):
        for source in ('"\\ud800"', '[' * 33 + ']' * 33, '{"a":1,"a":2}',
                       '{"' + 'a' * 257 + '":1}', '9007199254740992', '1e500', 'NaN'):
            with self.subTest(source=source[:30]), self.assertRaises(ValueError):
                load_strict(source)

    def test_sizes_and_directory_entries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifact = root / "data"
            with artifact.open("wb") as stream:
                stream.truncate(4 * 1024 * 1024 + 1)
            with PackageInput(root) as inputs, self.assertRaisesRegex(ValueError, "byte budget"):
                inputs.bytes(artifact)
            with artifact.open("wb") as stream:
                stream.truncate(4 * 1024 * 1024)
            with PackageInput(root) as inputs:
                for _ in range(8):
                    inputs.bytes(artifact)
                with self.assertRaisesRegex(ValueError, "byte budget"):
                    inputs.bytes(artifact)
            for i in range(129):
                (root / str(i)).touch()
            with PackageInput(root) as inputs, self.assertRaisesRegex(ValueError, "128 entries"):
                inputs.reply_files(root)

    def test_schema_uri_length_and_offline_reference(self):
        base = json.loads((FIXTURES / "manifests/install/host-create-after-item-ref-annotations.json").read_bytes())
        for ref in ("https://unregistered.invalid/type", "#" + "a" * 2048):
            with self.subTest(ref=ref[:80]):
                base["operations"][0]["output_schema"] = {"$ref": ref}
                errors = Validation()
                self.assertIsNone(parse_manifest(json.dumps(base), errors))
                self.assertIn("unresolvable embedded schema reference", errors.messages[0])

    def test_manifest_diagnostics_are_capped_across_embedded_schemas(self):
        manifest = json.loads((FIXTURES / "manifests/install/host-create-after-item-ref-annotations.json").read_bytes())
        for key in ("input_schema", "output_schema"):
            manifest["operations"][0][key] = {"allOf": [{"$ref": f"#/bad{i}"} for i in range(25)]}
        errors = Validation()
        self.assertIsNone(parse_manifest(json.dumps(manifest), errors))
        self.assertEqual(len(errors), 32)

    def test_reference_resolution_spends_shared_visit_budget(self):
        # A schema annotation and a reference have the same structural cost here.
        # Fit the walk in the budget, leaving one fewer visit than the ref count.
        count = 100
        baseline = Validation()
        compile_embedded({"allOf": [{"description": "#"}] * count}, "$", baseline)
        self.assertFalse(baseline.messages)
        limited = Validation(visit_budget=baseline.visits + count - 1)
        with self.assertRaisesRegex(ValueError, "Validation exceeds .* visits"):
            compile_embedded({"allOf": [{"$ref": "#"}] * count}, "$", limited)
        self.assertFalse(limited.messages)

    def test_schema_with_more_references_than_default_budget(self):
        errors = Validation()
        document = {"allOf": [{"$ref": "#"}] * (errors.visit_budget + 1)}
        with self.assertRaisesRegex(ValueError, "Validation exceeds 200000 visits"):
            compile_embedded(document, "$", errors)

    def test_schema_pointer_targets_are_positions(self):
        base = json.loads((FIXTURES / "manifests/install/host-create-after-item-ref-annotations.json").read_bytes())
        doc = base["operations"][0]
        for node in (True, {"type": "string"}):
            for annotation in ("default", "examples"):
                with self.subTest(node=node, annotation=annotation):
                    doc["output_schema"] = ({annotation: node, "$ref": "#/default"} if annotation == "default"
                                            else {annotation: [node], "$ref": "#/examples/0"})
                    errors = Validation()
                    self.assertIsNone(parse_manifest(json.dumps(base), errors))
                    self.assertIn("unresolvable embedded schema reference", errors.messages[0])
            doc["output_schema"] = {"definitions": {"value": node}, "$ref": "#/definitions/value"}
            errors = Validation()
            self.assertIsNotNone(parse_manifest(json.dumps(base), errors), errors)
        doc["output_schema"] = {"$ref": "host-catalogue-v2.json#/operations/host.item.trash/atomic_undo"}
        errors = Validation()
        self.assertIsNone(parse_manifest(json.dumps(base), errors))
        self.assertIn("unresolvable embedded schema reference", errors.messages[0])


if __name__ == "__main__":
    unittest.main()
