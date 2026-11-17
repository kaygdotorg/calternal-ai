/**
 * What the Notes app needs from a backend: spaces, notes (list, read, create, update with a base revision), and a way to hear that something changed. Two
 * implementations: fixtures (the default, in memory) and the real server (`server/client.ts`, development token mode). The app only sees this interface.
 */
import { ServerClient, ServerError } from "../../server/client";

export interface Space { id: string; name: string }
export interface NoteSummary { id: string; spaceId: string; title: string; updatedAt: string }
export interface Note extends NoteSummary { markdown: string; revision: string }

export interface NotesClient {
  spaces(): Promise<Space[]>;
  list(spaceId: string): Promise<NoteSummary[]>;
  get(spaceId: string, id: string): Promise<Note>;
  create(spaceId: string, title: string, markdown: string): Promise<Note>;
  /** Replaces the note against the revision the caller read; a stale revision is a conflict (`ServerError` "conflict"). */
  update(spaceId: string, id: string, baseRevision: string, title: string, markdown: string): Promise<Note>;
  /** Calls `onChange` when a note in the space changed elsewhere (the live stream). Returns the stop function. */
  watch(spaceId: string, onChange: () => void): () => void;
}

const SEED: [string, string, string][] = [
  ["n-1", "Journal replay notes", "# Journal replay notes\n\n## Why the journal is read twice\n\nThe journal is read once to find the tail and once more to verify it.\n\n## What changes on restart\n\nA restart with a long journal feels slow.\n\n## The bookmark\n\nThe bookmark is written after the last entry, not before.\n\n## Open questions\n\nShould the cache be per space or per installation?"],
  ["n-2", "Storage layout", "# Storage layout\n\nThree roots: appdata, userdata-ssd and userdata-hdd."],
  ["n-3", "Invite flow", "# Invite flow\n\nA single-use, expiring link that creates an account and assigns its groups."],
  ["n-4", "Release checklist", "# Release checklist\n\n- tests green\n- docs gate\n- changelog"],
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
  private notes = new Map<string, Note>([...SEED, ["n-long", "The long note", longNote()] as [string, string, string]].map(([id, title, markdown], i) => [id, { id, spaceId: "home", title, markdown, revision: "1", updatedAt: new Date(Date.UTC(2026, 10, 17, 9, 50 - i)).toISOString() }]));
  private listeners = new Set<() => void>();
  async spaces(): Promise<Space[]> { return [{ id: "home", name: "Home" }]; }
  async list(): Promise<NoteSummary[]> { return [...this.notes.values()].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)).map(({ markdown: _m, revision: _r, ...s }) => s); }
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
    const n = await this.get("", id);
    if (n.revision !== baseRevision) throw new ServerError("conflict", "the note changed since you read it");
    const next = { ...n, title, markdown, revision: String(Number(n.revision) + 1), updatedAt: new Date().toISOString() };
    this.notes.set(id, next);
    this.ping();
    return next;
  }
  watch(_s: string, onChange: () => void): () => void { this.listeners.add(onChange); return () => this.listeners.delete(onChange); }
  private ping() { for (const l of this.listeners) l(); }
}

/** The real server, for the operations it already offers. */
export class ServerNotes implements NotesClient {
  constructor(private server: ServerClient) {}
  async spaces(): Promise<Space[]> { return (await this.server.spaces.list()).spaces.map((s) => ({ id: s.id, name: s.name })); }
  async list(spaceId: string): Promise<NoteSummary[]> { return (await this.server.notes.list(spaceId)).notes.map((n) => ({ id: n.id, spaceId: n.space_id, title: n.title, updatedAt: n.updated_at })); }
  private conv(n: { id: string; space_id: string; title: string; markdown: string; revision: string; updated_at: string }): Note { return { id: n.id, spaceId: n.space_id, title: n.title, markdown: n.markdown, revision: n.revision, updatedAt: n.updated_at }; }
  async get(spaceId: string, id: string): Promise<Note> { return this.conv((await this.server.notes.get(spaceId, id)).note); }
  async create(spaceId: string, title: string, markdown: string): Promise<Note> { return this.conv((await this.server.notes.create({ space_id: spaceId, title, markdown })).note); }
  async update(spaceId: string, id: string, baseRevision: string, title: string, markdown: string): Promise<Note> { return this.conv((await this.server.notes.update({ space_id: spaceId, id, base_revision: baseRevision, title, markdown })).note); }
  watch(spaceId: string, onChange: () => void): () => void {
    const ac = new AbortController();
    const run = async () => {
      // After a reset the derived state is stale: tell the app to reload, then subscribe again from the head.
      for (let tries = 0; !ac.signal.aborted && tries < 5; tries++) {
        let reset = false;
        try { await this.server.live([{ space_id: spaceId, bookmark: null }], (f) => { if ("Entry" in f) onChange(); else if ("Reset" in f) { reset = true; onChange(); } }, ac.signal); } catch { /* the stream ended or the server is away */ }
        if (!reset) break;
      }
    };
    void run();
    return () => ac.abort();
  }
}
