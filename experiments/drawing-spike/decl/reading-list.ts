// Reading List, as authoring.md section 2.1 writes it (on lane/v2-host-base, 2026-11-21). Lines that differ from the
// document end in a `// Dn` comment naming the change in docs/reviews/drawing-spike-2026-11-21.md; nothing else changed.
import { plugin, kind, text, url, bool, collection, create, set } from "./sdk.ts";
export default plugin({
  id: "reading-list", name: "Reading List", version: "2.0.0", icon: "book",
  kinds: { reading: kind({ fields: { url: url().label("Link"), title: text().index(), read: bool(false).index().values({ false: "To read", true: "Read" }) }, // D3 D4
    row: { title: "title", detail: ["site(url)", "created"], trailing: [{ dot: { when: "!read" } }] },
    card: { title: "title", detail: ["site(url)"] } }) },
  operations: { add: create("reading", { inputs: ["url", "title"] }, { title: "Add link" }), // D2
    done: set("reading", { read: true }, { title: "Mark read", icon: "check", when: "!read", place: "swipe" }) }, // D1
  sidebar: [collection("Unread", { kind: "reading", where: "!read", sort: "-created" }),
    collection("All", { kind: "reading", sort: "-created", group: "read" })], // D5
});
