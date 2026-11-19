// The harness page: draws one description with the renderer, over the in-page mock host (`?mock=5000`, `&count=none`, `&latency=40`) or a real host (`?host=<base>&view=<ref>`; the bearer is in `?bearer=`,
// never logged). `?fixture=reading-list|notes` picks the sealed fixture. The suites and the performance measures drive this page; it holds no product code.
import { mount } from "svelte";
import "@calternal/ui/styles";
import { initInputModality, initScheme } from "@calternal/ui";
import { AppView, HostSession, HttpTransport, type AppDescription } from "../src";
import { MockHost } from "../testing/mock-host";
// every sealed description fixture, by file name without the extension
const all = import.meta.glob("../../../contract/fixtures/description-v2/*.json", { eager: true, import: "default" }) as Record<string, unknown>;

const q = new URLSearchParams(location.search);
initInputModality(); initScheme();
const fixtures: Record<string, unknown> = Object.fromEntries(Object.entries(all).map(([k, v]) => [k.split("/").pop()!.replace(/\.json$/, ""), v]));
fixtures["reading-list"] = fixtures["reading-list-app"]; fixtures.notes = fixtures["notes-app"];
const rows = Number(q.get("mock") ?? 0);
let description = fixtures[q.get("fixture") ?? "reading-list"] as AppDescription;
let viewRef = q.get("view") ?? "v-mock";
const bearer = q.get("bearer") ?? undefined;
if (rows === 0) {
  // A real host: its /fixture names the views it will open and carries each one's template.
  const f = await (await fetch(`${q.get("host")!.replace(/\/$/, "")}/fixture`, { headers: bearer ? { authorization: `Bearer ${bearer}` } : {} })).json() as { views: Record<string, string>; templates: Record<string, AppDescription> };
  const name = q.get("viewName") ?? "home";
  viewRef = f.views[name]!; description = f.templates[name] ?? f.templates.shared!;
}
const parentBinds = (description.binds ?? []).filter((b) => (b.source as { parent?: boolean }).parent).map((b) => b.id);
const transport = rows > 0
  ? new MockHost({ rows, parentBinds, rowsFor: (b) => ((description.binds ?? []).find((x) => x.id === b)?.source.type === "item" ? 1 : rows), count: (q.get("count") as "exact" | "none") ?? "exact", latencyMs: Number(q.get("latency") ?? 0), skipLimit: q.has("skip") ? Number(q.get("skip")) : undefined, endUnavailable: q.get("noend") === "1", fields: (i, bind) => ({ title: `${bind} ${i}`, added: 1_700_000_000 - i, read: i % 7 === 0, updated: 1_700_000_000 - i, pinned: i % 11 === 0, snippet: `Snippet of ${bind} ${i}`, thumbnail: `thumb-${i}-aaaaaaaaaaaaaaaaaaaaaaaa`, due: "Fri", unread: i % 3 === 0 }) })
  : new HttpTransport({ base: q.get("host")!, bearer });
const session = new HostSession({ transport });
const w = window as unknown as Record<string, unknown>;
w.__session = session; w.__t0 = performance.now(); w.__transport = transport;
const t0 = performance.now();
await session.open(viewRef);
w.__opened = performance.now() - t0;
mount(AppView, { target: document.getElementById("app")!, props: { description, session, kindIcons: { "reading-item": "notes", "reading-reminder": "clock", notes: "notes", collections: "folder" } } });
// First rows painted: the first skeleton-free row of the first list.
const mo = new MutationObserver(() => { if (!w.__painted && document.querySelector("[data-i] .row:not([data-skeleton])")) { w.__painted = performance.now() - t0; mo.disconnect(); } });
mo.observe(document.body, { childList: true, subtree: true });
