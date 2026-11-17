/**
 * Reactive state for the Notes app: the spaces, the current space's note list, the open note, kept current by the backend's change feed. The backend is the
 * fixture by default and the real server in server mode (`?server=1`, development token; server/client.ts).
 */
import { server, serverMode } from "../../server/client";
import { FixtureNotes, ServerNotes, type Note, type NoteSummary, type NotesClient, type Space } from "./client";

class NotesStore {
  client: NotesClient = serverMode() ? new ServerNotes(server) : new FixtureNotes();
  readonly live = serverMode();
  spaces = $state<Space[]>([]);
  space = $state<Space | null>(null);
  list = $state<NoteSummary[]>([]);
  note = $state<Note | null>(null);
  error = $state("");
  private stop: (() => void) | null = null;
  private started = false;

  /** Loads the spaces and the first space's notes, and follows changes. Call once. */
  async start(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      this.spaces = await this.client.spaces();
      this.space = this.spaces[0] ?? null;
      await this.refresh();
      if (this.space) this.stop = this.client.watch(this.space.id, () => void this.refresh());
    } catch (e) {
      this.error = (e as Error).message;
    }
  }
  async refresh(): Promise<void> {
    if (!this.space) return;
    this.list = await this.client.list(this.space.id);
  }
  /** Opens a note by id (null clears it). */
  async open(id: string | undefined): Promise<void> {
    if (!id || !this.space) { this.note = null; return; }
    try { this.note = await this.client.get(this.space.id, id); this.error = ""; } catch (e) { this.note = null; this.error = (e as Error).message; }
  }
  /** Saves the open note. A conflict reloads it and says so. */
  async save(title: string, markdown: string): Promise<boolean> {
    if (!this.note || !this.space) return false;
    try {
      this.note = await this.client.update(this.space.id, this.note.id, this.note.revision, title, markdown);
      await this.refresh();
      this.error = "";
      return true;
    } catch (e) {
      this.error = (e as Error).message;
      await this.open(this.note.id);
      return false;
    }
  }
  async create(title: string): Promise<Note | null> {
    if (!this.space) return null;
    const n = await this.client.create(this.space.id, title, `# ${title}\n`);
    await this.refresh();
    return n;
  }
  destroy(): void { this.stop?.(); }
}
/** The shared notes state. */
export const notes = new NotesStore();
