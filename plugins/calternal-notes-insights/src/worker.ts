/** Long-lived notes statistics worker. All item reads go through the authorized host broker.
 * @module
 */
import type { GenUiView, Stats } from "@calternal/contracts";
import { serveWorker } from "@calternal/plugin-sdk";
import type { WorkerHandlerV2 } from "@calternal/plugin-sdk";
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

const handlers = {
  "notes-insights.summarize": async (input, context) => {
    if (!input || typeof input !== "object" || Array.isArray(input) || Object.keys(input).length !== 1 || !("id" in input) || typeof input.id !== "string") throw new Error("invalid input");
    const item = await context.call("read_item", { kind: "notes", item_id: input.id });
    if (!item || typeof item !== "object" || !("text" in item) || typeof item.text !== "string") throw new Error("invalid item");
    const stats = summarize(item.text);
    return { stats, view: view(stats) };
  },
} satisfies Record<string, WorkerHandlerV2>;

serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("calternal-notes-insights worker protocol failure\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
