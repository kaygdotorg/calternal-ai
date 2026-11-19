import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

test("returns the complete offline submission guide without making broker calls", async () => {
  const fixture = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]));
  try {
    const result = await fixture.invoke("calternal-plugin-submission-guide.explain", {});
    const source = readFileSync(new URL("../README.md", import.meta.url), "utf8");
    assert.equal(result.markdown, source);
    assert.match(source, /calternal plugin verify-package/);
  } finally {
    await fixture.close();
  }
});
