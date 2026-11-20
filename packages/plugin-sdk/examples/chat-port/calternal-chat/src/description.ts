import type { AppDescription, Filter, NonSwipeInvoke, Predicate, RowTemplate, Sort } from "@calternal/plugin-sdk/types";

// The Threads interface as DATA (docs/description-contract.md, 14.43): the host fills every bind under the viewer's rights and each platform draws the result natively. No viewer data, no HTML, no plugin web code.
// This is what the manifest declares TODAY: the viewer-private fields (`preview unread mentions muted pinned draft`, `shape`), `chat.state`, the composer's draft slot, reading counts, Send with the agent and placement
// choices. What it does not yet declare (reply, edit, delete, calling an agent in on a message or range, attachments, mentions, timed mute) is in `description-next.ts`, which joins this file as each operation is
// declared. Design note and findings: Forgejo issue #99.
const item = { route: "item" } as const;
const row = { row: "id" } as const;
const eq = (field: string, value: boolean | string): Predicate => ({ field, op: "eq", value: { literal: value } });
const newest: Sort = [{ field: "updated", direction: "desc" }];
const fields = ["title", "updated", "preview", "unread", "mentions", "muted", "pinned", "shape"];
const state = (id: string, title: string, icon: string, inputs: Record<string, unknown>, ctx: "row" | "item" = "row"): NonSwipeInvoke =>
  ({ id, title, icon, operation: "chat.state", inputs: { item: ctx === "row" ? row : item, ...inputs } }) as NonSwipeInvoke;

/** A thread row is one line: shape icon, title, then the time and the counts. */
const rowTemplate = (n: string): RowTemplate => ({
  title: { field: "title", fallback: "Untitled chat" },
  icon: { field: "shape", name: "chat", map: { person: "user", group: "user", agent: "spark", mixed: "chat" } },
  // One line: the title, then at its end the time and the counts (a muted chat keeps only its mention count, in the accent: it never rings).
  marks: [
    { kind: "text", field: "updated" },
    { kind: "count", field: "mentions", when: eq("muted", true) },
    { kind: "count", field: "unread", when: eq("muted", false) },
  ],
  open: { route: { name: "item", value: "id" } },
  leadingActions: [state(`pin-${n}`, "Pin", "spark", { pinned: { literal: true } }), state(`unread-mark-${n}`, "Mark unread", "check", { unread_mark: { literal: true } })],
  trailingActions: [{ id: `archive-${n}`, title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item: row } }, state(`mute-${n}`, "Mute", "bell", { muted: { literal: true } })],
  menu: [
    state(`unpin-${n}`, "Unpin", "spark", { pinned: { literal: false } }), state(`unmute-${n}`, "Unmute", "bell", { muted: { literal: false } }),
    { id: `rename-${n}`, title: "Rename", icon: "edit", operation: "host.item.rename", inputs: { item: row, title: { prompt: { title: "Rename chat", initial: { row: "field", field: "title" } } } } },
    { id: `trash-${n}`, title: "Move to Trash", icon: "trash", role: "destructive", operation: "host.item.trash", inputs: { item: row } },
  ],
});
const list = (id: string, filter: Filter["all"], empty: { title: string; detail?: string }) => ({
  id, source: { type: "list", kind: "chats" }, fields, filter: { all: filter }, sort: [{ field: "pinned", direction: "desc" }, ...newest], window: { size: 60, count: "exact" },
  row: rowTemplate(id), states: { empty, error: { title: "Chats could not load", action: { id: `retry-${id}`, title: "Try again", shell: { reload: true } } } },
}) as NonNullable<AppDescription["binds"]>[number];
const none = { title: "Nothing here", detail: "Chats that match come back by themselves." };

/** The Threads app description: sidebar (selector with the archive count, new thread, the lists, search, account), header, the conversation with its reading counts, and the composer with its agent and placement choices. */
export const description: AppDescription = {
  version: 2, kind: "app", revision: 3, cache: { templates: "keep" },
  binds: [
    list("all", [eq("archived", false)], { title: "No chats yet", detail: "Start one with New chat: a conversation with an agent or with people." }),
    list("people", [eq("archived", false), { field: "shape", op: "in", value: { literal: ["person", "group"] } }], none),
    list("agents", [eq("archived", false), eq("shape", "agent")], none),
    { id: "archived", source: { type: "list", kind: "chats" }, fields, filter: { all: [eq("archived", true)] }, sort: newest, window: { size: 60, count: "exact" },
      row: { ...rowTemplate("archived"), leadingActions: undefined, trailingActions: [{ id: "unarchive", title: "Unarchive", icon: "check", tone: "good", operation: "host.item.unarchive", inputs: { item: row } }] },
      states: { empty: { title: "Nothing archived", detail: "A chat comes back to the list when someone writes in it." } } },
    { id: "archived-count", source: { type: "list", kind: "chats" }, fields: [], filter: { all: [eq("archived", true)] }, window: { size: 1, count: "exact" } },
    { id: "chat", source: { type: "item", kind: "chats", ref: item }, fields: ["title", "updated", "shape", "muted", "pinned"] },
    { id: "found", source: { type: "search", kind: "chats" }, fields: ["title", "updated", "preview"], window: { size: 30, count: "none" },
      row: { title: { field: "title" }, open: { route: { name: "item", value: "id" } }, icon: { kindIcon: true } },
      states: { empty: { title: "No chat matches" } } },
  ],
  chrome: {
    sidebar: { open: true, title: "Chat", refresh: true, width: 280,
      selectors: [{ id: "view", route: "view", selected: "all", options: [{ id: "all", title: "All" }, { id: "people", title: "People" }, { id: "agents", title: "Agents" }, { id: "archived", title: "Archived", count: { bind: "archived-count" } }] }],
      sections: [
        { id: "shortcuts", rows: [{ id: "new-chat", title: "New chat", icon: "plus", open: { invoke: { id: "create-chat", title: "New chat", operation: "chat.create", inputs: { title: { prompt: { title: "New chat", placeholder: "What is it about?" } } }, then: { route: { name: "item" } } } } }] },
        { id: "all", bind: "all", showWhen: { route: "view", equals: "all" } },
        { id: "people", bind: "people", showWhen: { route: "view", equals: "people" } },
        { id: "agents", bind: "agents", showWhen: { route: "view", equals: "agents" } },
        { id: "archived", bind: "archived", showWhen: { route: "view", equals: "archived" } },
      ] },
    header: { title: { bind: "chat", field: "title" }, trailing: [{ id: "archive-open", title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item } }] },
    footer: { account: { id: "account", title: "Account", menu: [{ id: "signout", title: "Sign out", role: "destructive", shell: { signOut: true } }] },
      apps: [{ id: "chat", title: "Chat", icon: "chat" }, { id: "notes", title: "Notes", icon: "notes" }, { id: "settings", title: "Settings", icon: "settings" }],
      search: { placeholder: "Search chats", bind: "found" } },
    panel: { id: "info", title: "Chat", open: true, content: [
      { type: "detail", bind: "chat", fields: [{ label: "Last activity", field: "updated" }] },
      { type: "button", invoke: state("mute-chat", "Mute this chat", "bell", { muted: { literal: true } }, "item") },
      { type: "button", invoke: state("pin-chat", "Pin", "spark", { pinned: { literal: true } }, "item") },
    ] },
  },
  view: { root: { type: "stack", children: [
    { type: "component", component: { type: "item-view", id: "transcript", config: { item, slot: "body", reading: { unread: "unread", mentions: "mentions" } } } },
    { type: "form", id: "composer", entry_context: { component: "transcript" },
      fields: [{ id: "message", label: "Message", input: "multiline", required: true, maxLength: 4000, placeholder: "Message", draft: { slot: "draft" } }],
      submit: { id: "send", title: "Send", icon: "send", operation: "chat.send", inputs: {
        item, text: { form: "composer", field: "message" },
        harness: { prompt: { title: "Agent", kind: "choice", options: { extension: "harnesses" } } },
        placement: { prompt: { title: "Run on", kind: "choice", options: { host: "execution_placement" } } },
      } } },
  ] } },
};
