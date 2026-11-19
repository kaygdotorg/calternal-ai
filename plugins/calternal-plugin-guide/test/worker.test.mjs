import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

function exactExample(markdown, path, language, name) {
  const source = readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8")
    .trimEnd().replaceAll("__PLUGIN__", name);
  assert.ok(markdown.includes(`\`\`\`${language}\n${source}\n\`\`\``), `missing scaffold example from ${path}`);
}

test("scaffolds, checks and explains a plugin and publishes its documentation", async () => {
  const worker = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    write_item(input) {
      assert.equal(input.kind, "plugin-guides");
      assert.match(input.text, /Write a Calternal plugin/);
      exactExample(input.text, "crates/server/plugin-template/worker.ts", "ts", "my-plugin");
      exactExample(input.text, "packages/plugin-sdk/templates/question.worker.txt", "ts", "question-job");
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
      exactExample(markdown, "packages/plugin-sdk/templates/question.manifest.json", "json", "question-job");
      exactExample(markdown, "packages/plugin-sdk/templates/question.worker.txt", "ts", "question-job");
      exactExample(markdown, "packages/plugin-sdk/templates/question.form.json", "json", "question-job");
      exactExample(markdown, "crates/server/plugin-template/question.test.mjs", "js", "question-job");
    }
    const { markdown: network } = await invoke("explain", { topic: "network" });
    for (const [kind, example] of [
      ["A", "a portfolio balance reader"], ["B", "a stored mail message"],
      ["C", "an RSS reader with user-added feeds"], ["D", "a read-only home automation dashboard"],
    ]) {
      assert.ok(network.includes(`### ${kind}.`));
      assert.ok(network.includes(example));
    }
    assert.ok(network.includes('"profile": "images_fonts"'));
    const { markdown: walkthrough } = await invoke("explain", { topic: "walkthrough" });
    exactExample(walkthrough, "packages/plugin-sdk/templates/manifest.json", "json", "my-plugin");
    exactExample(walkthrough, "crates/server/plugin-template/worker.ts", "ts", "my-plugin");
    exactExample(walkthrough, "crates/server/plugin-template/worker.test.mjs", "js", "my-plugin");
    assert.equal((await invoke("publish", {})).item.id, "fixture-document");
  } finally {
    await worker.close();
  }
});
