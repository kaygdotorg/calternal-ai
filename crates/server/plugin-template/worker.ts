/** Example plugin with a manifest-typed operation and a portable view. */
import { serveWorker, typedHandlers } from "@calternal/plugin-sdk";
import manifest from "./manifest.generated.js";

const handlers = typedHandlers(manifest, {
  "__PLUGIN__.hello": async (input, context) => {
    if (input.reads) {
      await context.callAll(Array.from({ length: 3 }, () => ["read_item", {
        kind: "__PLUGIN__",
        item_id: "01900000-0000-7000-8000-000000000003",
      }] as const));
    }
    return { view: { genui_version: 2, source: 'Row([Metric("Plugin", "Ready")])' } };
  },
});

serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("Worker protocol error. Check that the host speaks worker-v2.\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
