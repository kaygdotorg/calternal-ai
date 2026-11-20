// DRAFT SIDEBAR (issue #78): the sidebar below uses shapes the contract does not have yet (a pinned strip, avatar marks, per-person arrangement of flat sections: no subsections, no indentation); everything else is 14.43.
// The Threads interface at contract revision 14.43 (generic entry views, viewer-private fields, read position, checked per-entry actions, range selection, composer drafts and mentions, literal choices, quiet marks,
// panel content). It is NOT yet wired into the plugin: the build pins 4a1e099b (14.41), whose schema and manifest do not carry these shapes, and the manifest still has to declare the kind's `viewer_fields`
// (`unread` and `mentions` from the entry order, `muted`, `pinned`, `draft`, `preview`) and the operations named below (`threads.state`, `threads.edit`, `threads.delete`, `threads.call`, `threads.send` without a harness).
// When the plugin lane re-pins, this file replaces `description.ts`. Checked here against the 14.43 schema (issue #99). Data-free: every number, name and message arrives through a bind or the item view.
const item = { route: "item" } as const;
const row = { row: "id" } as const;
const entry = { entry: "anchor" } as const;
const eq = (field: string, value: unknown) => ({ field, op: "eq", value: { literal: value } });
const newest = [{ field: "updated", direction: "desc" }];
const fields = ["title", "updated", "preview", "unread", "mentions", "muted", "pinned", "shape"];
const mute = (id: string, title: string, inputs: Record<string, unknown>) => ({ id, title, icon: "bell", operation: "threads.state", inputs: { item: row, ...inputs } });
const muteFor = { prompt: { title: "Mute for", kind: "choice", options: [
  { value: "1h", label: "1 hour" }, { value: "8h", label: "8 hours" }, { value: "2d", label: "2 days" }, { value: "forever", label: "Until I unmute" }, { value: "mentions", label: "Mute, but tell me about mentions" }] } };
const pin = (id: string, on: boolean) => ({ id, title: on ? "Pin" : "Unpin", icon: "spark", operation: "threads.state", inputs: { item: row, pinned: { literal: on } } });
/** A thread row is one line: avatar, title, then the time and the counts (a muted thread keeps only its mention count, in the accent: it never rings). */
const rowTemplate = (n: string) => ({
  title: { field: "title", fallback: "Untitled thread" },
  icon: { avatar: { label: "title" } },
  marks: [
    { kind: "text", field: "updated" },
    { kind: "count", field: "mentions", when: eq("muted", true) },
    { kind: "count", field: "unread", when: eq("muted", false) },
  ],
  open: { route: { name: "item", value: "id" } },
  leadingActions: [pin(`pin-${n}`, true), { id: `unread-mark-${n}`, title: "Mark unread", icon: "check", operation: "threads.state", inputs: { item: row, unread_mark: { literal: true } } }],
  trailingActions: [{ id: `archive-${n}`, title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item: row } }, mute(`mute-${n}`, "Mute", { mute: muteFor })],
  menu: [
    pin(`unpin-${n}`, false), mute(`unmute-${n}`, "Unmute", { mute: { literal: "off" } }),
    { id: `rename-${n}`, title: "Rename", icon: "edit", operation: "host.item.rename", inputs: { item: row, title: { prompt: { title: "Rename thread", initial: { row: "field", field: "title" } } } } },
    { id: `trash-${n}`, title: "Move to Trash", icon: "trash", role: "destructive", operation: "host.item.trash", inputs: { item: row } },
  ],
});
const list = (id: string, filter: unknown[], empty: unknown, extra: Record<string, unknown> = {}) => ({
  id, source: { type: "list", kind: "threads" }, fields, filter: { all: filter }, sort: [{ field: "pinned", direction: "desc" }, ...newest], window: { size: 60, count: "exact" },
  row: rowTemplate(id), states: { empty, error: { title: "Threads could not load", action: { id: `retry-${id}`, title: "Try again", shell: { reload: true } } } }, ...extra,
});
const none = { title: "Nothing here", detail: "Threads that match come back by themselves." };
const harness = (showWhen: unknown) => ({ prompt: { title: "Agent", kind: "choice", options: { extension: "harnesses" }, showWhen } });
const place = (showWhen: unknown) => ({ prompt: { title: "Run on", kind: "choice", options: { host: "execution_placement" }, showWhen } });
const ask = { form: "composer", field: "ask", op: "eq", value: true };
const callIn = (id: string, title: string, scope: unknown, extra: Record<string, unknown> = {}) => ({
  id, title, icon: "spark", operation: "threads.call",
  inputs: { item, scope, harness: harness(undefined), placement: place(undefined), ...extra },
});
const notArchived = eq("archived", false), unpinned = eq("pinned", false), pinnedOnly = eq("pinned", true);
const pins = (id: string, bind: string) => ({ id, presentation: "strip", bind, drop: { action: "pin" }, dragOut: { action: "unpin" } });

export const descriptionNext = {
  version: 2, kind: "app", revision: 3, cache: { templates: "keep" },
  binds: [
    list("pinned", [notArchived, pinnedOnly], none),
    list("groups", [notArchived, unpinned, { field: "shape", op: "in", value: { literal: ["group", "mixed"] } }], { title: "No groups yet", detail: "A conversation with several people appears here." }),
    list("contacts", [notArchived, unpinned, eq("shape", "person")], { title: "No conversations yet", detail: "Start one with New thread." }),
    // A Cal is a saved assistant definition (docs/architecture-proposal.md): its row opens to its threads (nested child rows).
    { id: "cals", source: { type: "list", kind: "cals" }, fields: ["title", "updated"], sort: newest, window: { size: 60, count: "exact" },
      row: { title: { field: "title" }, icon: { avatar: { label: "title" } }, children: { bind: "cal-threads" } }, states: { empty: { title: "No Cals yet", detail: "Save an assistant as a Cal to see its threads here." } } },
    { id: "cal-threads", source: { type: "list", kind: "threads", parent: true }, fields, filter: { all: [notArchived] }, sort: newest, window: { size: 30, count: "exact" }, row: rowTemplate("cal-threads") },
    list("harnesses", [notArchived, unpinned, eq("shape", "agent"), eq("via_cal", false)], { title: "No harness threads", detail: "A thread with a plain agent appears here." }),
    { ...list("archived", [eq("archived", true)], { title: "Nothing archived", detail: "A thread comes back to the list when someone writes in it." }),
      row: { ...rowTemplate("archived"), leadingActions: undefined, trailingActions: [{ id: "unarchive", title: "Unarchive", icon: "check", tone: "good", operation: "host.item.unarchive", inputs: { item: row } }] } },
    { id: "thread", source: { type: "item", kind: "threads", ref: item }, fields: ["title", "updated", "shape", "muted", "pinned"] },
    { id: "found", source: { type: "search", kind: "threads" }, fields: ["title", "updated", "preview"], window: { size: 30, count: "none" },
      row: { title: { field: "title" }, open: { route: { name: "item", value: "id" } }, icon: { kindIcon: true } }, states: { empty: { title: "No thread matches" } } },
  ],
  chrome: {
    // DRAFT sidebar shapes (issue #78): sections with subsections and a pinned strip, all of it arranged by the person (reorder, collapse, hide, rename, folders), stored for them by the host.
    sidebar: { open: true, title: "Threads", refresh: true, width: 280,
      sections: [
        { id: "shortcuts", rows: [{ id: "new-thread", title: "New thread", icon: "plus", open: { invoke: { id: "create-thread", title: "New thread", operation: "threads.create", inputs: { title: { prompt: { title: "New thread", placeholder: "What is it about?" } } }, then: { route: { name: "item" } } } } }] },
        // ONE pinned strip holds every pin (the owner may want one per kind later); then plain sections in the default order. A folder the person makes is just another section.
        pins("pins", "pinned"),
        { id: "groups", title: "Groups", bind: "groups" },
        { id: "people", title: "People", bind: "contacts" },
        { id: "cals", title: "Cals", bind: "cals" },
        { id: "harnesses", title: "Harnesses", bind: "harnesses" },
        { id: "archive", title: "Archived", collapsed: true, bind: "archived", customise: { hide: false, rename: false } },
      ] },
    header: { title: { bind: "thread", field: "title" }, trailing: [callIn("call-in", "Call an agent in", { literal: "all" })] },
    footer: { account: { id: "account", title: "Account", menu: [{ id: "signout", title: "Sign out", role: "destructive", shell: { signOut: true } }] },
      apps: [{ id: "threads", title: "Threads", icon: "threads" }, { id: "notes", title: "Notes", icon: "notes" }, { id: "settings", title: "Settings", icon: "settings" }],
      search: { placeholder: "Search threads", bind: "found" } },
    panel: { id: "info", title: "Thread", open: true, content: [
      { type: "detail", bind: "thread", fields: [{ label: "Last activity", field: "updated" }] },
      { type: "button", invoke: mute("mute-thread", "Mute this thread", { mute: muteFor }) },
      { type: "button", invoke: { ...pin("pin-thread", true), inputs: { item, pinned: { literal: true } } } },
    ] },
  },
  view: { root: { type: "stack", children: [
    { type: "component", component: { type: "item-view", id: "transcript", config: {
      item, slot: "body", selectionMode: "range", reading: { unread: "unread", mentions: "mentions" },
      entry_actions: [
        { invoke: { id: "reply", title: "Reply", icon: "quote", operation: "threads.reply", inputs: { item, entry }, then: { composer: { form: "composer", mode: "reply" } } } },
        { invoke: { id: "edit", title: "Edit", icon: "edit", operation: "threads.edit", inputs: { item, entry }, then: { composer: { form: "composer", mode: "edit" } } }, own_only: true, max_age_seconds: 172800 },
        { invoke: { id: "delete", title: "Delete for everyone", icon: "trash", role: "destructive", operation: "threads.delete", inputs: { item, entry } }, own_only: true },
        { invoke: callIn("call-from-here", "Call an agent in from here", { literal: "from" }, { from: entry }) },
        { invoke: callIn("call-range", "Call an agent in on these", { literal: "range" }, { selection: { entry: "selection" } }) },
      ] } } },
    { type: "form", id: "composer", entry_context: { component: "transcript" },
      fields: [
        { id: "message", label: "Message", input: "multiline", required: true, maxLength: 4000, placeholder: "Message", initial: { entry: "text" }, draft: { slot: "message" }, mentions: "item_participants" },
        { id: "files", label: "Add photos and files", input: "file" },
        { id: "ask", label: "Ask an agent", input: "toggle" },
      ],
      submit: { id: "send", title: "Send", icon: "send", operation: "threads.send", inputs: {
        item, text: { form: "composer", field: "message" }, attachments: { form: "composer", field: "files" }, reply: { entry: "composer" }, harness: harness(ask), placement: place(ask) } } },
  ] } },
};
