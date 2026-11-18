/** Measure a worker round trip with a host-owned virtual list and streamed text.
 * Uses structured clone and a headless host state model. Measures no rendering.
 */
import { once } from "node:events";
import { performance } from "node:perf_hooks";
import { Worker, isMainThread, parentPort } from "node:worker_threads";

const rowCount = 10_000;
const visible = 60;
const chunks = 1_000;

if (!isMainThread) {
  const row = index => ({ id: index, label: `Conversation ${index}` });
  parentPort.on("message", ({ kind, index }) => {
    if (kind === "snapshot") {
      parentPort.postMessage(Array.from({ length: rowCount }, (_, i) => row(i)));
    } else if (kind === "viewport") {
      parentPort.postMessage(Array.from({ length: visible }, (_, i) => row(index + i)));
    } else {
      parentPort.postMessage({ id: 1, sequence: index, text: "x".repeat(256) });
    }
  });
} else {
  const worker = new Worker(new URL(import.meta.url));
  const host = new Map();
  let stream = "";
  async function step(kind, index) {
    const start = performance.now();
    const reply = once(worker, "message");
    worker.postMessage({ kind, index });
    const [update] = await reply;
    if (kind === "stream") {
      if (update.sequence !== index || update.id !== 1) throw new Error("wrong delta");
      stream += update.text;
    } else {
      if (update.length !== (kind === "snapshot" ? rowCount : visible)) {
        throw new Error("wrong page size");
      }
      for (const row of update) host.set(row.id, row.label);
    }
    return performance.now() - start;
  }
  async function measure(kind, count) {
    const values = [];
    for (let i = 0; i < 30; i++) await step(kind, i);
    stream = "";
    for (let i = 0; i < count; i++) values.push(await step(kind, i));
    values.sort((a, b) => a - b);
    return { samples: count, p50_ms: values[Math.ceil(count * 0.5) - 1], p95_ms: values[Math.ceil(count * 0.95) - 1] };
  }
  try {
    await once(worker, "online");
    const snapshot = await measure("snapshot", 100);
    host.clear();
    const viewport = await measure("viewport", 1_000);
    const streamDelta = await measure("stream", chunks);
    if (stream.length !== chunks * 256) throw new Error("stream incomplete");
    console.log(JSON.stringify({ node: process.version, arch: process.arch, snapshot, viewport, streamDelta }, null, 2));
  } finally {
    await worker.terminate();
  }
}
