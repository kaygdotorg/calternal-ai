/**
 * The open thread as the view needs it: a count, a cache of the messages read so far, and the streaming message.
 *
 * Messages are read in pages of `PAGE` around what is on screen (never the whole file). Streaming text lives in its own reactive
 * field, `stream.text`, which only the streaming row reads, so a delta re-renders one component and not the list.
 */
import type { ThreadsClient } from "./client";
import { appearance } from "../../appearance/prefs.svelte";
import type { Message, Step, ThreadSummary } from "./model";

/** The running turn as the working row shows it: what it is doing now, its steps so far, its reasoning if the provider shares it, and when the person sent (to say how long it worked). */
export interface Turn {
  /** The job id once the server has one; empty between the send and the job. */
  id: string;
  /** The message text as written, only when the person turned streaming on. */
  text: string;
  steps: Step[];
  /** What it is doing now, in words ("Sending", "Waiting to start", "Thinking", "Running ls", "Writing"). */
  phase: string;
  /** `Date.now()` when the person sent. */
  startedAt: number;
  /** The provider, model and reasoning level the thread had when the person sent: who wrote the message this turn becomes, until the saved file says so itself. */
  by: { provider: ThreadSummary["provider"]; model: string; reasoning?: string };
  /** The provider's reasoning so far; `ms` is set once it has finished. */
  thought?: { text: string; ms?: number };
}

const PAGE = 100;

/**
 * An agent message as this page saw its turn: the saved file may not record how long it worked or what it thought (an older file, or a provider that does not say), but this page measured it.
 * Only what the message lacks is filled in; what the file records wins.
 */
function carry(m: Message, t: Turn): Message {
  const ms = Date.now() - t.startedAt;
  const blocks = m.blocks.map((b) => (b.type === "steps" && !b.ms ? { ...b, ms } : b));
  if (t.thought && !blocks.some((b) => b.type === "thought")) blocks.unshift({ type: "thought", text: t.thought.text, ms: t.thought.ms ?? ms });
  return { ...m, blocks, by: { ...t.by, ...(m.by ?? {}), ms: m.by?.ms ?? ms } };
}

/** One open thread. Call `dispose` when leaving it. */
export class Conversation {
  /** Messages in the file, not counting a message still streaming. */
  count = $state(0);
  /** Bumped when a page arrives, so rows waiting on it redraw. Not bumped by streaming. */
  rev = $state(0);
  /** The running turn, or null: from the moment of the send (before the server has said anything) to the end of the turn. */
  stream = $state<Turn | null>(null);
  summary = $state<ThreadSummary>();

  /** The turn whose job stream just ended, until its saved message arrives (`onMessage`). */
  private ended: { turn: Turn; at: number } | null = null;
  private cache = new Map<number, Message>();
  private loading = new Set<number>();
  /** The rows last asked for, so rows that arrive later (the count grew) are read without waiting for a scroll. */
  private want: [number, number] = [0, 0];
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
        // The turn's own message ends the working row (and keeps what only this page saw of the turn: how long it worked, what it thought); the person's message does not.
        // (The job stream may end before the saved message arrives: the turn it ended is kept for a few seconds so its reasoning and duration still reach the message.)
        const turn = this.stream ?? (this.ended && Date.now() - this.ended.at < 15_000 ? this.ended.turn : null);
        if (m.role === "agent" && turn) { m = carry(m, turn); this.stream = null; this.ended = null; }
        this.cache.set(i, m);
        this.count = Math.max(this.count, i + 1);
        this.rev++;
      },
      onStreamStart: (id) => { const prev = this.stream; this.stream = { text: "", steps: [], phase: "Starting", startedAt: Date.now(), by: { provider: this.summary?.provider ?? "claude", model: this.summary?.model ?? "default" }, ...prev, id }; },
      onPhase: (phase) => { if (this.stream) this.stream.phase = phase; },
      onThought: (t) => { if (this.stream) this.stream.thought = { text: (this.stream.thought?.text ?? "") + t.text, ms: t.done ? t.ms ?? Date.now() - this.stream.startedAt : undefined }; },
      // Text is shown as it arrives only when the person turned streaming on (off by default): otherwise the working state and steps show, and the finished message appears at once.
      onDelta: (t) => {
        if (this.stream && appearance.prefs.streamText) this.stream.text += t;
      },
      onStep: (s) => {
        if (this.stream) this.stream.steps = [...this.stream.steps, s];
      },
      onSummary: (s) => { this.summary = s; if (s.messageCount > this.count) { this.count = s.messageCount; this.rev++; this.ensure(...this.want); } },
      onStreamEnd: () => { if (this.stream) this.ended = { turn: this.stream, at: Date.now() }; this.stream = null; },
    });
    // The list does not know a thread's message count: ask once (the fixtures answer at once, the server reads the file).
    void client.getThread(id).then((s) => { if (!s) return; this.summary = s; this.count = Math.max(this.count, s.messageCount); this.rev++; this.ensure(...this.want); });
  }

  /** A message if it has been read, else undefined (and the caller shows a placeholder until `rev` changes). Reads `rev` so the row redraws. */
  get(i: number): Message | undefined {
    void this.rev;
    return this.cache.get(i);
  }

  /** Makes sure the pages covering `[from, to)` are read or being read. */
  ensure(from: number, to: number): void {
    this.want = [from, to];
    for (let p = Math.floor(from / PAGE); p <= Math.floor((Math.min(to, this.count) - 1) / PAGE); p++) {
      if (this.loading.has(p)) continue;
      this.loading.add(p);
      void this.client.messages(this.id, p * PAGE, (p + 1) * PAGE).then((ms) => {
        ms.forEach((m, k) => this.cache.set(p * PAGE + k, m));
        this.rev++;
      });
    }
  }

  /** The person has sent: the working row shows at once ("Sending"), before the server has answered, and goes if the send is refused (`undo`). */
  begin(): () => void {
    if (this.stream) return () => {};
    const sm = this.summary;
    const turn: Turn = { id: "", text: "", steps: [], phase: "Sending", startedAt: Date.now(), by: { provider: sm?.provider ?? "claude", model: sm?.model ?? "default", reasoning: sm?.reasoning } };
    this.stream = turn;
    // (A `$state` proxy is not its target: the turn is told apart by its start, and a turn the server has already named is not taken back.)
    return () => { if (this.stream && this.stream.startedAt === turn.startedAt && !this.stream.id) this.stream = null; };
  }

  /** Stops watching. */
  dispose(): void {
    this.off();
  }
}
