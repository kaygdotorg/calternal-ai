/** Configurable composer accessory using the ordinary SDK. */
import type { GenUiView } from "@calternal/contracts";
import { compareAndSetPluginState, readPluginState, readSetting, readSpaceUsage, serveWorker, type WorkerHandlerV2 } from "@calternal/plugin-sdk";
import manifest from "../manifest.json" with { type: "json" };

const handlers = {
  "composer-settings.saved": async (_input, context) => ({ state: await readPluginState(context, "fixture") }),
  "composer-settings.remember": async (input, context) => {
    const current = await readPluginState(context, "fixture");
    await compareAndSetPluginState(context, "fixture", current?.revision ?? null, 1, input);
    // The caller still receives its durable host acknowledgment.
    return { invalid: true };
  },
  "composer-settings.admit": async input => {
    const { storage } = input as { storage: { text_bytes: string } };
    const used = BigInt(storage.text_bytes);
    if (used === 0n) return { decision: "allow" };
    if (used > 1024n) return { decision: "refuse", reason: "Fixture byte ceiling reached" };
    return { decision: "queue", reason: "Fixture is waiting", retry_after_ms: 500 };
  },
  "composer-settings.usage": async (input, context) => {
    const { scope } = input as { scope?: "owner" };
    return readSpaceUsage(context, scope);
  },
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
