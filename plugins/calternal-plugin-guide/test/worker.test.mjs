import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

function exactExample(markdown, path, language) {
  const source = readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8")
    .trimEnd();
  assert.ok(markdown.includes(`\`\`\`${language}\n${source}\n\`\`\``), `missing scaffold example from ${path}`);
}

test("scaffolds, checks and explains a plugin and publishes its documentation", async () => {
  const worker = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    write_item(input) {
      assert.equal(input.kind, "plugin-guides");
      assert.match(input.text, /Write a Calternal plugin/);
      exactExample(input.text, "plugins/calternal-plugin-guide/examples/first-list.ts", "ts");
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
    assert.match((await invoke("explain", { topic: "jobs" })).markdown, /run_operation/);
    assert.match((await invoke("explain", { topic: "waits" })).markdown, /replayable job/);
    assert.match((await invoke("explain", { topic: "network" })).markdown, /content_fetch/);
    const { markdown: walkthrough } = await invoke("explain", { topic: "walkthrough" });
    exactExample(walkthrough, "plugins/calternal-plugin-guide/examples/first-list.ts", "ts");
    assert.equal((await invoke("publish", {})).item.id, "fixture-document");
  } finally {
    await worker.close();
  }
});
