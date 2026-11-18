import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

test("renders its shared label and refuses a setting outside its declaration", async () => {
  const worker = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    read_setting_v1({ key }) {
      if (key !== "label") throw new Error("Undeclared setting");
      return 'Write a "note"';
    },
  });
  try {
    const operation = "composer-settings.label";
    assert.deepEqual((await worker.invoke(operation, {})).view, {
      genui_version: 2, source: 'Badge("Write a \\"note\\"")',
    });
    await assert.rejects(worker.invoke(operation, { key: "other/private" }));
  } finally {
    await worker.close();
  }
});
