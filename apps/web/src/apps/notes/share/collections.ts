/**
 * Collections (the hand-picked set that multi-select makes; the existing app called it a View): what the Notes sidebar needs to share several notes at once. `collections.create` makes one from the picked notes; the
 * Share sheet then shares it as `{kind: "collections", id}`. Fixtures by default, the real server's operation when one is chosen. (`collections.get` and `collections.set_members` are the collection page's.)
 */
import { ServerError, server, serverMode } from "../../../server/client";

/** The collection `collections.create` made: its id and first revision. */
export interface Created { id: string; revision: string }

/** A collection as the page shows it: its title, the revision a membership edit is checked against, and its notes in order. */
export interface Collection { id: string; title: string; revision: string; members: { id: string; title: string }[] }

/** The fixtures' collections (in memory), with one the person was shared into. */
const store = new Map<string, { title: string; revision: number; members: string[] }>([["c-demo", { title: "Launch notes", revision: 1, members: ["n-4", "n-3"] }]]);

/** Makes a collection of these notes in a space. */
export async function createCollection(space_id: string, title: string, members: string[]): Promise<Created> {
  if (serverMode()) return server.operate<Created>("collections.create", { space_id, title, members });
  const id = `c-${members.length}-${Date.now().toString(36)}`;
  store.set(id, { title, revision: 1, members });
  return { id, revision: "1" };
}

/** Reads a collection (`collections.get`): `not_found` when it is gone or the person cannot see it. */
export async function getCollection(id: string): Promise<Collection> {
  if (serverMode()) return server.operate<Collection>("collections.get", { id });
  const c = store.get(id);
  if (!c) throw new ServerError("not_found", "that collection is gone");
  const { notes } = await import("../store.svelte");
  const members = await Promise.all(c.members.map(async (m) => ({ id: m, title: await notes.client.get("", m).then((n) => n.title, () => "A note you cannot see") })));
  return { id, title: c.title, revision: String(c.revision), members };
}

/** Replaces a collection's notes (`collections.set_members`), checked against the revision the caller read: a stale one is a `conflict`. Returns the new revision. */
export async function setMembers(id: string, base_revision: string, members: string[]): Promise<Created> {
  if (serverMode()) return server.operate<Created>("collections.set_members", { id, base_revision, members });
  const c = store.get(id);
  if (!c) throw new ServerError("not_found", "that collection is gone");
  if (String(c.revision) !== base_revision) throw new ServerError("conflict", "the collection changed since you read it");
  c.members = members; c.revision++;
  return { id, revision: String(c.revision) };
}

/** A name for a collection of the picked notes: the first note's title and how many others. */
export const collectionTitle = (titles: string[]): string => (titles.length <= 1 ? (titles[0] ?? "Notes") : `${titles[0]} and ${titles.length - 1} more`);
