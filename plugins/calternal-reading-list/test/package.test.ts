import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const root = new URL("../dist/", import.meta.url);
const fixture = new URL("../../../contract/fixtures/description-v2/", import.meta.url);
const readJson = (url: URL) => JSON.parse(readFileSync(url, "utf8"));

it("matches the Reading List fixture with the draft create and alarm declarations", () => {
  const emitted = readJson(new URL("manifest.json", root));
  const sealed = readJson(new URL("manifests/reading-list-manifest-v2-app.json", fixture));
  expect(emitted.id).toBe("calternal-reading-list");
  for (const asset of [emitted.worker, emitted.apps[0].description.template]) {
    const bytes = readFileSync(new URL(asset.path, root));
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(asset.sha256);
  }

  const { schedule_eligibility: [due], ...original } = sealed;
  const createdOutput = {
    type: "object",
    additionalProperties: false,
    required: ["created"],
    properties: { created: { $ref: "host-catalogue-v2.json#/$defs/ItemRef" } },
  };

  expect({
    ...emitted,
    id: sealed.id,
    worker: sealed.worker,
    apps: emitted.apps.map((app: typeof emitted.apps[number], index: number) => ({
      ...app,
      description: { ...app.description, template: sealed.apps[index].description.template },
    })),
  }).toEqual({
    ...original,
    jobs: [{ id: due.id, run_operation: due.operation, replayable: true, alarm: true }],
    operations: original.operations.map((operation: typeof original.operations[number]) => {
      if (operation.id === "reading.add") {
        return { ...operation, methods: ["item_create"], output_schema: createdOutput };
      }
      if (operation.id === "reading.remind") {
        return { ...operation, methods: ["item_create", "alarm_set"], output_schema: createdOutput };
      }
      return operation;
    }),
  });
});

it("matches the sealed app with the SDK's window and message defaults", () => {
  const manifest = readJson(new URL("manifest.json", root));
  const emitted = readJson(new URL(manifest.apps[0].description.template.path, root));
  const sealed = readJson(new URL("reading-list-manifest-v2-app.json", fixture));
  expect(emitted).toEqual({
    ...sealed,
    binds: sealed.binds.map((bind: typeof sealed.binds[number]) => {
      const search = bind.source.type === "search";
      return {
        ...bind,
        window: bind.window ?? { size: 60, count: "none" },
        states: {
          empty: { title: search ? "No results" : "Nothing here yet" },
          loading: { title: "Loading" },
          error: { title: "Couldn’t load items" },
          ...bind.states,
        },
      };
    }),
  });
});
