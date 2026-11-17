/**
 * The open thread as the view needs it: a count, a cache of the messages read so far, and the streaming message.
 *
 * Messages are read in pages of `PAGE` around what is on screen (never the whole file). Streaming text lives in its own reactive
 * field, `stream.text`, which only the streaming row reads, so a delta re-renders one component and not the list.
 */
import type { ThreadsClient } from "./client";
import type { Message, ThreadSummary } from "./model";

const PAGE = 100;

/** One open thread. Call `dispose` when leaving it. */
export class Conversation {
  /** Messages in the file, not counting a message still streaming. */
  count = $state(0);
  /** Bumped when a page arrives, so rows waiting on it redraw. Not bumped by streaming. */
  rev = $state(0);
  /** The streaming agent message, or null. */
  stream = $state<{ id: string; text: string } | null>(null);
  summary = $state<ThreadSummary>();

  private cache = new Map<number, Message>();
  private loading = new Set<number>();
  private off: () => void;

  constructor(
    private client: ThreadsClient,
    readonly id: string,
    summary: ThreadSummary,
  ) {
    this.summary = summary;
    this.count = summary.messageCount;
    this.off = client.watch(id, {
      onMessage: (m, i) => {
        this.cache.set(i, m);
        if (this.stream) this.stream = null;
        this.count = Math.max(this.count, i + 1);
        this.rev++;
      },
      onStreamStart: (id) => (this.stream = { id, text: "" }),
      onDelta: (t) => {
        if (this.stream) this.stream.text += t;
      },
      onSummary: (s) => (this.summary = s),
    });
  }

  /** A message if it has been read, else undefined (and the caller shows a placeholder until `rev` changes). Reads `rev` so the row redraws. */
  get(i: number): Message | undefined {
    void this.rev;
    return this.cache.get(i);
  }

  /** Makes sure the pages covering `[from, to)` are read or being read. */
  ensure(from: number, to: number): void {
    for (let p = Math.floor(from / PAGE); p <= Math.floor((Math.min(to, this.count) - 1) / PAGE); p++) {
      if (this.loading.has(p)) continue;
      this.loading.add(p);
      void this.client.messages(this.id, p * PAGE, (p + 1) * PAGE).then((ms) => {
        ms.forEach((m, k) => this.cache.set(p * PAGE + k, m));
        this.rev++;
      });
    }
  }

  /** Stops watching. */
  dispose(): void {
    this.off();
  }
}
