/**
 * The spaces the person is in, and the one the sidebar is showing. Everyone has a personal space from sign-up (the first one here); a person may be in others. The sidebar lists only the current
 * space's threads and notes; the palette searches every space; addresses never name a space (items are addressed by ID). Choosing a space only changes what the sidebar lists: the item open in
 * the middle card stays open until the person goes elsewhere, and opening an item of another space (from the palette, a link) makes that space current.
 * Fixtures: two spaces. The real list comes from the server (`spaces.list`); `setList` takes it.
 */
import { server, serverMode } from "../server/client";

const KEY = "cal.space";

/** A space: a UUID and a name the person sees. */
export interface SpaceInfo { id: string; name: string; /** The person's role in it (`owner`, `manager`, `editor`, `reader`), from the server. */ role?: string }

/** The fixture spaces. The first is the personal space everyone has. Ids are UUIDs; a name never stands in for one. */
export const FIXTURE_SPACES: SpaceInfo[] = [
  { id: "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11", name: "Personal" },
  { id: "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b22", name: "Infra" },
];

/** The fixture spaces; `?spaces=N` (fixtures only) shows exactly N, generating "Space k" beyond the two named ones, for checking the account menu's space list at 1, 3, 4 and 50. */
function fixtureSpaces(): SpaceInfo[] {
  const n = typeof location === "undefined" || serverMode() ? NaN : Number(new URLSearchParams(location.search).get("spaces"));
  if (!Number.isFinite(n) || n < 1) return FIXTURE_SPACES;
  const extra = Array.from({ length: Math.max(0, Math.floor(n) - FIXTURE_SPACES.length) }, (_, i) => ({ id: `0197f3a2-5c1e-7b4d-9a60-3d2e8f1c${String(i + 100).padStart(4, "0")}`, name: `Space ${i + 3}` }));
  return [...FIXTURE_SPACES, ...extra].slice(0, Math.min(200, Math.floor(n)));
}

class Spaces {
  list = $state<SpaceInfo[]>(fixtureSpaces());
  currentId = $state(read());
  /** The current space (the first when the remembered one is gone). */
  get current(): SpaceInfo { return this.list.find((s) => s.id === this.currentId) ?? this.list[0]!; }
  /** The personal space. */
  get personal(): SpaceInfo { return this.list[0]!; }
  /** Makes a space current and remembers it. */
  switchTo(id: string): void {
    if (id === this.currentId || !this.list.some((s) => s.id === id)) return;
    this.currentId = id;
    try { localStorage.setItem(KEY, id); } catch { /* private mode: for this page only */ }
  }
  /** Replaces the list (the server's), keeping the current space when it is still there. */
  setList(l: SpaceInfo[]): void { if (l.length) this.list = l; }
  /**
   * The current space's id once the server's list is in. In server mode `current` is a fixture placeholder until `load()` has run, and no operation may carry a fixture id (the server answers
   * `not_found` for it): code that can run before the list is loaded (a screen right after sign-up) takes the id from here. In fixtures mode it resolves at once.
   */
  async whenCurrent(): Promise<string> { await this.load(); return this.current.id; }
  private loading: Promise<void> | null = null;
  /**
   * Reads the signed-in person's spaces from the real server (`spaces.list`; fixtures stay when it is not selected). The personal space is the one the server marks (`personal: true`) and comes first in the list. A failure keeps what is shown and is tried again on the next call. One request; later callers share it.
   */
  load(): Promise<void> {
    if (!serverMode()) return Promise.resolve();
    this.loading ??= (async () => {
      const { spaces: all } = await server.spaces.list();
      const byAge = [...all].sort((a, b) => (a.created < b.created ? -1 : a.created > b.created ? 1 : 0));
      const personal = byAge.find((s) => s.personal);
      this.setList((personal ? [personal, ...byAge.filter((s) => s !== personal)] : byAge).map((s) => ({ id: s.id, name: s.name, role: s.role })));
    })().catch((e) => { this.loading = null; throw e; });
    return this.loading;
  }
}

function read(): string {
  try { return localStorage.getItem(KEY) ?? FIXTURE_SPACES[0]!.id; } catch { return FIXTURE_SPACES[0]!.id; }
}

/** The shared spaces state. */
export const spaces = new Spaces();
