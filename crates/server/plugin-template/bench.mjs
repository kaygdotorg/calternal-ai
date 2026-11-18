import { spawn } from "node:child_process";
import { performance } from "node:perf_hooks";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

const fixture = () => fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
  read_item: () => ({ text: "Fixture Markdown" }),
});
const summarize = samples => {
  samples.sort((a, b) => a - b);
  return { samples: samples.length, p50_ms: samples[Math.ceil(samples.length * 0.5) - 1],
    p95_ms: samples[Math.ceil(samples.length * 0.95) - 1] };
};
const cold = [];
for (let i = 0; i < 30; i++) {
  const start = performance.now();
  const worker = fixture();
  try {
    await worker.invoke("__PLUGIN__.hello", {});
    cold.push(performance.now() - start);
  } finally { await worker.close(); }
}
const worker = fixture();
async function measure(input) {
  const samples = [];
  for (let i = 0; i < 50; i++) await worker.invoke("__PLUGIN__.hello", input);
  for (let i = 0; i < 500; i++) {
    const start = performance.now();
    await worker.invoke("__PLUGIN__.hello", input);
    samples.push(performance.now() - start);
  }
  return summarize(samples);
}
try {
  console.log(JSON.stringify({ cold: summarize(cold), warm: await measure({}),
    three_host_calls: await measure({ reads: true }) }));
} finally { await worker.close(); }
