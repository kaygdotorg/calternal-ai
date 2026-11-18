import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { fixtureWorker } from "../dist/testing.js";

test("reports the fixture worker's stderr and exit code", async () => {
  const fixture = fixtureWorker(spawn(process.execPath, ["-e", 'process.stderr.write("startup failed"); process.exitCode = 7']));
  try {
    await assert.rejects(fixture.invoke("demo.render", {}), /exit code 7\)\nWorker stderr:\nstartup failed/);
  } finally {
    await fixture.close();
  }
});
