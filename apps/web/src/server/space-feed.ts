/**
 * One space's live journal (`GET /api/v1/live`, SSE) as the Threads client uses it: entries for threads, a reset when derived state must be reloaded, reconnects with the server's bookmark
 * (taken from each `Ready` frame, so a dropped connection resumes where it was) and a short, growing wait between attempts. `stop` ends it.
 * Cost: one open connection for the space; nothing runs between entries.
 */
import { ServerClient } from "./client";

/** The parts of a journal entry the Threads client reads: what changed (`kind`, such as `threads.rename`), which item (the thread's UUID) and the entry's own data. */
export interface FeedEntry { kind: string; item: string; data?: Record<string, unknown> }

/** A reconnecting follower of one space's journal: calls `onentry` for each entry and `onreset` when derived state must be reloaded; after a drop it resumes from the server's bookmark (so no entry is missed) after a wait that grows from 0.25 s to 15 s, and `stop` ends it. A failed connection is retried, never thrown. */
export class SpaceFeed {
  private ac = new AbortController();
  private bookmark: string | null = null;

  constructor(private server: ServerClient, readonly space: string, private onentry: (e: FeedEntry) => void, private onreset: () => void) { void this.run(); }

  stop(): void { this.ac.abort(); }

  private async run(): Promise<void> {
    for (let wait = 500; !this.ac.signal.aborted; wait = Math.min(wait * 2, 15000)) {
      let reset = false;
      try {
        await this.server.live([{ space_id: this.space, bookmark: this.bookmark }], (f) => {
          wait = 500 / 2; // a frame arrived: the connection works
          if ("Entry" in f) { const e = f.Entry.entry as Partial<FeedEntry> | undefined; if (e?.kind) this.onentry({ kind: e.kind, item: String(e.item ?? ""), data: e.data }); }
          else if ("Ready" in f) this.bookmark = f.Ready.bookmark;
          else if ("Reset" in f) { reset = true; this.bookmark = null; this.onreset(); }
        }, this.ac.signal);
      } catch { /* the stream ended or the server is away: wait, then try again */ }
      if (this.ac.signal.aborted) return;
      if (reset) wait = 250;
      await new Promise((r) => setTimeout(r, wait));
    }
  }
}
