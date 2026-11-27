/** The Notes mockup app: fixture notes, its sidebar as collections, its window page (pinned and recent), the note page, the right panel, and the note's looks (row, strip, card, cell) which the shell uses wherever a note is drawn. Registered by `../index.ts`. */
import type { MockupApp } from "../registry";
import { collections } from "./data";
import NotesHome from "./NotesHome.svelte";
import NotePage from "./NotePage.svelte";
import NotesPanel from "./NotesPanel.svelte";
import NoteLook from "./NoteLook.svelte";

const app: MockupApp = {
  id: "notes", name: "Notes", icon: "notes", kinds: ["note"], action: "New note",
  collections, sidebar: ["notes-pinned", "recent-notes", "nt-folders", "tags"],
  page: NotesHome, itemPage: { note: NotePage }, panel: NotesPanel, looks: { note: NoteLook },
};
export default app;
