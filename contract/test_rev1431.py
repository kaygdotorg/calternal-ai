"""Regression checks for package validation and the revision 14.31 worker context."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from description_input import Validation
from description_verification import CATALOGUE, check, check_worker_invocation, parse_manifest, schema


def package(name):
    return {
        "api_version": 2, "id": name, "name": name, "version": "0.1.0",
        "icon": "circle", "apps": [], "kinds": [], "operations": [], "network": "none",
    }


def operation(name):
    return {
        "id": name, "title": "Run", "required_capability": "plugins.use",
        "risk": "external", "idempotency": "request_id", "input_schema": {}, "output_schema": {},
        "methods": [], "credential_routes": [],
    }


class Revision1431(unittest.TestCase):
    def test_execution_job_invocation_uses_current_declaration(self):
        manifest = package("sample")
        manifest["kinds"] = [{
            "id": "sample-item", "icon": "circle", "title_field": "title",
            "storage": {"folder": "Sample"}, "claimed_text": True, "execution_target": True,
            "schema": {"type": "object", "properties": {"title": {"type": "string"}}},
        }]
        manifest["worker"] = {"path":"worker.mjs","sha256":"a" * 64}
        manifest["operations"] = [{**operation("sample.run"), "target": {"kind": "sample-item", "field": "item"}}]
        manifest["jobs"] = [{"id": "run", "run_operation": "sample.run", "execution": True, "alarm": True, "replayable": True}]
        errors = Validation()
        checked = parse_manifest(json.dumps(manifest), errors)
        self.assertFalse(errors, list(errors))
        context = {"intent": "job-1", "item": "item-1", "harness": "harness-1"}
        frame = {
            "v": 1, "epoch": "epoch-1", "seq": 1, "package": "sample", "digest": "a" * 64, "ticket": "ticket-1",
            "message": {
                "type": "invoke", "operation": "sample.run", "input": {"item":"item-1"}, "deadline": "2026-11-20T00:01:00Z",
                "author_context": {"request_id":"admission-1", "actor":"actor-1", "authored_at":"2026-11-20T00:00:00Z"},
                "job_context": context,
                "alarm": {"alarm":"alarm-1","scheduled_at":"2026-11-19T23:59:00Z","actual_at":"2026-11-20T00:00:00Z","missed":True},
            },
        }
        check(CATALOGUE["$defs"]["HostWorkerInvokeFrame"], frame, "$", errors, CATALOGUE)
        check_worker_invocation(frame, checked, "run", errors)
        self.assertFalse(errors, list(errors))
        del frame["message"]["job_context"]
        check_worker_invocation(frame, checked, "run", errors)
        self.assertTrue(any("required for the admitted execution job" in error for error in errors), list(errors))

    def test_package_mode_qualifies_contribution_and_checks_entry_digest(self):
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp)
            owner, contributor = parent / "owner", parent / "contributor"
            owner.mkdir()
            contributor.mkdir()
            owner_manifest = package("sample")
            template = b'{"version":2,"kind":"app","revision":1}'
            (owner / "app.json").write_bytes(template)
            owner_manifest["apps"] = [{
                "id":"sample-app","name":"Sample","icon":"circle",
                "description":{"id":"app","template":{"path":"app.json","sha256":hashlib.sha256(template).hexdigest()}},
                "extension_points":[{"id":"extensions","kind":"harness","version":1,"input_schema":{},"output_schema":{}}],
            }]
            (owner / "manifest.json").write_text(json.dumps(owner_manifest))
            (contributor / "installed-owner.json").write_text(json.dumps(owner_manifest))
            contributor_manifest = package("sample-harness")
            worker = b"export default {}"
            (contributor / "worker.mjs").write_bytes(worker)
            contributor_manifest["worker"] = {"path":"worker.mjs","sha256":hashlib.sha256(worker).hexdigest()}
            contributor_manifest["operations"] = [operation("sample-harness.run")]
            entry = b"disposable executable specimen"
            (contributor / "entry.bin").write_bytes(entry)
            contributor_manifest["contributions"] = [{
                "app":{"package":"sample","app":"sample-app"},"point":"extensions","version":1,
                "id":"worker","kind":"harness","operation":"sample-harness.run",
                "harness":{"image":"sha256:" + "a" * 64,"entry":{"path":"entry.bin","sha256":hashlib.sha256(entry).hexdigest()},
                           "name":"Harness"},
            }]
            (contributor / "manifest.json").write_text(json.dumps(contributor_manifest))
            tool = Path(__file__).with_name("verify-description.py")

            def verify(path, *args):
                return subprocess.run([sys.executable, "-B", str(tool), "--package", str(path), *args],
                                      text=True, capture_output=True, check=False)

            result = verify(owner)
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            result = verify(contributor, "--installed-manifest", str(contributor / "installed-owner.json"))
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            (contributor / "entry.bin").write_bytes(b"mismatch")
            result = verify(contributor)
            self.assertNotEqual(0, result.returncode)
            self.assertIn("harness entry SHA-256 mismatch", result.stdout + result.stderr)

    def test_snapshot_text_counts_unicode_code_points(self):
        leaf = schema["$defs"]["SnapshotText"]
        for length in (2000, 2001):
            errors = Validation()
            check(leaf, {"type":"snapshot-text", "text":"界" * length}, "$", errors, schema)
            self.assertEqual(length > 2000, bool(errors), list(errors))


if __name__ == "__main__":
    unittest.main()
