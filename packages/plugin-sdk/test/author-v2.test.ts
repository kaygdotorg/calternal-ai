import { describe, expect, it } from "vitest";
import { defineApp, defineChrome, defineKind, definePlugin, defineView } from "../dist/index.js";

const kind = defineKind({
  id: "bookmark",
  icon: "book",
  storage: { hidden: true },
  claimed_text: true,
  schema: { type: "object", properties: { title: { type: "string" } } },
});

describe("v2 author defaults", () => {
  it("fills the manifest defaults and combines authored and inferred reads", () => {
    expect(kind.title_field).toBe("title");
    const plugin = definePlugin({
      api_version: 2, id: "bookmarks", name: "Bookmarks", version: "1.0.0", icon: "book",
      apps: [], kinds: [kind], reads: ["notes", "bookmark"],
    });
    expect(plugin.metadata).toMatchObject({
      reads: ["bookmark", "notes"], network: "none", operations: [],
    });
  });

  it("requires the chosen title field to declare a string", () => {
    expect(() => defineKind({ ...kind, title_field: "name" })).toThrow(/name needs a declared string/);
    expect(defineKind({
      ...kind, title_field: "name", schema: { properties: { name: { type: "string" } } },
    }).title_field).toBe("name");
  });

  it("applies identical defaults to Chrome and View Binds and keeps overrides", () => {
    const view = defineView({
      root: { type: "list", bind: "items" },
      binds: [{ id: "items", source: { type: "list", kind: "bookmark" } }],
    });
    const chrome = defineChrome({
      sidebar: { sections: [{ id: "items", title: "Bookmarks", bind: "items" }] },
      binds: [{
        id: "search",
        source: { type: "search", kind: "bookmark" },
        window: { size: 20 },
        states: { empty: { title: "Search found nothing" } },
      }],
    });
    const app = defineApp({ id: "bookmarks", name: "Bookmarks", icon: "book", view, chrome });
    expect(app.template.binds).toMatchObject([
      {
        fields: ["title"],
        window: { size: 60, count: "exact" },
        row: { title: { field: "title" }, icon: { kindIcon: true } },
        states: { empty: { title: "Nothing here yet" }, loading: { title: "Loading" }, error: { title: expect.any(String) } },
      },
      {
        window: { size: 20, count: "none" },
        row: { title: { field: "title" }, icon: { kindIcon: true } },
        states: { empty: { title: "Search found nothing" }, loading: { title: "Loading" } },
      },
    ]);
  });
});
