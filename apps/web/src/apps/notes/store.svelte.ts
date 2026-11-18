/**
 * Reactive state for the Notes app: the current space's note list (the newest page, a tag filter), the open note and its save state (saved, saving, conflict, offline), the Trash, and the private draft, kept current by
 * the backend's change feed. The backend is the fixture by default and the real server in server mode (server/client.ts). A save never touches what the editor shows (`view`), so typing is never overwritten.
 */
import { spaces as shellSpaces } from "../../shell/spaces.svelte";
import { server, serverMode } from "../../server/client";
import { FixtureNotes, ServerNotes, type Note, type NoteSummary, type NotesClient, type Space, type TagInfo } from "./client";
import { FixtureInput, ServerInput, type Draft, type InputClient, type PendingWrite } from "./input";
import { ServerError } from "../../server/client";
import { say } from "../../server/say";
import { failed, undoable } from "@calternal/ui";
import { navigate, segments } from "../../lib/route.svelte";

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
  /** The tags in use in the space, and the one the list is filtered by (null: all notes). */
  tags = $state<TagInfo[]>([]);
  tag = $state<string | null>(null);
  /** The space's Trash (loaded when the Trash view opens, and after a delete or restore). */
  trashed = $state<NoteSummary[]>([]);
  /** The saved note: the revision the next save is checked against. What the editor shows is `view`, which a save never touches (so typing during a save is never overwritten). */
  note = $state<Note | null>(null);
  /** What the editor shows: set when a note is opened, replaced from elsewhere, or when the person takes the other version; `seq` says it is new. */
  view = $state<{ id: string; text: string; seq: number }>({ id: "", text: "", seq: 0 });
  /** Where the open note stands: `saved`, `dirty` (typed, not sent yet), `saving`, `conflict` (the server has a newer version), `waiting` (offline: kept on this device, sent when the server is back), `failed`. */
  saveState = $state<"saved" | "dirty" | "saving" | "conflict" | "waiting" | "failed">("saved");
  /** The server's newer version, while `saveState` is `conflict`. */
  conflict = $state<Note | null>(null);
  /** Saving took this long the last time, in milliseconds (shown nowhere; the performance report reads it). */
  lastSaveMs = 0;
  error = $state("");
  /** The stable code of the failure behind `error` (`unavailable`, `not_found`, `forbidden`, …), so the screen can offer the right next step. */
  errorCode = $state("");
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
      if (this.space) this.stop = this.client.watch(this.space.id, () => void this.follow());
      await this.reloadInput();
      this.stopInput = this.input.watch(() => void this.reloadInput());
    } catch (e) {
      this.error = say(e, "load the notes"); this.errorCode = e instanceof ServerError ? e.code : "";
    }
  }
  /** Follows the shell's current space: the list is that space's notes. The open note stays open. */
  async useSpace(id: string): Promise<void> {
    const next = this.spaces.find((s) => s.id === id);
    if (!next || next.id === this.space?.id) return;
    this.space = next;
    this.stop?.();
    this.stop = this.client.watch(next.id, () => void this.follow());
    await this.refresh();
  }
  /**
   * A change came from elsewhere (the space journal, no polling): refresh the list, and if the open note changed there, show it (nothing being typed), or remember it as the conflict the next save will meet.
   */
  async follow(): Promise<void> {
    await this.refresh().catch(() => {});
    const n = this.note;
    if (!n || !this.space) return;
    try {
      const latest = await this.client.get(this.space.id, n.id);
      if (latest.revision === n.revision) return;
      if (this.saveState === "saved") { this.note = latest; this.show(latest); await this.reloadInput(); }
      else if (this.saveState !== "saving") { this.conflict = latest; this.saveState = "conflict"; }
    } catch { /* gone or away: the list refresh and the banner tell it */ }
  }
  private show(n: Note): void { this.typing = false; this.view = { id: n.id, text: n.markdown, seq: this.view.seq + 1 }; }
  /** The person typed: the note is no longer in step with the server. */
  touch(): void {
    this.typing = true; if (this.saveState === "saved" || this.saveState === "failed" || this.saveState === "waiting") this.saveState = "dirty"; }
  async refresh(): Promise<void> {
    if (!this.space) return;
    this.list = await this.client.list(this.space.id, this.tag ?? undefined);
    void this.client.tags(this.space.id).then((t) => (this.tags = t), () => {});
  }
  /** Filters the list by a tag (null: all notes). */
  async filter(tag: string | null): Promise<void> { this.tag = tag; await this.refresh(); }
  /** Reads the Trash. */
  async loadTrash(): Promise<void> {
    if (!this.space) return;
    try { this.trashed = await this.client.trash(this.space.id); this.error = ""; } catch (e) { this.error = say(e, "load the Trash"); }
  }
  /** Moves a note to the Trash with an Undo; if it is the open note the card goes back to the list. */
  async remove(id: string, title: string): Promise<void> {
    try {
      await this.client.remove(id);
      await this.refresh();
      if (segments()[0] === "notes" && segments()[1] === id) navigate("/notes");
      undoable("note.trash", id, `Moved \u201c${title}\u201d to the Trash`, async () => { try { await this.client.restore(id); await this.refresh(); return true; } catch { return false; } });
    } catch (e) { failed(say(e, "move the note to the Trash"), () => void this.remove(id, title)); }
  }
  /** Brings a note back from the Trash. */
  async restore(id: string): Promise<void> {
    try { await this.client.restore(id); await Promise.all([this.refresh(), this.loadTrash()]); } catch (e) { failed(say(e, "restore the note"), () => void this.restore(id)); }
  }
  /** Deletes a trashed note for good. */
  async purge(id: string): Promise<void> {
    try { await this.client.purge(id); await this.loadTrash(); } catch (e) { failed(say(e, "delete the note"), () => void this.purge(id)); }
  }
  /** Renames a note from the list (the revision is the list's: a stale one is a conflict, said in words). */
  async rename(n: NoteSummary, title: string): Promise<void> {
    const t = title.trim();
    if (!t || t === n.title) return;
    try {
      const renamed = await this.client.rename(n.id, n.revision ?? "", t);
      if (this.note?.id === n.id) { this.note = renamed; this.show(renamed); this.saveState = "saved"; }
      await this.refresh();
    } catch (e) { failed(say(e, "rename the note"), () => void this.rename(n, title)); }
  }
  /** Tries again after a failed load (the start-up, or the open note). */
  async retry(id?: string): Promise<void> {
    this.error = ""; this.errorCode = "";
    if (!this.space) { this.started = false; await this.start(); }
    else await this.refresh().catch((e) => { this.error = say(e, "load the notes"); });
    if (id) await this.open(id);
  }
  /** Reloads the private snapshot (the open note's draft, the pending writes): at start and whenever the account feed says it changed. */
  async reloadInput(): Promise<void> {
    const mine = ++this.inputSeq; // only the newest read may set the state: an older one finishing late must not bring back a draft that was just dropped
    try {
      const waiting = await this.input.pending();
      const n = this.note;
      const d = n ? await this.input.draft(n.spaceId, n.id) : null;
      if (mine !== this.inputSeq) return;
      this.waiting = waiting;
      // A draft is offered only when it is not this editor's own (text kept while typing, dropped by the save) and differs from the saved text.
      this.draft = d && d.text !== n?.markdown && this.saveState === "saved" && this.view.id === n?.id && !this.typing ? d : null;
    } catch { /* the server is away: what is shown stays */ }
  }
  private inputSeq = 0;
  /** True from the first keystroke of this open until the note is opened again: drafts written meanwhile are this editor's own. */
  typing = false;
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
      this.show(this.note); this.saveState = "saved"; this.conflict = null; this.typing = false;
      this.error = ""; this.errorCode = "";
      if (this.note.spaceId !== this.space.id) shellSpaces.switchTo(this.note.spaceId); // a note of another space (from the palette, a link): that space becomes current
    } catch (e) { this.note = null; this.error = say(e, "open the note"); this.errorCode = e instanceof ServerError ? e.code : ""; }
    await this.reloadInput();
  }
  /**
   * Saves the open note: the whole text replaces it, checked against the revision last read. The editor keeps what is typed meanwhile. If the server is away the write is queued (with its time) and the draft stays;
   * if the server has a newer version the state is `conflict` and the person chooses (`keepMine`, `takeTheirs`); nothing typed is dropped either way.
   */
  async save(title: string, markdown: string): Promise<boolean> {
    if (!this.note || !this.space) return false;
    this.saveState = "saving";
    const t0 = performance.now();
    try {
      const saved = await this.client.update(this.space.id, this.note.id, this.note.revision, title, markdown);
      this.lastSaveMs = performance.now() - t0;
      this.note = saved;
      this.saveState = "saved"; this.conflict = null; this.error = "";
      // The draft goes first, then the snapshot is read: otherwise the stale draft is read back and offered as "unsent" right after a save.
      void this.input.dropDraft(saved.spaceId, saved.id).catch(() => {}).then(() => this.refresh()).then(() => this.reloadInput());
      return true;
    } catch (e) {
      if (e instanceof ServerError && e.code === "unavailable") {
        // The server could not be reached: the write waits, with its creation time, and is sent when the person says so (or on reconnect).
        const n = this.note;
        await this.input.queue({ id: crypto.randomUUID(), spaceId: n.spaceId, itemId: n.id, createdAtMs: Date.now(), operation: "notes.update", input: { space_id: n.spaceId, id: n.id, base_revision: n.revision, title, markdown } }).catch(() => {});
        await this.reloadInput();
        this.saveState = "waiting"; this.error = "";
        return true;
      }
      if (e instanceof ServerError && e.code === "conflict") {
        try { this.conflict = await this.client.get(this.space.id, this.note.id); this.saveState = "conflict"; } catch { this.saveState = "failed"; this.error = say(e, "save the note"); }
        return false;
      }
      this.saveState = "failed"; this.error = say(e, "save the note");
      return false;
    }
  }
  /** Conflict: keep what was typed and save it over the server's newer version. */
  async keepMine(title: string, markdown: string): Promise<void> {
    if (!this.conflict) return;
    this.note = this.conflict; this.conflict = null; this.saveState = "dirty";
    await this.save(title, markdown);
  }
  /** Conflict: drop what was typed here and show the server's version. */
  async takeTheirs(): Promise<void> {
    const theirs = this.conflict;
    if (!theirs) return;
    this.note = theirs; this.conflict = null; this.show(theirs); this.saveState = "saved";
    await this.input.dropDraft(theirs.spaceId, theirs.id).catch(() => {});
    await this.reloadInput();
  }
  async create(title: string): Promise<Note | null> {
    if (!this.space) return null;
    try {
      const n = await this.client.create(this.space.id, title, `# ${title}\n`);
      await this.refresh();
      return n;
    } catch (e) { this.error = say(e, "make the note"); return null; }
  }
  /** Sends a waiting write now; if the server is still away it stays queued. */
  async sendWaiting(id: string): Promise<void> {
    try { await this.input.send(id); await this.refresh(); await this.open(this.note?.id); this.error = ""; } catch (e) { this.error = say(e, "send the waiting change"); }
    await this.reloadInput();
  }
  async cancelWaiting(id: string): Promise<void> { await this.input.cancel(id); await this.reloadInput(); }
  destroy(): void { this.stop?.(); this.stopInput?.(); }
}
/** The shared notes state. */
export const notes = new NotesStore();
