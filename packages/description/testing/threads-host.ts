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
/** What the viewer-private fields say per thread in the 14.43 fixture: shape, counts, mute, pin and the last line (the draft when there is one). */
export const EXTRA: Record<string, { shape: string; unread: number; mentions: number; muted: boolean; pinned: boolean; preview: string }> = {
  "t-lunch": { shape: "mixed", unread: 3, mentions: 1, muted: false, pinned: true, preview: "River it is. Booking now." },
  "t-release": { shape: "agent", unread: 0, mentions: 0, muted: false, pinned: true, preview: "Draft: Add the new search filters to the" },
  "t-bug": { shape: "agent", unread: 12, mentions: 0, muted: false, pinned: false, preview: "Codex: the index folds accents now." },
  "t-design": { shape: "group", unread: 41, mentions: 2, muted: true, pinned: false, preview: "Ben: 12 px between rows looks right" },
  "t-trip": { shape: "person", unread: 0, mentions: 0, muted: false, pinned: false, preview: "Lena: I can do the 14th to the 18th" },
  "t-recipe": { shape: "agent", unread: 0, mentions: 0, muted: true, pinned: false, preview: "Claude: Less sugar, more buttermilk." },
  "t-tax": { shape: "group", unread: 5, mentions: 0, muted: true, pinned: false, preview: "Sam: Receipts are in the folder" },
};
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
export interface ThreadsHostOptions { scenario?: string; actionIds: string[]; latencyMs?: number; /** The 14.43 interface: viewer-private fields, entries and their overlays. */ next?: boolean }

/** A `MockHost` whose view open also issues the contexts of the description's view-level actions (the composer's Send, the header's Archive, the New thread shortcut). */
class ViewContexts extends MockHost {
  override async post<T>(route: Route, body: unknown): Promise<T> {
    const r = await super.post<T>(route, body);
    if (route === "view.open") (r as { activation_contexts: Record<string, string> }).activation_contexts = { send: "ctx-send", "archive-open": "ctx-archive-open", "create-thread": "ctx-create-thread", "call-in": "ctx-call-in", "call-range": "ctx-call-range", "mute-thread": "ctx-mute-thread", "pin-thread": "ctx-pin-thread" };
    return r;
  }
}

/** The host: a `MockHost` over the thread lists, plus the item-view, choice and activation routes the Threads description uses. */
export function threadsHost(o: ThreadsHostOptions) {
  const empty = o.scenario === "empty";
  const byPin = [...THREADS].sort((a, b) => Number(EXTRA[b.id]?.pinned ?? 0) - Number(EXTRA[a.id]?.pinned ?? 0) || a.ago - b.ago);
  const lists: Record<string, { id: string; title: string; ago: number }[]> = o.next
    ? { all: empty ? [] : byPin, people: byPin.filter((t) => ["person", "group"].includes(EXTRA[t.id]!.shape)), agents: byPin.filter((t) => EXTRA[t.id]!.shape === "agent"), archived: ARCHIVED, "archived-count": ARCHIVED, thread: [THREADS[0]!], found: THREADS.slice(0, 3) }
    : { threads: empty ? [] : THREADS, archived: ARCHIVED, "archived-count": ARCHIVED, thread: [THREADS[0]!], found: THREADS.slice(0, 3) };
  const counts: Record<string, number> = Object.fromEntries(Object.entries(lists).map(([k, v]) => [k, v.length]));
  const rowOf = (bind: string, i: number) => lists[bind]?.[i] ?? THREADS[0]!;
  const sent: { text: string; harness: string; placement: string }[] = [];
  // ---- 14.43 entry views: the viewer is Ada (own entries); every other author is someone else. Reading state, drafts and extra messages are per fixture host, in memory.
  const unreadLeft = new Map<string, number>(), drafts = new Map<string, { v: number; text: string }>(), extra: { who: string; text: string; at: number; quote?: string }[] = [];
  const A = (n: number) => `e${n}`;
  const T = (m: number) => iso(m * 60);
  function entries(id: string) {
    const E = (n: number, key: string, name: string, mins: number, children: unknown[], more: Record<string, unknown> = {}) => ({ type: "snapshot-entry", anchor: A(n), timestamp: T(mins), author: { key, name }, children, ...more });
    const txt = (text: string, markup = true) => ({ type: "snapshot-text", text, ...(markup ? { markup: "inline" } : {}) });
    const list: unknown[] = [
      { type: "snapshot-marker", kind: "date", timestamp: T(60 * 26) },
      E(1, "p-ada", "Ada", 60 * 26, [txt("Can we agree the plan before Friday? I would like to book by **Wednesday**.")]),
      E(2, "p-ben", "Ben", 60 * 26 - 3, [txt("Yes. Two options: the old market, or the place by the river.")]),
      E(3, "p-ben", "Ben", 60 * 26 - 4, [txt("The market is closed on Fridays, I think.")], { edited_at: T(60 * 25) }),
      { type: "snapshot-marker", kind: "system", text: "Ada called Codex in, reading messages 1 to 3" },
      E(4, "a-codex", "Codex", 60 * 26 - 6, [txt("I read the three messages. The river place has a free table at `19:00` on Friday; the market is closed. [Their menu](https://example.test/menu)")], { asked_by: "Ada" }),
      { type: "snapshot-marker", kind: "date", timestamp: T(60 * 5) },
      E(5, "p-ada", "Ada", 60 * 5, [txt("River it is. Booking now.")]),
      E(6, "p-ben", "Ben", 60 * 4, [{ type: "snapshot-quote", source: { item: id, revision: "rev-1", span: "s5" } }, { type: "snapshot-attachment", ref: "asset-menu", label: "menu-friday.pdf" }, txt("Great. *Please* ask for the terrace.")]),
      E(7, "a-claude", "Claude", 60 * 2, [txt("A consult with Codex is done: both of us recommend the terrace table."), { type: "snapshot-link", label: "Consultation with Codex", target: { space: "sp", id: "t-consult" } }, { type: "snapshot-code", text: "curl -X POST https://example.test/book \\\n  -d 'time=19:00'" }], { asked_by: "Ben" }),
      E(8, "p-ben", "Ben", 20, [txt("@Ada can you confirm the terrace?")]),
      E(9, "p-ben", "Ben", 19, [txt("Sending from my phone…")], { state: "queued" }),
      ...extra.map((x, k) => E(10 + k, x.who === "Ada" ? "p-ada" : "p-ben", x.who, Math.max(0, Math.round((Date.now() - x.at) / 60000)), [...(x.quote ? [{ type: "snapshot-quote", source: { item: id, revision: "rev-1", span: "s5" } }] : []), txt(x.text)])),
    ];
    return list;
  }
  function page(id: string) {
    const left = unreadLeft.get(id) ?? 3, es = entries(id);
    const anchors = (es as { type: string; anchor?: string }[]).filter((e) => e.type === "snapshot-entry").map((e) => e.anchor!);
    const ctrls = anchors.map((a) => {
      const mine = (es as { anchor?: string; author?: { key: string } }[]).find((e) => e.anchor === a)?.author?.key === "p-ada";
      const agent = (es as { anchor?: string; author?: { key: string } }[]).find((e) => e.anchor === a)?.author?.key?.startsWith("a-");
      const act = (id2: string) => ({ action: id2, activation_context: `ctx-${id2}-${a}` });
      return { anchor: a, entry: `h-${a}`, ...(mine ? { own: true as const } : {}), ...(agent ? { provider_icon: (es as { anchor?: string; author?: { key: string } }[]).find((e) => e.anchor === a)!.author!.key === "a-codex" ? "codex" : "claude" } : {}),
        actions: [act("reply"), ...(mine ? [act("edit"), act("delete")] : []), act("call-from-here")] };
    });
    const firstUnread = anchors[anchors.length - Math.min(left, anchors.length)];
    const quotes = [{ entry: "e6", source: { item: id, revision: "rev-1", span: "s5" }, text: "River it is. Booking now.", author: "Ada", target: { anchor: "e5", item: { space: "sp", id } } },
      ...extra.filter((x) => x.quote).map((_, k) => ({ entry: A(10 + k), source: { item: id, revision: "rev-1", span: "s5" }, text: "River it is. Booking now.", author: "Ada", target: { anchor: "e5", item: { space: "sp", id } } }))];
    return { state: o.scenario === "stale" ? "stale" : "live", source_revision: `rev-${extra.length}`, window_ref: `win-${id}`, result_version: `v-${extra.length}`, ...(o.scenario === "stale" ? {} : { follow: `f-${id}-${extra.length}` }),
      view: { content: { type: "stack", children: es } }, quotes, entry_controls: ctrls,
      access: o.scenario === "lapsed" ? { state: "read_only", reason: "lapsed" } : { state: "read_write" },
      reading: { unread: left, mentions: left > 0 ? 1 : 0, ...(left > 0 ? { first_unread: firstUnread, next_mention: A(8) } : {}), version: `rd-${left}` },
      ...(o.scenario === "consent" ? { requests: [{ anchor: A(8), card: { request_ref: "rq-1", title: "Ada asks Codex to read your messages 1 to 3", detail: "Codex would read the three messages you wrote in this thread.", material: [], state: "open", card_context: "ctx-card", expires_at: new Date(Date.now() + 60000).toISOString(), actions: [{ decision: "approve", lifetimes: ["once", "this_item"] }, { decision: "deny" }] } }] } : {}),
      activity: [{ label: "Codex is working", kind: "working", expires_at: new Date(Date.now() + 120000).toISOString() }] };
  }
  async function nextRoute(route: Route, body: any): Promise<unknown | undefined> {
    const id = String(body.item ?? body.window_ref?.replace(/^win-/, "") ?? "t-lunch");
    switch (route) {
      case "item_view.open": if (o.scenario === "reconnect" && ++opens >= 2 && opens <= 4) throw new Error("connection lost"); if (o.scenario === "loading") await new Promise((r) => setTimeout(r, 60_000)); if (o.scenario === "unavailable") return { state: "unavailable" }; return page(id);
      case "item_view.follow": await new Promise((r) => setTimeout(r, o.scenario === "reconnect" ? 800 : 6000)); if (extra.length < 3) extra.push({ who: extra.length % 2 ? "Ada" : "Ben", text: `Streamed message ${extra.length + 1}`, at: Date.now(), quote: extra.length === 1 ? "x" : undefined }); return { type: "item_view_signal", follow: body.follow, state: "changed" };
      case "item_view.read": { const tid = String(body.window_ref).replace(/^win-/, ""); unreadLeft.set(tid, 0); return { unread: 0, mentions: 0, version: "rd-0" }; }
      case "item_view.selection": return { selection_ref: `sel-${body.first}-${body.last ?? body.first}` };
      case "item.locate": return { item: "t-bug" };
      case "entry_composer.open": { const ref = String(body.composer); const [, mode, anchor] = /^c-(reply|edit)-(.+)$/.exec(ref)!; return { mode, anchor, ...(mode === "edit" ? { initial: anchor === "e5" ? "River it is. Booking now." : "Edited text" } : {}) }; }
      case "entry_composer.mentions": return { options: [{ ref: "m-ben", label: "Ben", kind: "person" }, { ref: "m-codex", label: "Codex", kind: "contribution" }, { ref: "m-claude", label: "Claude", kind: "contribution" }].filter((x) => x.label.toLowerCase().startsWith(String(body.text ?? "").toLowerCase())) };
      case "entry_composer.draft.read": { const d = drafts.get(id) ?? { v: 0, text: "" }; return { version: `d${d.v}`, text: d.text }; }
      case "entry_composer.draft": { const d = { v: (drafts.get(id)?.v ?? 0) + 1, text: String(body.text) }; drafts.set(id, d); return { version: `d${d.v}`, text: d.text }; }
      case "attachment.begin": return { stage: `st-${body.request_id}`, next_offset: 0, max_bytes: 10_000_000, expires_at: iso(-600) };
      case "attachment.chunk": return { stage: body.stage, next_offset: Number(body.offset) + Math.floor((String(body.bytes).length * 3) / 4), max_bytes: 10_000_000, expires_at: iso(-600) };
      case "attachment.finish": return { attachment: `att-${String(body.stage).slice(3, 11)}` };
      case "request.resolve": return { activation_id: "x", state: "ok" };
      case "activation.resolve": {
        const m = /^ctx-(reply|edit)-(.+)$/.exec(String(body.activation_context));
        if (m) return { activation_id: body.activation_id, state: "ok", entry_composer: `c-${m[1]}-${m[2]}` };
        if (body.action_id === "delete" || body.action_id === "call-from-here" || body.action_id === "call-range" || body.action_id === "call-in") return { activation_id: body.activation_id, resolution_ref: "res-x", confirmation_challenge: "chal-x", material: [{ name: "operation", label: "Operation", value: body.action_id === "delete" ? "Delete for everyone" : "Call an agent in" }, { name: "effect", label: "Effect", value: body.action_id === "delete" ? "The message disappears for everyone." : "The agent reads the selected messages. People whose messages are included are asked first." }] };
        if (body.action_id === "send" && !body.interaction?.choices?.harness) { const c = body.interaction?.choices ?? {}; sent.push({ text: String(body.interaction?.form?.composer?.message ?? ""), harness: String(c.harness ?? ""), placement: String(c.placement ?? ""), ...(body.interaction as object) } as never); extra.push({ who: "Ada", text: String(body.interaction?.form?.composer?.message ?? ""), at: Date.now() }); return { activation_id: body.activation_id, state: "ok" }; }
        return undefined;
      }
      default: return undefined;
    }
  }
  let opens = 0;
  let version = 0; const follows = new Map<string, number>();
  const host: MockHost = new ViewContexts({
    rows: 1, actionIds: o.actionIds, latencyMs: o.latencyMs,
    rowsFor: (b) => counts[b] ?? 0,
    fields: (i, b) => {
      const t = rowOf(b, i);
      if (!o.next) return { title: t.title, updated: NOW - t.ago, archived: b === "archived" };
      const ex = EXTRA[t.id] ?? { shape: "person", unread: 0, mentions: 0, muted: false, pinned: false, preview: "" };
      return { title: t.title, updated: NOW - t.ago, archived: b === "archived", ...ex };
    },
    ...(o.next ? { /* the sections of the 14.43 list are filtered by the host; the fixture serves the same seven threads, pinned first */ } : {}),
    intercept: (req) => (o.scenario === "error" && (req.bind === "threads" || req.bind === "all") ? ({ status: 422, code: "unsupported_query_plan", reason: "host_error" } as never) : undefined),
    handle: async (route: Route, body: any) => {
      if (o.next) { const r = await nextRoute(route, body); if (r !== undefined) return r; }
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
      if (route === "choice.options" && body.slot === "mute") return { options: [["1h", "1 hour"], ["8h", "8 hours"], ["2d", "2 days"], ["forever", "Until I unmute"], ["mentions", "Mute, but tell me about mentions"]].map(([ref, label]) => ({ ref: `mute-${ref}`, label })) };
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
