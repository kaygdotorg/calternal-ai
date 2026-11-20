import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const root = new URL("../dist/", import.meta.url);
const fixture = new URL("../../../contract/fixtures/description-v2/", import.meta.url);
const readJson = (url: URL) => JSON.parse(readFileSync(url, "utf8"));

it("matches the Reading List fixture with the draft create and alarm declarations", () => {
  const emitted = readJson(new URL("manifest.json", root));
  const installer = readJson(new URL("manifests/reading-list-installer-v2-app.json", fixture));
  expect(emitted).toEqual(installer);
  const draft = readJson(new URL("manifests/reading-list-manifest-v2-app.json", fixture));
  expect(emitted.id).toBe("calternal-reading-list");
  for (const asset of [emitted.worker, emitted.apps[0].description.template]) {
    const bytes = readFileSync(new URL(asset.path, root));
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(asset.sha256);
  }

  expect({
    ...emitted,
    id: draft.id,
    worker: draft.worker,
    apps: emitted.apps.map((app: typeof emitted.apps[number], index: number) => ({
      ...app,
      description: { ...app.description, template: draft.apps[index].description.template },
    })),
  }).toEqual(draft);
});

it("matches the authored app with the SDK's window and message defaults", () => {
  const manifest = readJson(new URL("manifest.json", root));
  const emitted = readJson(new URL(manifest.apps[0].description.template.path, root));
  expect(emitted).toEqual(readJson(new URL("reading-list-installer-v2-app.json", fixture)));
  const authored = readJson(new URL("reading-list-manifest-v2-app.json", fixture));
  expect(emitted).toEqual({
    ...authored,
    binds: authored.binds.map((bind: typeof authored.binds[number]) => {
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
