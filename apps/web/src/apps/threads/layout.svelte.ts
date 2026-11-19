/**
 * The person's own arrangement of the thread list: sections they create, name, collapse and delete, and the order of threads inside them, dragged
 * freely between and within (or moved with the keyboard). There are no automatic groups. Threads that are in no section sit in a list at the top
 * with no label. Child threads always follow their parent, so only top-level threads are placed here.
 * Stored in localStorage for now; the server will keep it per account (and per space).
 */

/** One section of the list. */
export interface Section {
  /** Unique id of the section (a UUID made here); the key the list and drag-and-drop use. */
  id: string;
  /** The person's name for it; may be empty while it is being renamed. */
  name: string;
  /** Whether its threads are hidden; the section header stays. */
  collapsed: boolean;
  /** Top-level thread ids, in the order shown. */
  threadIds: string[];
}

/** Where a thread can be dropped: a section id, or null for the unlabelled list at the top. */
export type Container = string | null;

interface Stored {
  sections: Section[];
  loose: string[];
  /** The name and folded state of the section that holds threads in no section of their own ("Threads" until renamed). */
  looseName?: string;
  looseCollapsed?: boolean;
}

import { floodOf } from "../../lib/flood";

const KEY = "cal.threads.layout";

function read(): Stored {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as Stored | null;
    if (s && Array.isArray(s.sections) && Array.isArray(s.loose)) return s;
  } catch {
    /* first run */
  }
  const fl = floodOf();
  if (fl) return floodLayout(fl.count);
  return { sections: [], loose: [] };
}

/** The flood's arrangement (`?flood`): five named sections holding the first threads in turn, the rest loose. */
function floodLayout(n: number): Stored {
  const names = ["Now", "This week", "Mesh", "Notes plugin", "Parked"], sections: Section[] = names.map((name, k) => ({ id: `fl-sec-${k}`, name, collapsed: false, threadIds: [] }));
  for (let i = 0; i < Math.min(n, 400); i++) sections[i % sections.length]!.threadIds.push(`t-fl-${i}`);
  return { sections, loose: [] };
}

class Layout {
  sections = $state<Section[]>(read().sections);
  /** Order of the unlabelled list; threads not mentioned are appended by recency. */
  loose = $state<string[]>(read().loose);
  /** The unlabelled list is a section too: it sits first, named "Threads" until renamed, and folds like the others. */
  looseName = $state<string>(read().looseName || "Threads");
  looseCollapsed = $state<boolean>(!!read().looseCollapsed);

  private save(): void {
    try {
      localStorage.setItem(KEY, JSON.stringify({ sections: this.sections, loose: this.loose, looseName: this.looseName, looseCollapsed: this.looseCollapsed } satisfies Stored));
    } catch {
      /* private mode */
    }
  }

  /** Which container a thread is in. */
  containerOf(id: string): Container {
    return this.sections.find((s) => s.threadIds.includes(id))?.id ?? null;
  }

  /** The ordered ids of a container, restricted to `existing` ids (threads gone from the space drop out), with unplaced threads (loose only) appended in the given order. */
  idsOf(c: Container, existing: string[]): string[] {
    const have = new Set(existing);
    if (c === null) {
      const placed = new Set(this.sections.flatMap((s) => s.threadIds));
      const order = this.loose.filter((i) => have.has(i) && !placed.has(i));
      const ordered = new Set(order);
      return [...order, ...existing.filter((i) => !ordered.has(i) && !placed.has(i))];
    }
    return (this.sections.find((s) => s.id === c)?.threadIds ?? []).filter((i) => have.has(i));
  }

  /** Creates a section at the end; returns it. */
  create(name: string): Section {
    const s: Section = { id: `s-${Date.now().toString(36)}${Math.floor(Math.random() * 1e3)}`, name: name.trim() || "New section", collapsed: false, threadIds: [] };
    this.sections = [...this.sections, s];
    this.save();
    return s;
  }
  /** Renames a section. */
  rename(id: string, name: string): void {
    const s = this.sections.find((x) => x.id === id);
    if (s && name.trim()) s.name = name.trim();
    this.save();
  }
  /** Renames the section of threads that are in no section of their own. */
  renameLoose(name: string): void {
    if (name.trim()) this.looseName = name.trim();
    this.save();
  }
  /** Folds or unfolds that section. */
  toggleLoose(): void {
    this.looseCollapsed = !this.looseCollapsed;
    this.save();
  }
  /** Collapses or expands a section. */
  toggle(id: string): void {
    const s = this.sections.find((x) => x.id === id);
    if (s) s.collapsed = !s.collapsed;
    this.save();
  }
  /** Deletes a section; its threads go back to the unlabelled list. Returns the undo: it puts the section back where it was with the same threads (false if it is already back). */
  remove(id: string): (() => boolean) | null {
    const at = this.sections.findIndex((x) => x.id === id);
    if (at < 0) return null;
    const s = { ...this.sections[at], threadIds: [...this.sections[at].threadIds] };
    this.loose = [...s.threadIds, ...this.loose];
    this.sections = this.sections.filter((x) => x.id !== id);
    this.save();
    return () => {
      if (this.sections.some((x) => x.id === id)) return false;
      this.loose = this.loose.filter((i) => !s.threadIds.includes(i));
      this.sections = [...this.sections.slice(0, at), s, ...this.sections.slice(at)];
      this.save();
      return true;
    };
  }
  /** Moves a section up or down among the sections. */
  shift(id: string, by: -1 | 1): void {
    const i = this.sections.findIndex((x) => x.id === id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= this.sections.length) return;
    const next = [...this.sections];
    [next[i], next[j]] = [next[j]!, next[i]!];
    this.sections = next;
    this.save();
  }

  /**
   * Puts a thread in a container at an index (0 is first), taking it out of wherever it was. `visible` is the container's currently shown ids, so the
   * index counts what the person sees.
   */
  move(id: string, to: Container, index: number, visible: string[]): void {
    for (const s of this.sections) s.threadIds = s.threadIds.filter((i) => i !== id);
    this.loose = this.loose.filter((i) => i !== id);
    const list = visible.filter((i) => i !== id);
    list.splice(Math.max(0, Math.min(index, list.length)), 0, id);
    if (to === null) this.loose = list;
    else {
      const s = this.sections.find((x) => x.id === to);
      if (s) {
        s.threadIds = list;
        s.collapsed = false;
      }
    }
    this.save();
  }
}

/** The shared layout. */
export const layout = new Layout();
