/**
 * Notes' side of the native chrome contract (shell/chrome): the space's notes (newest first, narrowed by the search text in the native field), the person's collections and what others shared with them as sections of rows, a New note
 * and a Trash shortcut, a context menu and a swipe action (Move to Trash) per note; and how it answers the shell's events by calling what the drawn sidebar calls (`notes.create`, `notes.rename`, `notes.remove`, `navigate`).
 * Only the notes already loaded are listed (the next page is read when the list nears its end in the page's own sidebar; windowed lists in the native one come with the contract's window event). Pure of the DOM.
 */
import { toast } from "@calternal/ui";
import { navigate, route, segments } from "../../lib/route.svelte";
import type { ChromeBadge, ChromeBody, ChromeEvent, ChromeItem, ChromeProvider, ChromeRow, ChromeSection } from "../../shell/chrome/types";
import { collectionList } from "./share/collections-list.svelte";
import { incoming, incomingCollections } from "./share/incoming.svelte";
import { openShare } from "./share/share.svelte";
import { notes } from "./store.svelte";

const DOT: ChromeBadge[] = [{ kind: "dot", tone: "info" }];
const noteMenu = (title: string): ChromeItem[] => [{ id: "rename", title: "Rename", icon: "edit", input: { placeholder: "Title", value: title } }, { id: "trash", title: "Move to Trash", icon: "trash", role: "destructive" }];

/** The Notes app's provider for the native chrome. */
export const notesChrome: ChromeProvider = {
  describe(query: string): ChromeBody {
    const q = query.trim().toLowerCase();
    const match = (title: string) => !q || title.toLowerCase().includes(q);
    const current = segments()[0] === "notes" ? segments()[1] : undefined;
    const noteRow = (id: string, title: string, over: Partial<ChromeRow> = {}): ChromeRow => ({ id, title, icon: "notes", selected: current === id, ...over });
    const sections: ChromeSection[] = [
      { id: "top", title: null, rows: [{ id: "new", title: "New note", icon: "plus" }, { id: "trash", title: "Trash", icon: "trash", selected: route.path.startsWith("/notes") && route.query.get("view") === "trash" }] },
      { id: "notes", title: notes.space?.name ?? "Notes", rows: notes.list.filter((n) => match(n.title)).map((n) => noteRow(n.id, n.title, { trailingActions: [{ id: "trash", title: "Move to Trash", icon: "trash", role: "destructive" }], menu: noteMenu(n.title) })) },
    ];
    const cols = collectionList.items.filter((c) => match(c.title));
    if (cols.length) sections.push({ id: "collections", title: "Collections", rows: cols.map((c) => ({ id: `c:${c.id}`, title: c.title, icon: "folder", selected: route.path === `/collections/${c.id}` })) });
    const shared = [...incoming.items.filter((s) => match(s.title)).map((s) => noteRow(s.item.id, s.title, { trailing: s.unread ? DOT : [], hint: "Shared with you" })), ...incomingCollections.items.filter((s) => match(s.title)).map((s) => ({ id: `c:${s.item.id}`, title: s.title, icon: "folder", selected: route.path === `/collections/${s.item.id}`, trailing: s.unread ? DOT : [] }))];
    if (shared.length) sections.push({ id: "shared", title: "Shared with me", rows: shared });
    const open = notes.note && current === notes.note.id ? notes.note : undefined;
    return { header: { title: current ? notes.list.find((n) => n.id === current)?.title ?? incoming.of(current)?.title ?? "Notes" : "Notes", trailing: open ? [{ id: "share", title: "Share", icon: "share" }] : [] }, selectors: [], sections, search: { placeholder: "Find a note" } };
  },

  handle(ev: ChromeEvent): boolean {
    const note = (id?: string) => (id ? notes.list.find((n) => n.id === id) : undefined);
    switch (ev.type) {
      case "row": {
        if (!ev.id) return true;
        if (ev.id === "new") void notes.create("Untitled").then((n) => { if (n) navigate(`/notes/${n.id}`); }, (e: Error) => toast(`Could not make a note: ${e.message}`));
        else if (ev.id === "trash") navigate("/notes?view=trash");
        else if (ev.id.startsWith("c:")) navigate(`/collections/${ev.id.slice(2)}`);
        else navigate(`/notes/${ev.id}`);
        return true;
      }
      case "action": case "menu": {
        const n = note(ev.rowId);
        if (!n || !ev.id) return false;
        if (ev.id === "rename") { const t = (ev.text ?? "").trim(); if (t && t !== n.title) void notes.rename(n, t); }
        else if (ev.id === "trash") void notes.remove(n.id, n.title);
        return true;
      }
      case "header": if (ev.id === "share" && notes.note) openShare({ kind: "notes", id: notes.note.id }, notes.note.title, null); return true;
      case "refresh": void notes.refresh(); return true;
      default: return false;
    }
  },
};
