/**
 * Spaces and the sharing of a sidebar (fixture only; nothing is sent anywhere).
 *  - A person's arrangement of a sidebar is a quiet personal copy: dragging never changes anyone else's. It is "Customized" while it differs from the space's sidebar.
 *  - An Owner or Manager can make the arrangement the space's sidebar; people who arranged their own keep theirs, the rest get the new one with a notice.
 *  - "Reset to the space's sidebar" puts the space's sidebar back (and can be undone).
 * The space's sidebar per sidebar is kept here as a snapshot of the sections, captured the first time the sidebar is looked at (the stand-in for the file the space shares).
 */
import { arr, type SecRef } from "./arrange.svelte";
import { SPACES, FIRST_SPACE } from "./data/apps";

export interface Space { id: string; name: string; members: number; /** Whether agents may be started in this space (one switch; the scope is chosen per run). */ agents?: boolean }
const clone = (l: SecRef[]) => l.map((x) => ({ ...x }));

class Share {
  spaces = $state<Space[]>(SPACES.map((s) => ({ ...s })));
  current = $state(FIRST_SPACE);
  private base = $state<Record<string, SecRef[]>>({});
  get space(): Space { return this.spaces.find((s) => s.id === this.current) ?? this.spaces[0]!; }
  /** Remembers the space's sidebar the first time a sidebar is seen (untouched by dragging after that). */
  ensure(sid: string) { if (!this.base[sid]) this.base[sid] = clone(arr.sections(sid)); }
  /** True while this person's arrangement differs from the space's sidebar. */
  customized(sid: string): boolean { const b = this.base[sid]; return !!b && JSON.stringify(b) !== JSON.stringify(arr.sections(sid)); }
  /** The space's sidebar, as the sections it lists (for the preview in the sheet). */
  shared(sid: string): SecRef[] { return this.base[sid] ?? arr.sections(sid); }
  /** Owner or Manager: makes the current arrangement the space's sidebar. */
  makeShared(sid: string) { this.base[sid] = clone(arr.sections(sid)); }
  /** Puts the space's sidebar back; returns what was there, to undo with. */
  reset(sid: string): SecRef[] { const prev = clone(arr.sections(sid)); arr.setSections(sid, this.base[sid] ?? prev); return prev; }
  restore(sid: string, list: SecRef[]) { arr.setSections(sid, list); }
  /** A new space the person made; they are its Owner. */
  create(name: string, members: number, agents: boolean): Space { const s: Space = { id: `s-${Date.now().toString(36)}`, name: name.trim() || "New space", members, agents }; this.spaces = [...this.spaces, s]; this.current = s.id; return s; }
}
export const share = new Share();
