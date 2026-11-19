import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

test("starts a job and handles a typed answer or deadline", async () => {
  const job_id = "01900000-0000-7000-8000-000000000008";
  const fixture = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    start_job(request) {
      assert.deepEqual(request, { operation: "__PLUGIN__.run", input: {} });
      return { job_id };
    },
  });
  try {
    assert.deepEqual(await fixture.invoke("__PLUGIN__.start", {}), { job_id });
    const run = { job_id, input: {}, checkpoint: null, children: [] };
    const parked = await fixture.invoke("__PLUGIN__.run", run);
    assert.equal(parked.status, "parked");
    assert.equal(parked.wait.source.kind, "question");
    const resumed = { ...run, checkpoint: parked.wait.checkpoint };
    assert.deepEqual(
      await fixture.invoke("__PLUGIN__.run", { ...resumed, signal: { kind: "answer", value: { text: "Blue" } } }),
      { status: "done", result: { answer: "Blue" } },
    );
    assert.deepEqual(
      await fixture.invoke("__PLUGIN__.run", { ...resumed, signal: { kind: "deadline" } }),
      { status: "done", result: { timed_out: true } },
    );
  } finally {
    await fixture.close();
  }
});
