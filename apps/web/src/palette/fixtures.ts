// Fixture data for the palette: the notes of this space, and threads and notes from a second space, so "search every space" and the space label can be seen.
// The real search runs on the server per space the person can reach (docs/composer.md); this is the stand-in.
import type { Item } from "./search";

export const THIS_SPACE = "Home";
const NOTES: [string, string][] = [["n-1", "Journal replay notes"], ["n-2", "Storage layout"], ["n-3", "Invite flow"], ["n-4", "Release checklist"], ["n-5", "Reading list"]];
const OTHER: Item[] = [
  { id: "x-1", kind: "thread", title: "Rotate the mesh keys", space: "Infra", go: "/threads" },
  { id: "x-2", kind: "thread", title: "Journal compaction on backup01", space: "Infra", go: "/threads" },
  { id: "x-3", kind: "note", title: "Runbook: restoring a space", space: "Infra", go: "/notes" },
];
/** The notes of this space and everything of the other space. */
export const fixtureItems = (): Item[] => [...NOTES.map(([id, title]): Item => ({ id, kind: "note", title, space: THIS_SPACE, go: `/notes/${id}` })), ...OTHER];
