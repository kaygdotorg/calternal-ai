/**
 * Reactive state for the Notes app: the spaces, the current space's note list, the open note, kept current by the backend's change feed. The backend is the
 * fixture by default and the real server in server mode (`?server=1`, development token; server/client.ts).
 */
import { spaces as shellSpaces } from "../../shell/spaces.svelte";
import { server, serverMode } from "../../server/client";
import { FixtureNotes, ServerNotes, type Note, type NoteSummary, type NotesClient, type Space } from "./client";
import { FixtureInput, ServerInput, type Draft, type InputClient, type PendingWrite } from "./input";
import { ServerError } from "../../server/client";

class NotesStore {
  client: NotesClient = serverMode() ? new ServerNotes(server) : new FixtureNotes();
  /** Drafts and pending submissions: the account's private input (docs/account-input.md). */
  input: InputClient = serverMode() ? new ServerInput(server) : new FixtureInput((w) => this.client.update(w.spaceId, w.itemId, String(w.input.base_revision), String(w.input.title ?? ""), String(w.input.markdown ?? "")).then(() => {}));
  /** The unsent draft of the open note, if it differs from the saved text. */
  draft = $state<Draft | null>(null);
  /** Writes waiting to be sent. */
  waiting = $state<PendingWrite[]>([]);
  readonly live = serverMode();
  spaces = $state<Space[]>([]);
  space = $state<Space | null>(null);
  list = $state<NoteSummary[]>([]);
  note = $state<Note | null>(null);
  error = $state("");
  private stop: (() => void) | null = null;
  private stopInput: (() => void) | null = null;
  private started = false;

  /** Loads the spaces and the first space's notes, and follows changes. Call once. */
  async start(): Promise<void> {
    if (this.started) return;
    this.started = true;
    try {
      await shellSpaces.load(); // in server mode the shell's list is the server's (and the order puts the personal space first)
      this.spaces = serverMode() ? shellSpaces.list : await this.client.spaces();
      this.space = this.spaces.find((s) => s.id === shellSpaces.current.id) ?? this.spaces[0] ?? null;
      await this.refresh();
      if (this.space) this.stop = this.client.watch(this.space.id, () => void this.refresh());
      await this.reloadInput();
      this.stopInput = this.input.watch(() => void this.reloadInput());
    } catch (e) {
      this.error = (e as Error).message;
    }
  }
  /** Follows the shell's current space: the list is that space's notes. The open note stays open. */
  async useSpace(id: string): Promise<void> {
    const next = this.spaces.find((s) => s.id === id);
    if (!next || next.id === this.space?.id) return;
    this.space = next;
    this.stop?.();
    this.stop = this.client.watch(next.id, () => void this.refresh());
    await this.refresh();
  }
  async refresh(): Promise<void> {
    if (!this.space) return;
    this.list = await this.client.list(this.space.id);
  }
  /** Reloads the private snapshot (the open note's draft, the pending writes): at start and whenever the account feed says it changed. */
  async reloadInput(): Promise<void> {
    try {
      this.waiting = await this.input.pending();
      const n = this.note;
      const d = n ? await this.input.draft(n.spaceId, n.id) : null;
      this.draft = d && d.text !== n?.markdown ? d : null;
    } catch { /* the server is away: what is shown stays */ }
  }
  /** Keeps the text being edited as the note's draft (a complete replacement), so a closed tab or a dropped connection loses nothing. */
  async keepDraft(markdown: string): Promise<void> {
    const n = this.note;
    if (!n) return;
    try { await this.input.saveDraft(n.spaceId, n.id, markdown); } catch { /* best effort: the editor still has the text */ }
  }
  /** Opens a note by id (null clears it). */
  async open(id: string | undefined): Promise<void> {
    if (!id || !this.space) { this.note = null; return; }
    try {
      this.note = await this.client.get(this.space.id, id);
      this.error = "";
      if (this.note.spaceId !== this.space.id) shellSpaces.switchTo(this.note.spaceId); // a note of another space (from the palette, a link): that space becomes current
    } catch (e) { this.note = null; this.error = (e as Error).message; }
    await this.reloadInput();
  }
  /** Saves the open note. A conflict reloads it and says so. */
  async save(title: string, markdown: string): Promise<boolean> {
    if (!this.note || !this.space) return false;
    try {
      this.note = await this.client.update(this.space.id, this.note.id, this.note.revision, title, markdown);
      await this.input.dropDraft(this.note.spaceId, this.note.id).catch(() => {});
      await this.refresh();
      await this.reloadInput();
      this.error = "";
      return true;
    } catch (e) {
      if (e instanceof ServerError && e.code === "unavailable") {
        // The server could not be reached: the write waits, with its creation time, and is sent when the person says so (or on reconnect).
        const n = this.note;
        await this.input.queue({ id: crypto.randomUUID(), spaceId: n.spaceId, itemId: n.id, createdAtMs: Date.now(), operation: "notes.update", input: { space_id: n.spaceId, id: n.id, base_revision: n.revision, title, markdown } });
        await this.reloadInput();
        this.error = "";
        return true;
      }
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
  /** Sends a waiting write now; if the server is still away it stays queued. */
  async sendWaiting(id: string): Promise<void> {
    try { await this.input.send(id); await this.refresh(); await this.open(this.note?.id); this.error = ""; } catch (e) { this.error = (e as Error).message; }
    await this.reloadInput();
  }
  async cancelWaiting(id: string): Promise<void> { await this.input.cancel(id); await this.reloadInput(); }
  destroy(): void { this.stop?.(); this.stopInput?.(); }
}
/** The shared notes state. */
export const notes = new NotesStore();
