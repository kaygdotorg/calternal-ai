import type { AppDescription, Filter, NonSwipeAction, RowTemplate, Sort } from "../generated/wire.js";

// The Threads interface as DATA (docs/description-contract.md, 14.41): the host fills every bind under the viewer's rights and each platform
// draws the result natively (Svelte in the browser, SwiftUI on Apple platforms). No viewer data, no HTML, no plugin web code. Design note and the
// list of what this contract cannot say yet: Forgejo issue #99. A screen part that needs one of those findings (per-person unread and mute marks,
// the message node, quotes, separators, the right panel) is deliberately absent here, not drawn another way.
const item = { route: "item" } as const;
const row = { row: "id" } as const;
const notArchived: Filter = { all: [{ field: "archived", op: "eq", value: { literal: false } }] };
const archivedOnly: Filter = { all: [{ field: "archived", op: "eq", value: { literal: true } }] };
const newest: Sort = [{ field: "updated", direction: "desc" }];
const open = { route: { name: "item", value: "id" } } as const;
const rename: NonSwipeAction = {
  id: "rename", title: "Rename", icon: "edit", operation: "host.item.rename",
  inputs: { item: row, title: { prompt: { title: "Rename thread", initial: { row: "field", field: "title" } } } },
};

/** What a thread row says: the title, then when it last changed. */
const rowTemplate: RowTemplate = {
  title: { field: "title", fallback: "Untitled thread" },
  secondary: [{ field: "updated", format: "relative_date" }],
  icon: { kindIcon: true },
  open,
};

/** The Threads app description: sidebar (selector, new thread, list, search, account), header, and the card (the conversation and its composer). */
export const description: AppDescription = {
  version: 2, kind: "app", revision: 2,
  cache: { templates: "keep" },
  binds: [
    { id: "threads", source: { type: "list", kind: "threads" }, fields: ["title", "updated"], filter: notArchived, sort: newest,
      window: { size: 60, count: "exact" },
      row: { ...rowTemplate,
        trailingActions: [{ id: "archive", title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item: row } }],
        menu: [rename,
          { id: "archive-menu", title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item: row } },
          { id: "trash", title: "Move to Trash", icon: "trash", role: "destructive", operation: "host.item.trash", inputs: { item: row } }] },
      states: { empty: { title: "No threads yet", detail: "Start one with New thread: a conversation with an agent or with people." },
        error: { title: "Threads could not load", action: { id: "retry", title: "Try again", shell: { reload: true } } } } },
    { id: "archived", source: { type: "list", kind: "threads" }, fields: ["title", "updated"], filter: archivedOnly, sort: newest,
      window: { size: 60, count: "exact" },
      row: { ...rowTemplate,
        trailingActions: [{ id: "unarchive", title: "Unarchive", icon: "check", tone: "good", operation: "host.item.unarchive", inputs: { item: row } }] },
      states: { empty: { title: "Nothing archived", detail: "Archived threads stay here, and come back to the list when someone writes in them." } } },
    { id: "archived-count", source: { type: "list", kind: "threads" }, fields: [], filter: archivedOnly, window: { size: 1, count: "exact" } },
    { id: "thread", source: { type: "item", kind: "threads", ref: item }, fields: ["title", "updated"] },
    { id: "found", source: { type: "search", kind: "threads" }, fields: ["title", "updated"], window: { size: 30, count: "none" },
      row: { ...rowTemplate }, states: { empty: { title: "No thread matches" } } },
  ],
  chrome: {
    sidebar: {
      open: true, title: "Threads", refresh: true,
      selectors: [{ id: "view", route: "view", selected: "threads", options: [
        { id: "threads", title: "Threads" },
        { id: "archived", title: "Archived", count: { bind: "archived-count" } }] }],
      sections: [
        { id: "shortcuts", rows: [{ id: "new-thread", title: "New thread", icon: "plus", open: { invoke: {
          id: "create-thread", title: "New thread", operation: "threads.create",
          inputs: { title: { prompt: { title: "New thread", placeholder: "What is it about?" } } }, then: { route: { name: "item" } } } } }] },
        { id: "threads", bind: "threads", showWhen: { route: "view", equals: "threads" } },
        { id: "archived", title: "Archived", bind: "archived", showWhen: { route: "view", equals: "archived" } },
      ],
    },
    header: { title: { bind: "thread", field: "title" },
      trailing: [{ id: "archive-open", title: "Archive", icon: "archive", operation: "host.item.archive", inputs: { item } }] },
    footer: {
      account: { id: "account", title: "Account", menu: [{ id: "signout", title: "Sign out", role: "destructive", shell: { signOut: true } }] },
      apps: [{ id: "threads", title: "Threads", icon: "threads" }, { id: "notes", title: "Notes", icon: "notes" }, { id: "settings", title: "Settings", icon: "settings" }],
      search: { placeholder: "Search threads", bind: "found" },
    },
  },
  view: { root: { type: "stack", children: [
    { type: "component", component: { type: "item-view", id: "transcript", config: { item, slot: "body" } } },
    { type: "form", id: "composer", fields: [{ id: "message", label: "Message", input: "multiline", required: true, maxLength: 4000, placeholder: "Message" }],
      submit: { id: "send", title: "Send", icon: "send", operation: "threads.send", inputs: {
        item, text: { form: "composer", field: "message" },
        harness: { prompt: { title: "Agent", kind: "choice", options: { extension: "harnesses" } } },
        placement: { prompt: { title: "Run on", kind: "choice", options: { host: "execution_placement" } } },
      } } },
  ] } },
};
