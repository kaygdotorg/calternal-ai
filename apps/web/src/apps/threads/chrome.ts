/**
 * Threads' side of the native chrome contract (shell/chrome): what the app publishes (the title of the open thread, the Inbox and All Threads choice, the shortcuts, the person's sections and their threads as rows with the
 * provider's mark and the state or reason marks, a context menu and swipe actions per row) and how it answers the shell's events by calling the same functions the drawn sidebar calls (`threadItems`/`runThreadItem`,
 * `archiveThread`, `layout.toggle`, `startThread`). Only top-level threads are listed natively (children are drawn under their parent in the page's own sidebar; the native list has no nesting yet). Pure of the DOM, so it is tested
 * with the stores alone.
 */
import { toast } from "@calternal/ui";
import { navigate, segments } from "../../lib/route.svelte";
import { spaces } from "../../shell/spaces.svelte";
import type { ChromeBadge, ChromeBody, ChromeEvent, ChromeItem, ChromeProvider, ChromeRow, ChromeSection } from "../../shell/chrome/types";
import { windowedOf } from "../../shell/chrome/native-chrome.svelte";
import { layout } from "./layout.svelte";
import type { ThreadSummary } from "./model";
import { client, startThread, store } from "./store.svelte";
import { archiveThread, copyThreadLink, runThreadItem, threadItems, topLevelIds } from "./thread-menu";
import { iconOf, top, type TopEntry } from "./top-entries.svelte";
import { view } from "./view.svelte";

const REASON: Record<string, ChromeBadge> = {
  Question: { kind: "icon", icon: "question", tone: "info" }, Approval: { kind: "icon", icon: "shield", tone: "info" }, Secret: { kind: "icon", icon: "key", tone: "info" },
  "Sign in": { kind: "icon", icon: "lock", tone: "info" }, Failed: { kind: "icon", icon: "warning", tone: "bad" }, Interrupted: { kind: "icon", icon: "stop", tone: "warn" },
};

const path = (e: TopEntry): string | null => (e.target.startsWith("route:") ? e.target.slice(6) : e.target.startsWith("thread:") ? `/threads/${e.target.slice(7)}` : null);
const asItem = (m: { id: string; label: string; icon?: string; danger?: boolean; input?: ChromeItem["input"] }): ChromeItem => ({ id: m.id, title: m.label, ...(m.icon ? { icon: m.icon } : {}), ...(m.danger ? { role: "destructive" as const } : {}), ...(m.input ? { input: m.input } : {}) });
const menuOf = (t: ThreadSummary): ChromeItem[] => threadItems(t).map((m) => asItem(m.id === "rename" ? { ...m, input: { placeholder: "Name", value: t.title } } : m));

/** The marks at a row's end: why it is in the Inbox (one per reason), else its state when it means something. */
function marksOf(t: ThreadSummary, reasons?: readonly string[]): ChromeBadge[] {
  if (reasons?.length) return reasons.map((r) => REASON[r] ?? { kind: "dot" });
  if (t.state === "running") return [{ kind: "icon", icon: "pulse", tone: "info" }];
  if (t.state === "waiting") return [{ kind: "icon", icon: "bell", tone: "info" }];
  if (t.state === "failed") return [REASON.Failed!];
  return [];
}

const rowOf = (t: ThreadSummary, current: string | undefined, reasons?: readonly string[]): ChromeRow => ({
  id: t.id, title: t.title, icon: t.provider, trailing: marksOf(t, reasons), selected: t.id === current,
  leadingActions: [{ id: "copy", title: "Copy link", icon: "link" }], trailingActions: [{ id: "archive", title: "Archive", icon: "archive", role: "destructive" }], menu: menuOf(t),
});

/** The Threads app's provider for the native chrome. */
export const threadsChrome: ChromeProvider = {
  describe(query: string): ChromeBody {
    const q = query.trim().toLowerCase();
    const match = (t: ThreadSummary) => !q || t.title.toLowerCase().includes(q);
    const here = spaces.current.id;
    const current = segments()[0] === "threads" ? segments()[1] : undefined;
    const open = current ? store.get(current) : undefined;
    const byId = new Map(store.threads.filter((t) => t.spaceId === here).map((t) => [t.id, t]));
    const inbox = store.inbox.filter((i) => i.thread.spaceId === here);
    const tab = view.chosen ?? (inbox.length || store.jobPrompts.length ? "inbox" : "all");
    const shortcuts: ChromeSection = {
      id: "top", title: null,
      // "Add to this list" is the last row of the shortcuts, as on the web: an untitled section has no header to hang an action on (it was drawn as an orphan plus under the selector).
      rows: [...top.entries.map((e) => ({ id: `top:${e.id}`, title: e.label, icon: iconOf(e.target), selected: path(e) !== null && `/${segments().join("/")}` === path(e) })), { id: "top-add", title: "Add to this list", icon: "plus" as const }],
    };
    const sections: ChromeSection[] = [shortcuts];
    if (tab === "inbox") {
      sections.push({ id: "inbox", title: null, ...windowedOf("inbox", inbox.filter((i) => match(i.thread)), (i) => rowOf(i.thread, current, i.reasons)) });
    } else {
      const top0 = topLevelIds();
      const rowsIn = (sectionId: string, ids: string[]) => windowedOf(sectionId, ids.map((id) => byId.get(id)).filter((t): t is ThreadSummary => !!t && match(t)), (t) => rowOf(t, current));
      sections.push({ id: "loose", title: layout.looseName, collapsed: layout.looseCollapsed, ...rowsIn("loose", layout.idsOf(null, top0)) });
      for (const s of layout.sections) sections.push({ id: s.id, title: s.name, collapsed: s.collapsed, ...rowsIn(s.id, layout.idsOf(s.id, top0)) });
    }
    return {
      header: { title: open?.title ?? "Threads", trailing: open ? [{ id: "more", title: "More", icon: "more", menu: menuOf(open) }] : [] },
      selectors: [{ id: "filter", selected: tab, options: [{ id: "inbox", title: "Inbox", ...(inbox.length ? { badge: { kind: "count" as const, value: inbox.length } } : {}) }, { id: "all", title: "All Threads" }] }],
      sections, search: { placeholder: "Search threads" },
    };
  },

  handle(ev: ChromeEvent): boolean {
    const thread = (id?: string) => (id ? store.get(id) : undefined);
    switch (ev.type) {
      case "row": {
        if (!ev.id) return true;
        if (ev.id.startsWith("top:")) {
          const e = top.entries.find((x) => x.id === ev.id!.slice(4));
          if (!e) return true;
          if (e.target === "new-thread") void startThread().then((t) => navigate(`/threads/${t.id}`), (err: Error) => toast(`Could not start a thread: ${err.message}`));
          else { const p = path(e); if (p) navigate(p); }
        } else if (ev.id === "top-add") toast("Add shortcuts from the sidebar on this device.");
        else if (store.get(ev.id)) navigate(`/threads/${ev.id}`);
        return true;
      }
      case "action": {
        const t = thread(ev.rowId);
        if (t && ev.id === "copy") copyThreadLink(t);
        else if (t && ev.id === "archive") void archiveThread(t).catch((e: Error) => toast(`Not archived: ${e.message}`));
        else if (ev.rowId === "top" && ev.id === "add") toast("Add shortcuts from the sidebar on this device.");
        return true;
      }
      case "menu": {
        const t = thread(ev.rowId);
        if (!t || !ev.id) return false;
        if (ev.id === "rename") { const name = (ev.text ?? "").trim(); if (name && name !== t.title) void client.rename(t.id, name).catch((e: Error) => toast(`Not renamed: ${e.message}`)); }
        else runThreadItem(ev.id, t, () => {});
        return true;
      }
      case "header": {
        const open = thread(segments()[1]);
        if (open && ev.id && ev.id !== "more") threadsChrome.handle({ ...ev, type: "menu", rowId: open.id });
        return true;
      }
      case "selector": if (ev.id === "filter" && (ev.option === "inbox" || ev.option === "all")) view.chosen = ev.option; return true;
      case "section": if (ev.id === "loose") layout.toggleLoose(); else if (ev.id) layout.toggle(ev.id); return true;
      case "refresh": void store.refresh(); return true;
      default: return false;
    }
  },
};
