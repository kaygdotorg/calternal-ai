/**
 * What the Notes app needs from a backend: spaces, notes (list, read, create, update with a base revision, rename, trash, restore), tags and title lookups, and a way to hear that something changed. Two
 * implementations: fixtures (the default, in memory) and the real server (`server/client.ts`). The app only sees this interface.
 */
import { ServerClient, ServerError } from "../../server/client";
import { FIXTURE_SPACES } from "../../shell/spaces.svelte";

/** One page of a list: its rows and the cursor for the next page (null at the end). A cursor is opaque, belongs to the account, space and filter that made it, and may stop working (the server restarted): the caller then starts over. */
export interface Page<T> { items: T[]; next: string | null }
/** The rows of a page of `size` (the server's is 100) starting where `cursor` says; the fixtures' cursor is an offset. */
const pageOf = <T,>(all: T[], cursor?: string, size = 100): Page<T> => { const from = cursor ? Number(cursor) : 0; return { items: all.slice(from, from + size), next: from + size < all.length ? String(from + size) : null }; };

/** A journal entry as the screens need it: what happened (`notes.edited`, …) and to which note. */
export interface Change { kind: string; item: string }

/** A space as Notes needs it: its UUID and the name the person sees. */
export interface Space { id: string; name: string }
/** A note in a list. `revision` is what a rename is checked against; in the Trash `updatedAt` is when it was deleted. */
export interface NoteSummary { id: string; spaceId: string; title: string; updatedAt: string; revision?: string }
/** A tag and how many live notes carry it (a parent tag counts its children). */
export interface TagInfo { name: string; count: number }
/** A note a `[[` picker can link to: `target` is the relative link to insert (`Plan.md`); fixtures have none and link by title. */
export interface NoteTarget { id: string; title: string; target?: string }
/** A whole note: its Markdown and the revision the next save is checked against (opaque; a stale one is a conflict). */
export interface Note extends NoteSummary { markdown: string; revision: string }

/**
 * What the Notes app needs from a backend, whether fixtures or the real server: list, read, create, save, rename, trash, restore, tags and lookups, and a way to hear that something changed. Failures are `ServerError`
 * (`conflict` for a stale revision, `unavailable` when the server cannot be reached, `not_found` for a note that is gone); nothing here retries. Lists come a page (100 on the server) at a time with a cursor.
 */
export interface NotesClient {
  /** The spaces the person is in; the real server takes them from the shell's list instead. */
  spaces(): Promise<Space[]>;
  /** A page of the space's live notes, newest first, optionally only those under a tag; `cursor` is the previous page's `next`. */
  list(spaceId: string, tag?: string, cursor?: string): Promise<Page<NoteSummary>>;
  /** Reads one note. Throws `not_found` when it is gone or the person cannot see it. */
  get(spaceId: string, id: string): Promise<Note>;
  /** Creates a note with this title and Markdown and returns it with its first revision. */
  create(spaceId: string, title: string, markdown: string): Promise<Note>;
  /** Replaces the note against the revision the caller read; a stale revision is a conflict (`ServerError` "conflict"). */
  update(spaceId: string, id: string, baseRevision: string, title: string, markdown: string): Promise<Note>;
  /** Sets a new title at the revision the caller read (the list's); the first heading follows. */
  rename(id: string, revision: string, title: string): Promise<Note>;
  /** Moves a note to the space's Trash (history kept). */
  remove(id: string): Promise<void>;
  /** Brings a note back from the Trash (same id). */
  restore(id: string): Promise<Note>;
  /** A page of the space's Trash, newest deletion first. */
  trash(spaceId: string, cursor?: string): Promise<Page<NoteSummary>>;
  /** Deletes a trashed note for good. */
  purge(id: string): Promise<void>;
  /** A page of the tags in use in the space. */
  tags(spaceId: string, cursor?: string): Promise<Page<TagInfo>>;
  /** Notes whose title starts with `prefix`, for the `[[` picker. */
  lookupNotes(spaceId: string, prefix: string): Promise<NoteTarget[]>;
  /** Tags whose name starts with `prefix`, for the `#` picker. */
  lookupTags(spaceId: string, prefix: string): Promise<TagInfo[]>;
  /** Calls `onChange` when a note in the space changed elsewhere (the live stream). Returns the stop function. */
  /** Calls `onChange` when the space changed (no polling); `change` names the item and kind of the journal entry when the server sent one, so a screen can deal with that one note without reading its lists again. */
  watch(spaceId: string, onChange: (change?: Change) => void): () => void;
}

const SEED: [string, string, string][] = [
  ["n-1", "Journal replay notes", "# Journal replay notes\n\n## Why the journal is read twice\n\nThe journal is read once to find the tail and once more to verify it.\n\n## What changes on restart\n\nA restart with a long journal feels slow.\n\n## The bookmark\n\nThe bookmark is written after the last entry, not before.\n\n## Open questions\n\nShould the cache be per space or per installation?"],
  ["n-2", "Storage layout", "# Storage layout\n\nThree roots: appdata, userdata-ssd and userdata-hdd."],
  ["n-3", "Invite flow", "# Invite flow\n\nA single-use, expiring link that creates an account and assigns its groups."],
  ["n-4", "Release checklist", "# Release checklist\n\n- tests green\n- docs gate\n- changelog"],
  ["n-i1", "Mesh rollout plan", "# Mesh rollout plan\n\nRoll the mesh out one machine at a time, starting with the dev VMs."],
  ["n-i2", "Peer memory budget", "# Peer memory budget\n\nIdle peers must stay inside the RSS budget."],
  ["n-5", "Reading list", "# Reading list\n\nNative UI study, the Craft study."],
  ["n-rich", "Rich note", "---\ntitle: Rich note\ntags: [design]\n---\n# Rich note\n\nA table:\n\n| Root | Where | Speed |\n| :-- | --- | --: |\n| appdata | SSD | fast |\n| userdata-hdd | HDD | slow |\n\n- [ ] write the grid ^k2m8q1w4zre7k3f9\n- [x] hide the markers\n\n```ts\nconst answer: number = 42; // the answer\nfunction greet(name: string) { return `hi ${name}`; }\n```\n\nDone."],
];

/** A long fixture note of about `words` words (headings, paragraphs, lists, code), for measuring the editor on a document the size of a book chapter. */
export function longNote(words = 50_000): string {
  const out: string[] = ["# The long note", ""];
  const sentence = "The journal is read once to find the tail and once more to verify it, so a restart with a long journal feels slow. ";
  let n = 0, i = 0;
  while (n < words) {
    out.push(`## Section ${++i}`, "");
    out.push(sentence.repeat(4) + "A **bold** claim, some `code`, and a [link](https://example.test/page).", "");
    out.push("- the reader no longer calls `sync_all` per entry", "  - verified once over the tail", "- [ ] write it down", "- [x] measure it", "");
    if (i % 5 === 0) out.push("```rust", "let tail = journal.read_tail(max_bytes)?;", "```", "");
    if (i % 7 === 0) out.push("> A quoted line that records why.", "");
    n += 4 * 21 + 30;
  }
  return out.join("\n");
}

/** The in-memory fixture backend. */
export class FixtureNotes implements NotesClient {
  private notes = new Map<string, Note>([...SEED, ["n-long", "The long note", longNote()] as [string, string, string]].map(([id, title, markdown], i) => [id, { id, spaceId: id.startsWith("n-i") ? FIXTURE_SPACES[1]!.id : FIXTURE_SPACES[0]!.id, title, markdown, revision: "1", updatedAt: new Date(Date.UTC(2026, 10, 17, 9, 50 - i)).toISOString() }]));
  private listeners = new Set<() => void>();
  async spaces(): Promise<Space[]> { return FIXTURE_SPACES; }
  async list(spaceId: string, tag?: string, cursor?: string): Promise<Page<NoteSummary>> { return pageOf([...this.notes.values()].filter((n) => n.spaceId === spaceId && (!tag || this.tagsOf(n).some((t) => t === tag || t.startsWith(`${tag}/`)))).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).map(({ markdown: _m, revision: _r, ...s }) => s), cursor); }
  async get(_s: string, id: string): Promise<Note> {
    const n = this.notes.get(id);
    if (!n) throw new ServerError("not_found", "note not found");
    return n;
  }
  async create(spaceId: string, title: string, markdown: string): Promise<Note> {
    const n: Note = { id: `n-${this.notes.size + 1}`, spaceId, title, markdown, revision: "1", updatedAt: new Date().toISOString() };
    this.notes.set(n.id, n);
    this.ping();
    return n;
  }
  async update(_s: string, id: string, baseRevision: string, title: string, markdown: string): Promise<Note> {
    // `?revoked=1` (fixtures only) is a note whose share has ended: every write is refused, to try the rescue.
    if (new URLSearchParams(location.search).has("revoked")) throw new ServerError("forbidden", "you can no longer edit this note");
    const n = await this.get("", id);
    if (n.revision !== baseRevision) throw new ServerError("conflict", "the note changed since you read it");
    const next = { ...n, title, markdown, revision: String(Number(n.revision) + 1), updatedAt: new Date().toISOString() };
    this.notes.set(id, next);
    this.ping();
    return next;
  }
  private trashed = new Map<string, Note>();
  private tagsOf = (n: Note): string[] => [...new Set([...n.markdown.matchAll(/(?:^|\s)#([\p{L}\d_/-]+)/gu)].map((m) => m[1]!))];
  async rename(id: string, revision: string, title: string): Promise<Note> { const n = await this.get("", id); return this.update("", id, revision || n.revision, title, n.markdown.replace(/^# .*/m, `# ${title}`)); }
  async remove(id: string): Promise<void> { const n = await this.get("", id); this.notes.delete(id); this.trashed.set(id, { ...n, updatedAt: new Date().toISOString() }); this.ping(); }
  async restore(id: string): Promise<Note> { const n = this.trashed.get(id); if (!n) throw new ServerError("not_found", "not in the trash"); this.trashed.delete(id); this.notes.set(id, n); this.ping(); return n; }
  async trash(spaceId: string, cursor?: string): Promise<Page<NoteSummary>> { return pageOf([...this.trashed.values()].filter((n) => n.spaceId === spaceId).sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).map(({ markdown: _m, ...x }) => x), cursor); }
  async purge(id: string): Promise<void> { this.trashed.delete(id); this.ping(); }
  async tags(spaceId: string, cursor?: string): Promise<Page<TagInfo>> { const c = new Map<string, number>(); for (const n of this.notes.values()) if (n.spaceId === spaceId) for (const t of this.tagsOf(n)) c.set(t, (c.get(t) ?? 0) + 1); return pageOf([...c].map(([name, count]) => ({ name, count })).sort((a, b) => a.name.localeCompare(b.name)), cursor); }
  async lookupNotes(spaceId: string, prefix: string): Promise<NoteTarget[]> { return [...this.notes.values()].filter((n) => n.spaceId === spaceId && n.title.toLowerCase().startsWith(prefix.toLowerCase())).map((n) => ({ id: n.id, title: n.title })); }
  async lookupTags(spaceId: string, prefix: string): Promise<TagInfo[]> { return (await this.tags(spaceId)).items.filter((t) => t.name.toLowerCase().startsWith(prefix.toLowerCase())); }
  watch(_s: string, onChange: (change?: Change) => void): () => void { this.listeners.add(onChange); return () => this.listeners.delete(onChange); }
  private ping() { for (const l of this.listeners) l(); }
}

/** The real server, for the operations it already offers. */
export class ServerNotes implements NotesClient {
  constructor(private server: ServerClient) {}
  async spaces(): Promise<Space[]> { return (await this.server.spaces.list()).spaces.map((s) => ({ id: s.id, name: s.name })); }
  sum = (n: { id: string; space_id: string; title: string; updated_at: string; revision: string }): NoteSummary => ({ id: n.id, spaceId: n.space_id, title: n.title, updatedAt: n.updated_at, revision: n.revision });
  async list(spaceId: string, tag?: string, cursor?: string): Promise<Page<NoteSummary>> { const p = await this.server.notes.list(spaceId, tag, cursor); return { items: p.notes.map(this.sum), next: p.next_cursor ?? null }; }
  conv(n: { id: string; space_id: string; title: string; markdown: string; revision: string; updated_at: string }): Note { return { id: n.id, spaceId: n.space_id, title: n.title, markdown: n.markdown, revision: n.revision, updatedAt: n.updated_at }; }
  async get(_spaceId: string, id: string): Promise<Note> { return this.conv((await this.server.notes.get("", id)).note); } // the server finds the owning space from the id: a note shared from another space is not in the current one
  async create(spaceId: string, title: string, markdown: string): Promise<Note> { return this.conv((await this.server.notes.create({ space_id: spaceId, title, markdown })).note); }
  async update(spaceId: string, id: string, baseRevision: string, title: string, markdown: string): Promise<Note> { return this.conv((await this.server.notes.update({ space_id: spaceId, id, base_revision: baseRevision, title, markdown })).note); }
  async rename(id: string, revision: string, title: string): Promise<Note> { return this.conv((await this.server.operate<{ note: Parameters<ServerNotes["conv"]>[0] }>("notes.rename", { id, base_revision: revision, title })).note); }
  async remove(id: string): Promise<void> { await this.server.operate("notes.delete", { id }); }
  async restore(id: string): Promise<Note> { return this.conv((await this.server.operate<{ note: Parameters<ServerNotes["conv"]>[0] }>("notes.restore", { id })).note); }
  async trash(spaceId: string, cursor?: string): Promise<Page<NoteSummary>> { const p = await this.server.operate<{ notes: Parameters<ServerNotes["sum"]>[0][]; next_cursor?: string }>("trash.list", { space_id: spaceId, ...(cursor ? { cursor } : {}) }); return { items: p.notes.map(this.sum), next: p.next_cursor ?? null }; }
  async purge(id: string): Promise<void> { await this.server.operate("trash.purge", { id }); }
  async tags(spaceId: string, cursor?: string): Promise<Page<TagInfo>> { const p = await this.server.operate<{ tags: TagInfo[]; next_cursor?: string }>("tags.list", { space_id: spaceId, ...(cursor ? { cursor } : {}) }); return { items: p.tags, next: p.next_cursor ?? null }; }
  async lookupNotes(spaceId: string, prefix: string): Promise<NoteTarget[]> { return (await this.server.operate<{ notes: { id: string; title: string; link_target: string }[] }>("notes.lookup", { space_id: spaceId, prefix })).notes.map((n) => ({ id: n.id, title: n.title, target: n.link_target })); }
  async lookupTags(spaceId: string, prefix: string): Promise<TagInfo[]> { return (await this.server.operate<{ tags: TagInfo[] }>("tags.lookup", { space_id: spaceId, prefix })).tags; }
  watch(spaceId: string, onChange: (change?: Change) => void): () => void {
    const ac = new AbortController();
    const run = async () => {
      // After a reset the derived state is stale: tell the app to reload, then subscribe again from the head.
      for (let tries = 0; !ac.signal.aborted && tries < 5; tries++) {
        let reset = false;
        try { await this.server.live([{ space_id: spaceId, bookmark: null }], (f) => { if ("Entry" in f) { const e = f.Entry.entry as { kind?: unknown; item?: unknown } | null; onChange(typeof e?.item === "string" ? { kind: String(e.kind ?? ""), item: e.item } : undefined); } else if ("Reset" in f) { reset = true; onChange(); } }, ac.signal); } catch { /* the stream ended or the server is away */ }
        if (!reset) break;
      }
    };
    void run();
    return () => ac.abort();
  }
}
