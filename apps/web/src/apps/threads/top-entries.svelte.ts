/**
 * The fixed top section of the Threads sidebar: a short list of entries (New thread, Automations, Skills by default) the person can add to, remove from, rename and reorder. It is the person's own arrangement,
 * so it is a file per account in the file-over-app pattern of the shortcuts (docs/composer.md) and the settings (docs/settings.md): `Account/threads-sidebar.md` in the account's userdata, Markdown with
 * `key: value` lines, readable and editable by hand. Until the server stores such files (no operation for it yet) this page keeps the same text in the browser (`cal.threads.top`), so moving it later is a copy.
 *
 * The file:
 *
 *     ---
 *     kind: threads-sidebar-top
 *     ---
 *     - new-thread: New thread
 *     - route:/threads/automations: Automations
 *     - thread:019a8c00-0000-7000-8000-000000000001: Release checklist
 *
 * A line is `- <target>: <label>`; the target is `new-thread` (starts a thread), `route:<path>` (opens a screen of the app) or `thread:<id>` (opens a thread). An unreadable file gives the default list.
 */
import type { IconName } from "@calternal/ui";

/** One entry of the top section. */
export interface TopEntry {
  /** Unique in the list (made when the entry is read or added). */
  id: string;
  /** What it does: `new-thread`, `route:<path>` or `thread:<id>`. */
  target: string;
  /** What the person calls it. */
  label: string;
}

const KEY = "cal.threads.top";
/** The default list: the Threads app's own first actions. Automations and Skills open placeholder screens until their design round. */
export const DEFAULT_ENTRIES: Omit<TopEntry, "id">[] = [
  { target: "new-thread", label: "New thread" },
  { target: "route:/threads/automations", label: "Automations" },
  { target: "route:/threads/skills", label: "Skills" },
];

/** The icon of an entry, from what it does. */
export function iconOf(target: string): IconName {
  if (target === "new-thread") return "plus";
  if (target === "route:/threads/automations") return "clock";
  if (target === "route:/threads/skills") return "wrench";
  if (target.startsWith("thread:")) return "threads";
  if (target.startsWith("route:/notes")) return "notes";
  if (target.startsWith("route:/settings")) return "settings";
  return "list";
}

/** The file's text for these entries. */
export function toMarkdown(entries: readonly Omit<TopEntry, "id">[]): string {
  return `---\nkind: threads-sidebar-top\n---\n${entries.map((e) => `- ${e.target}: ${e.label.replace(/\n/g, " ")}`).join("\n")}\n`;
}

/** The entries in a file's text; the default list when it has none or is not this kind of file. Targets other than the three kinds are dropped. */
export function fromMarkdown(text: string | null | undefined): Omit<TopEntry, "id">[] {
  if (!text || !/^kind:\s*threads-sidebar-top\s*$/m.test(text)) return DEFAULT_ENTRIES.map((e) => ({ ...e }));
  const out: Omit<TopEntry, "id">[] = [];
  for (const line of text.split("\n")) {
    const m = /^- (new-thread|route:\/\S*|thread:\S+): (.+)$/.exec(line.trim());
    if (m) out.push({ target: m[1]!, label: m[2]!.trim().slice(0, 60) });
  }
  return out.length || /^- /m.test(text) ? out : DEFAULT_ENTRIES.map((e) => ({ ...e }));
}

class TopEntries {
  private n = 0;
  entries = $state<TopEntry[]>(this.read());
  private read(): TopEntry[] {
    let text: string | null = null;
    try { text = localStorage.getItem(KEY); } catch { /* storage is off: the defaults */ }
    return fromMarkdown(text).map((e) => ({ ...e, id: `e${++this.n}` }));
  }
  private save(): void {
    try { localStorage.setItem(KEY, toMarkdown(this.entries)); } catch { /* storage is off: the change lasts for this visit */ }
  }
  /** Adds an entry at the end (a target already in the list is not added twice). */
  add(target: string, label: string): void {
    if (this.entries.some((e) => e.target === target)) return;
    this.entries = [...this.entries, { id: `e${++this.n}`, target, label: label.trim().slice(0, 60) || "Entry" }];
    this.save();
  }
  /** Takes an entry out. */
  remove(id: string): void { this.entries = this.entries.filter((e) => e.id !== id); this.save(); }
  /** Renames an entry (an empty name changes nothing). */
  rename(id: string, label: string): void {
    const l = label.trim().slice(0, 60);
    if (!l) return;
    this.entries = this.entries.map((e) => (e.id === id ? { ...e, label: l } : e));
    this.save();
  }
  /** Moves an entry one place up (-1) or down (1). */
  move(id: string, by: -1 | 1): void {
    const i = this.entries.findIndex((e) => e.id === id), j = i + by;
    if (i < 0 || j < 0 || j >= this.entries.length) return;
    const next = [...this.entries]; [next[i], next[j]] = [next[j]!, next[i]!];
    this.entries = next; this.save();
  }
  /** Back to New thread, Automations and Skills. */
  reset(): void { this.entries = DEFAULT_ENTRIES.map((e) => ({ ...e, id: `e${++this.n}` })); this.save(); }
}

/** The one list of the page. */
export const top = new TopEntries();
