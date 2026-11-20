// The harness page: draws one description with the renderer, over the in-page mock host (`?mock=5000`, `&count=none`, `&latency=40`) or a real host (`?host=<base>&view=<ref>`; the bearer is in `?bearer=`,
// never logged). `?fixture=reading-list|notes` picks the sealed fixture. The suites and the performance measures drive this page; it holds no product code.
import { mount } from "svelte";
import "@calternal/ui/styles";
import { initInputModality, initScheme } from "@calternal/ui";
import { AppView, HostSession, HttpTransport, ProductTransport, type AppDescription } from "../src";
import { MockHost } from "../testing/mock-host";
import { SyntheticSource } from "../testing/synthetic-source";
import { threadsHost } from "../testing/threads-host";
import { description as threadsDescription } from "../../../plugins/calternal-threads/src/description";
import { descriptionNext as threadsNext } from "../../../plugins/calternal-threads/src/description-next";
// every sealed description fixture, by file name without the extension
const all = import.meta.glob(["../../../contract/fixtures/description-v2/*.json", "!../../../contract/fixtures/description-v2/invalid-*.json"], { eager: true, import: "default" }) as Record<string, unknown>;

const q = new URLSearchParams(location.search);
initInputModality(); initScheme();
const fixtures: Record<string, unknown> = Object.fromEntries(Object.entries(all).map(([k, v]) => [k.split("/").pop()!.replace(/\.json$/, ""), v]));
fixtures["reading-list"] = fixtures["reading-list-app"]; fixtures.notes = fixtures["notes-app"];
const rows = Number(q.get("mock") ?? 0);
const next = q.get("fixture") === "threads-next", threads = q.get("fixture") === "threads" || next;
let description = (next ? threadsNext : threads ? threadsDescription : fixtures[q.get("fixture") ?? "reading-list"]) as unknown as AppDescription;
let viewRef = q.get("view") ?? "v-mock";
const bearer = q.get("bearer") ?? undefined;
// A product route (`?product=<view address>&token=<bearer>`): the view describes itself.
const product = q.has("product") ? new ProductTransport({ base: q.get("product")!, bearer: q.get("token") ?? undefined }) : null;
if (product) { const d = await product.describe(); viewRef = d.view_ref; description = d.description as AppDescription; }
else if (rows === 0 && !threads) {
  // A real host: its /fixture names the views it will open and carries each one's template.
  const f = await (await fetch(`${q.get("host")!.replace(/\/$/, "")}/fixture`, { headers: bearer ? { authorization: `Bearer ${bearer}` } : {} })).json() as { views: Record<string, string>; templates: Record<string, AppDescription> };
  const name = q.get("viewName") ?? "home";
  viewRef = f.views[name]!; description = f.templates[name] ?? f.templates.shared!;
}
// `?extend=multi:<bind>`: the page adds a multiple-selection list of a bind the template already declares to the view (a valid description node), so a template whose lists sit in the sidebar can be ticked in a suite.
const ext = /^multi:(.+)$/.exec(q.get("extend") ?? "");
if (ext && description.view?.root.type === "stack") description = { ...description, view: { ...description.view, root: { ...description.view.root, children: [...description.view.root.children, { type: "list", bind: ext[1]!, selectionMode: "multiple", title: "Pick notes" }] } } };
const parentBinds = (description.binds ?? []).filter((b) => (b.source as { parent?: boolean }).parent).map((b) => b.id);
const actionIds = [...new Set((description.binds ?? []).flatMap((b) => [...(b.row?.leadingActions ?? []), ...(b.row?.trailingActions ?? []), ...(b.row?.menu ?? [])].map((a) => a.id)))];
// `?challenge=1`: the mock host answers every activation with a confirmation challenge and the material it wrote (to see the protected sheet), a choice with two options, Undo and status.
const challenge = q.get("challenge") === "1";
const handle = challenge ? (route: string, body: any) => route === "activation.resolve" ? { activation_id: body.activation_id, resolution_ref: "res", confirmation_challenge: "chal", material: [{ name: "destination", label: "Destination", value: "Archive" }, { name: "count", label: "Items", value: 1 }] } : route === "activation.confirm" ? { confirmation_proof: "proof" } : route === "activation.dispatch" ? { activation_id: body.activation_id, state: "ok", undo: { ref: "undo-1", label: "Moved", expires_at: "2026-11-19T23:00:00Z" } } : route === "choice.options" ? { bind: "collections", options: [{ ref: "ch-1", label: "Archive" }, { ref: "ch-2", label: "Review" }] } : route === "undo" ? { activation_id: "u", state: "ok" } : undefined : undefined;
// `?fixture=threads[&scenario=empty|error|stale|loading|unavailable]`: the real Threads description (the plugin's own file) over the fixture host.
const threadsFixture = threads ? threadsHost({ next, scenario: q.get("scenario") ?? undefined, actionIds: [...new Set([...actionIds, "open"])] }) : null;
const transport = threadsFixture ? threadsFixture : rows > 0 && !product
  ? new MockHost({ rows, parentBinds, actionIds, handle: handle as never, rowsFor: (b) => ((description.binds ?? []).find((x) => x.id === b)?.source.type === "item" ? 1 : rows), count: (q.get("count") as "exact" | "none") ?? "exact", latencyMs: Number(q.get("latency") ?? 0), skipLimit: q.has("skip") ? Number(q.get("skip")) : undefined, endUnavailable: q.get("noend") === "1", fields: (i, bind) => ({ title: `${bind} ${i}`, added: 1_700_000_000 - i, read: i % 7 === 0, updated: 1_700_000_000 - i, pinned: i % 11 === 0, snippet: `Snippet of ${bind} ${i}`, thumbnail: `thumb-${i}-aaaaaaaaaaaaaaaaaaaaaaaa`, due: "Fri", unread: i % 3 === 0 }) })
  : product ?? new HttpTransport({ base: q.get("host")!, bearer });
// `?synthetic=1`: the routes the host has no answer for (transcript, composer options and send, request card) come from the stand-in; everything else passes through.
const source = q.get("synthetic") === "1" ? new SyntheticSource({ inner: transport }) : transport;
const session = new HostSession({ transport: source });
if (threads) { session.app = "threads"; session.formKeyed = true; }
const w = window as unknown as Record<string, unknown>;
w.__session = session; w.__t0 = performance.now(); w.__transport = transport; w.__source = source;
const t0 = performance.now();
await session.open(viewRef);
w.__opened = performance.now() - t0;
mount(AppView, { target: document.getElementById("app")!, props: { description, session, kindIcons: { threads: "threads", "reading-item": "notes", "reading-reminder": "clock", notes: "notes", collections: "folder" } } });
// First rows painted: the first skeleton-free row of the first list.
const mo = new MutationObserver(() => { if (!w.__painted && document.querySelector("[data-i] .row:not([data-skeleton])")) { w.__painted = performance.now() - t0; mo.disconnect(); } });
mo.observe(document.body, { childList: true, subtree: true });
