import { defineApp, defineChrome, defineKind, definePlugin, defineView } from "@calternal/plugin-sdk";

export default definePlugin({
  api_version: 2,
  id: "bookmarks",
  name: "Bookmarks",
  icon: "book",
  version: "1.0.0",
  kinds: [defineKind({
    id: "bookmark",
    icon: "book",
    storage: { hidden: true },
    claimed_text: true,
    schema: {
      type: "object",
      required: ["title"],
      additionalProperties: false,
      properties: { title: { type: "string" } },
    },
  })],
  apps: [defineApp({
    id: "bookmarks",
    name: "Bookmarks",
    icon: "book",
    view: defineView({
      binds: [{ id: "items", source: { type: "list", kind: "bookmark" } }],
      root: { type: "list", bind: "items" },
    }),
    chrome: defineChrome({
      sidebar: { sections: [{ id: "items", title: "Bookmarks", bind: "items" }] },
    }),
  })],
});
