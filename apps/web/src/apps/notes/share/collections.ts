/**
 * Collections (the hand-picked set that multi-select makes; the existing app called it a View): what the Notes sidebar needs to share several notes at once. `collections.create` makes one from the picked notes; the
 * Share sheet then shares it as `{kind: "collections", id}`. Fixtures by default, the real server's operation when one is chosen. (`collections.set_members` and `collections.get` are for editing a collection and are not called yet.)
 */
import { server, serverMode } from "../../../server/client";

/** The collection `collections.create` made: its id and first revision. */
export interface Created { id: string; revision: string }

/** Makes a collection of these notes in a space. */
export async function createCollection(space_id: string, title: string, members: string[]): Promise<Created> {
  if (serverMode()) return server.operate<Created>("collections.create", { space_id, title, members });
  return { id: `c-${members.length}-${Date.now().toString(36)}`, revision: "1" };
}

/** A name for a collection of the picked notes: the first note's title and how many others. */
export const collectionTitle = (titles: string[]): string => (titles.length <= 1 ? (titles[0] ?? "Notes") : `${titles[0]} and ${titles.length - 1} more`);
