import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";
import { spawn } from "node:child_process";

test("returns a generated view over the worker protocol", async () => {
  const fixture = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]));
  try {
    const view = await fixture.invoke("__PLUGIN__.hello", {});
    assert.equal(view.view.genui_version, 2);
    assert.match(view.view.source, /Plugin/);
  } finally {
    await fixture.close();
  }
});
