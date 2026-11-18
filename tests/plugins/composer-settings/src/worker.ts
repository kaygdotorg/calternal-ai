/** Configurable composer accessory using the ordinary SDK. */
import type { GenUiView } from "@calternal/contracts";
import { readSetting, serveWorker, type WorkerHandlerV2 } from "@calternal/plugin-sdk";
import manifest from "../manifest.json" with { type: "json" };

const handlers = {
  "composer-settings.label": async (input, context) => {
    const key = input && typeof input === "object" && "key" in input && typeof input.key === "string"
      ? input.key
      : "label";
    const label = await readSetting(context, key);
    if (typeof label !== "string") throw new Error("Expected a label string");
    const view: GenUiView = {
      genui_version: 2,
      source: "Badge(" + JSON.stringify(label) + ")",
    };
    return { view };
  },
} satisfies Record<string, WorkerHandlerV2>;

await serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
);
