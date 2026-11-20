/**
 * FIXTURE host for the Threads description (the harness page with `?fixture=threads`, the browser suite `e2e/threads.mjs`): a few threads with made-up people and agents, an archive, a search, the committed
 * transcript of the open thread as snapshot nodes (a heading and text per message: the vocabulary the contract has today), `item_view.follow`, the harness and placement choices and a confirmation for Send. Everything
 * here is invented for display; nothing is a real conversation, and it decides no authority.
 *
 * Scenarios (`?scenario=`): `empty` (no threads), `error` (the list fails), `stale` (the open thread is a retained earlier revision), `loading` (the transcript is slow), `unavailable`.
 */
import { MockHost } from "./mock-host";
import type { Route } from "../src/wire/transport";

/** The fixture threads, newest first. The id is the row id and the thread's item ref. */
export const THREADS: { id: string; title: string; ago: number }[] = [
  { id: "t-lunch", title: "Where to eat on Friday", ago: 300 },
  { id: "t-release", title: "Release notes for 0.4", ago: 3600 },
  { id: "t-bug", title: "Search misses accented names", ago: 7200 * 3 },
  { id: "t-design", title: "Sidebar spacing", ago: 86400 },
  { id: "t-trip", title: "Trip to Lisbon: dates", ago: 86400 * 3 },
  { id: "t-recipe", title: "Pancakes, but fluffier", ago: 86400 * 9 },
  { id: "t-tax", title: "Paperwork for the accountant", ago: 86400 * 30 },
];
export const ARCHIVED = [{ id: "t-old", title: "Moving flat: checklist", ago: 86400 * 60 }, { id: "t-demo", title: "Demo script", ago: 86400 * 90 }];

const NOW = Math.floor(Date.now() / 1000);
const iso = (ago: number) => new Date((NOW - ago) * 1000).toISOString().replace(/\.\d+Z$/, "Z");

/** The snapshot of one thread: invented messages, a code block and an attachment so every node is drawn. */
export function transcript(id: string, extra = 0) {
  const msgs: { who: string; ago: number; text?: string; code?: string; file?: string }[] = [
    { who: "Ada", ago: 86400 + 7200, text: "Can we agree the plan before Friday? I would like to book by Wednesday." },
    { who: "Ben", ago: 86400 + 7000, text: "Yes. Two options: the old market, or the place by the river." },
    { who: "Codex", ago: 86400 + 6900, text: "I read the last two messages. The river place has a free table at 19:00 on Friday; the market is closed on Fridays." },
    { who: "Ada", ago: 3600 * 5, text: "River it is. Booking now." },
    { who: "Ben", ago: 3600 * 4, text: "Here is the menu they sent.", file: "menu-friday.pdf" },
    { who: "Claude", ago: 3600 * 2, text: "A booking script, if you want to reuse it:", code: "curl -X POST https://example.test/book \\\n  -d 'date=friday' -d 'time=19:00'" },
    ...Array.from({ length: extra }, (_, i) => ({ who: i % 2 ? "Ben" : "Ada", ago: 600 - i * 30, text: `Streamed message ${i + 1}` })),
  ];
  return { type: "stack", children: msgs.flatMap((m) => [
    { type: "snapshot-heading", title: m.who, author: m.who === "Codex" || m.who === "Claude" ? "agent" : undefined, timestamp: iso(m.ago), anchor: `a-${id}-${m.ago}` },
    ...(m.text ? [{ type: "snapshot-text", text: m.text }] : []),
    ...(m.code ? [{ type: "snapshot-code", text: m.code }] : []),
    ...(m.file ? [{ type: "snapshot-attachment", ref: `asset-${m.file}`, label: m.file }] : []),
  ]) };
}

/** Options of the fixture host. */
export interface ThreadsHostOptions { scenario?: string; actionIds: string[]; latencyMs?: number }

/** A `MockHost` whose view open also issues the contexts of the description's view-level actions (the composer's Send, the header's Archive, the New thread shortcut). */
class ViewContexts extends MockHost {
  override async post<T>(route: Route, body: unknown): Promise<T> {
    const r = await super.post<T>(route, body);
    if (route === "view.open") (r as { activation_contexts: Record<string, string> }).activation_contexts = { send: "ctx-send", "archive-open": "ctx-archive-open", "create-thread": "ctx-create-thread" };
    return r;
  }
}

/** The host: a `MockHost` over the thread lists, plus the item-view, choice and activation routes the Threads description uses. */
export function threadsHost(o: ThreadsHostOptions) {
  const empty = o.scenario === "empty";
  const counts: Record<string, number> = { threads: empty ? 0 : THREADS.length, archived: ARCHIVED.length, "archived-count": ARCHIVED.length, thread: 1, found: 3 };
  const rowOf = (bind: string, i: number) => {
    if (bind === "archived") return ARCHIVED[i]!;
    if (bind === "found") return THREADS[i]!;
    return THREADS[i] ?? THREADS[0]!;
  };
  let version = 0; const follows = new Map<string, number>();
  const sent: { text: string; harness: string; placement: string }[] = [];
  const host: MockHost = new ViewContexts({
    rows: 1, actionIds: o.actionIds, latencyMs: o.latencyMs,
    rowsFor: (b) => counts[b] ?? 0,
    fields: (i, b) => { const t = rowOf(b, i); return { title: t.title, updated: NOW - t.ago, archived: b === "archived" }; },
    intercept: (req) => (o.scenario === "error" && req.bind === "threads" ? ({ status: 422, code: "unsupported_query_plan", reason: "host_error" } as never) : undefined),
    handle: async (route: Route, body: any) => {
      if (route === "item_view.open") {
        if (o.scenario === "loading") await new Promise((r) => setTimeout(r, 60_000));
        if (o.scenario === "unavailable") return { state: "unavailable" };
        const id = String(body.item), n = follows.get(id) ?? 0;
        const view = { content: transcript(id, n) };
        const follow = `f-${id}-${n}`;
        return { state: o.scenario === "stale" ? "stale" : "live", source_revision: `rev-${n}`, window_ref: `p-${n}`, result_version: `v-${n}`, ...(o.scenario === "stale" ? {} : { follow }), view };
      }
      if (route === "item_view.follow") {
        // The first follow answers when something is "committed": the host appends a message every few seconds while a follow is open.
        const f = String(body.follow), id = f.replace(/^f-/, "").replace(/-\d+$/, "");
        await new Promise((r) => setTimeout(r, 4000));
        follows.set(id, Math.min(6, (follows.get(id) ?? 0) + 1)); version++;
        return { type: "item_view_signal", follow: f, state: "changed" };
      }
      if (route === "extension.choices") return { options: [{ ref: "h-claude", label: "Claude Code", harness_ref: "harness-claude" }, { ref: "h-codex", label: "Codex", harness_ref: "harness-codex" }] };
      if (route === "choice.options") return { host: "execution_placement", options: [{ ref: "desk", label: "Desk machine · My login" }, { ref: "laptop", label: "Laptop · My login" }], default_ref: "desk" };
      if (route === "activation.resolve") {
        if (body.action_id === "send") {
          const c = body.interaction?.choices ?? {};
          sent.push({ text: String(body.interaction?.form?.composer?.message ?? ""), harness: String(c.harness), placement: String(c.placement) });
          return { activation_id: body.activation_id, resolution_ref: "res-send", confirmation_challenge: "chal-send", material: [
            { name: "operation", label: "Operation", value: "Send" }, { name: "effect", label: "Effect", value: "Start an agent run" }, { name: "input:placement", label: "Machine and login", value: c.placement === "laptop" ? "Laptop · My login" : "Desk machine · My login" }] };
        }
        return { activation_id: body.activation_id, state: "ok", ...(body.action_id === "create-thread" ? { created: "t-new" } : {}) };
      }
      if (route === "activation.confirm") return { activation_id: body.activation_id, resolution_ref: body.resolution_ref, confirmation_proof: "proof" };
      if (route === "activation.dispatch") return { activation_id: body.activation_id, state: "ok" };
      if (route === "selection.settled") return { type: "selection_context", activation_contexts: { open: `ctx-${body.row}` } };
      if (route === "search") return { ok: true };
      throw new Error(`The threads fixture has no ${route}.`);
    },
  });
  void version;
  return Object.assign(host, { sent });
}
