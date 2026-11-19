import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const root = new URL("../dist/", import.meta.url);
const fixture = new URL("../../../contract/fixtures/description-v2/", import.meta.url);
const readJson = (url: URL) => JSON.parse(readFileSync(url, "utf8"));

it("matches the sealed Reading List manifest apart from package identity and emitted asset pins", () => {
  const emitted = readJson(new URL("manifest.json", root));
  const sealed = readJson(new URL("manifests/reading-list-manifest-v2-app.json", fixture));
  expect(emitted.id).toBe("calternal-reading-list");
  for (const asset of [emitted.worker, emitted.apps[0].description.template]) {
    const bytes = readFileSync(new URL(asset.path, root));
    expect(createHash("sha256").update(bytes).digest("hex")).toBe(asset.sha256);
  }

  expect({
    ...emitted,
    id: sealed.id,
    worker: sealed.worker,
    apps: emitted.apps.map((app: typeof emitted.apps[number], index: number) => ({
      ...app,
      description: { ...app.description, template: sealed.apps[index].description.template },
    })),
  }).toEqual(sealed);
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
