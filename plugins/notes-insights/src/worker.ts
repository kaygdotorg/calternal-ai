/** Long-lived notes statistics worker. All item reads go through the authorized host broker.
 * @module
 */
import type { GenUiView, ManifestV1, Stats } from "@calternal/contracts";
import { WorkerRuntimeV2, parseWorkerFrameV2, encodeWorkerFrameV2, MAX_V2_FRAME_BYTES } from "@calternal/plugin-sdk";
import manifest from "../manifest.json" with { type: "json" };

function summarize(markdown: string): Stats {
  const words = markdown.match(/[\p{L}\p{N}]+(?:['’][\p{L}\p{N}]+)*/gu) ?? [];
  const breaks = markdown.match(/\r\n|\r|\n/gu) ?? [];
  const endsInBreak = /(?:\r\n|\r|\n)$/u.test(markdown);
  return {
    words: words.length,
    characters: Array.from(markdown).length,
    lines: markdown.length === 0 ? 0 : breaks.length + (endsInBreak ? 0 : 1),
  };
}

/** Generated-UI statistics view: three metrics in a row, drawn by each client natively. */
function view(stats: Stats): GenUiView {
  return {
    genui_version: 2,
    source: `Row([Metric("Words", "${stats.words}"), Metric("Characters", "${stats.characters}"), Metric("Lines", "${stats.lines}")])`,
  };
}

const runtime = new WorkerRuntimeV2(manifest as ManifestV1, {
  "notes-insights.summarize": async (input, context) => {
    if (!input || typeof input !== "object" || Array.isArray(input) || Object.keys(input).length !== 1 || !("id" in input) || typeof input.id !== "string") throw new Error("invalid input");
    const item = await context.call("read_item", { kind: "notes", item_id: input.id });
    if (!item || typeof item !== "object" || !("text" in item) || typeof item.text !== "string") throw new Error("invalid item");
    const stats = summarize(item.text);
    return { stats, view: view(stats) };
  },
}, async frame => {
  const line = encodeWorkerFrameV2(frame);
  await new Promise<void>((resolve, reject) => process.stdout.write(line, error => error ? reject(error) : resolve()));
});

async function main(): Promise<void> {
  let buffered = new Uint8Array(0);
  for await (const chunk of process.stdin) {
    const bytes = typeof chunk === "string" ? new TextEncoder().encode(chunk) : chunk;
    // Consume incrementally so a multi-frame OS read cannot bypass the per-frame budget.
    let start = 0;
    for (let i = 0; i < bytes.length; i++) {
      if (bytes[i] !== 10) continue;
      if (buffered.length + i - start > MAX_V2_FRAME_BYTES) throw new Error("frame limit");
      const line = new Uint8Array(buffered.length + i - start); line.set(buffered); line.set(bytes.subarray(start, i), buffered.length);
      await runtime.receive(parseWorkerFrameV2(line));
      buffered = new Uint8Array(0); start = i + 1;
    }
    if (buffered.length + bytes.length - start > MAX_V2_FRAME_BYTES) throw new Error("frame limit");
    const tail = new Uint8Array(buffered.length + bytes.length - start); tail.set(buffered); tail.set(bytes.subarray(start), buffered.length); buffered = tail;
  }
  if (buffered.length) throw new Error("unterminated frame");
}
main().catch(() => { process.stderr.write("notes-insights worker protocol failure\n"); process.exitCode = 1; process.stdin.destroy(); });
