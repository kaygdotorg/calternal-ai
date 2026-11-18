import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

test("scaffolds, checks and explains a plugin and publishes its documentation", async () => {
  const worker = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    write_item(input) {
      assert.equal(input.kind, "plugin-guides");
      assert.match(input.text, /Write a Calternal plugin/);
      return { id: "fixture-document" };
    },
  });
  try {
    const invoke = (operation, input) => worker.invoke(`calternal-plugin-guide.${operation}`, input);
    const scaffold = await invoke("scaffold", { name: "my-plugin" });
    assert.equal(scaffold.manifest.id, "my-plugin");
    assert.equal((await invoke("check", { manifest: scaffold.manifest })).valid, true);
    assert.equal((await invoke("check", { manifest: {} })).valid, false);
    assert.match((await invoke("explain", { topic: "skill" })).markdown, /^---/);
    for (const topic of ["jobs", "waits"]) {
      const { markdown } = await invoke("explain", { topic });
      assert.match(markdown, /Form\(\[Input\(\$text/);
      assert.match(markdown, /signal\.value\.text/);
      assert.match(markdown, /deadline_ms/);
      assert.match(markdown, /### manifest\.json/);
    }
    assert.equal((await invoke("publish", {})).item.id, "fixture-document");
  } finally {
    await worker.close();
  }
});
