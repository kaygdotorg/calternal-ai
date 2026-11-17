/**
 * The spaces the person is in, and the one the sidebar is showing. Everyone has a personal space from sign-up (the first one here); a person may be in others. The sidebar lists only the current
 * space's threads and notes; the palette searches every space; addresses never name a space (items are addressed by ID). Choosing a space only changes what the sidebar lists: the item open in
 * the middle card stays open until the person goes elsewhere, and opening an item of another space (from the palette, a link) makes that space current.
 * Fixtures: two spaces. The real list comes from the server (`spaces.list`); `setList` takes it.
 */
import { server, serverMode } from "../server/client";

const KEY = "cal.space";

/** A space: a UUID and a name the person sees. */
export interface SpaceInfo { id: string; name: string }

/** The fixture spaces. The first is the personal space everyone has. Ids are UUIDs; a name never stands in for one. */
export const FIXTURE_SPACES: SpaceInfo[] = [
  { id: "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11", name: "Personal" },
  { id: "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b22", name: "Infra" },
];

/**
 * TEMPORARY: a new account has no space until the server creates the personal one itself at sign-up (announced). Until then the first load makes it ("Personal", one request, once). When the
 * server does it, delete this function and its one call in `Spaces.load`.
 */
async function withPersonal<T extends { id: string; name: string; owner: string; created: unknown }>(all: T[]): Promise<T[]> {
  if (all.length) return all;
  return [(await server.operate<{ space: T }>("spaces.create", { name: "Personal" })).space];
}

class Spaces {
  list = $state<SpaceInfo[]>(FIXTURE_SPACES);
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
  private loading: Promise<void> | null = null;
  /**
   * Reads the signed-in person's spaces from the real server (`spaces.list`; fixtures stay when it is not selected). The personal space is the one this account owns that was
   * created first, and comes first in the list. A failure keeps what is shown and is tried again on the next call. One request; later callers share it.
   */
  load(): Promise<void> {
    if (!serverMode()) return Promise.resolve();
    this.loading ??= (async () => {
      let [{ spaces: all }, me] = await Promise.all([server.spaces.list(), fetch("/api/v1/auth/me", { headers: server.authHeaders() }).then((r) => (r.ok ? r.json() : null)).catch(() => null) as Promise<{ user?: { id?: string } } | null>]);
      const mine = me?.user?.id;
      all = await withPersonal(all);
      const byAge = [...all].sort((a, b) => (a.created < b.created ? -1 : a.created > b.created ? 1 : 0));
      const personal = byAge.find((s) => (s as { personal?: boolean }).personal) ?? byAge.find((s) => s.owner === mine); // the server marks it (`personal: true`); owner-and-oldest is the stand-in until that is in dev
      this.setList((personal ? [personal, ...byAge.filter((s) => s !== personal)] : byAge).map((s) => ({ id: s.id, name: s.name })));
    })().catch((e) => { this.loading = null; throw e; });
    return this.loading;
  }
}

function read(): string {
  try { return localStorage.getItem(KEY) ?? FIXTURE_SPACES[0]!.id; } catch { return FIXTURE_SPACES[0]!.id; }
}

/** The shared spaces state. */
export const spaces = new Spaces();
