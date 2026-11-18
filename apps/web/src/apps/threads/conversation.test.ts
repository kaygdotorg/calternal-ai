import { describe, expect, test } from "vitest";
import { Conversation } from "./conversation.svelte";
import type { ThreadEvents, ThreadsClient } from "./client";
import type { Message, ThreadSummary } from "./model";
import { byOf, parseMessages } from "../../server/thread-file";

const summary = { id: "t", spaceId: "s", title: "T", parentId: null, state: "idle", workspace: "", updatedAt: 0, provider: "claude", model: "default", permission: "approve", runsOn: { machine: "m", mode: "sandboxed" }, messageCount: 0 } as ThreadSummary;
/** A client that only lets the test reach the events the conversation registered. */
function double() {
  let ev!: ThreadEvents;
  const client = { watch: (_: string, e: ThreadEvents) => { ev = e; return () => {}; }, getThread: async () => summary, messages: async () => [] } as unknown as ThreadsClient;
  return { client, ev: () => ev };
}
const agent = (blocks: Message["blocks"], by?: Message["by"]): Message => ({ id: "a", role: "agent", at: 1, blocks, ...(by ? { by } : {}) });

describe("the running turn", () => {
  test("the working row is there from the send, keeps its start through the job, and the person's own message does not end it", () => {
    const d = double(); const c = new Conversation(d.client, "t", summary);
    c.begin();
    expect(c.stream).toMatchObject({ phase: "Sending", steps: [] });
    const startedAt = c.stream!.startedAt;
    d.ev().onMessage({ id: "u", role: "user", at: 1, blocks: [{ type: "text", text: "hi" }] }, 0);
    expect(c.stream).not.toBeNull();
    d.ev().onStreamStart("job-1"); d.ev().onPhase!("Thinking");
    expect(c.stream).toMatchObject({ id: "job-1", phase: "Thinking", startedAt });
  });
  test("a refused send takes the row away again", () => {
    const d = double(); const c = new Conversation(d.client, "t", summary);
    const undo = c.begin(); undo();
    expect(c.stream).toBeNull();
  });
  test("the finished message keeps what only this page saw: how long it worked and what it thought; what the file records wins", () => {
    const d = double(); const c = new Conversation(d.client, "t", summary);
    c.begin(); d.ev().onStreamStart("j"); d.ev().onThought!({ text: "plan" }); d.ev().onThought!({ text: "", done: true, ms: 4000 });
    d.ev().onMessage(agent([{ type: "steps", steps: [], ms: 0 }, { type: "text", text: "done" }]), 1);
    const m = c.get(1)!;
    expect(m.blocks.map((b) => b.type)).toEqual(["thought", "steps", "text"]);
    expect(m.blocks[0]).toMatchObject({ text: "plan", ms: 4000 });
    expect(m.by?.ms).toBeGreaterThanOrEqual(0);
    expect(c.stream).toBeNull();
    c.begin(); d.ev().onMessage(agent([{ type: "steps", steps: [], ms: 7000 }], { ms: 9000 }), 2);
    expect(c.get(2)!.blocks[0]).toMatchObject({ ms: 7000 }); expect(c.get(2)!.by?.ms).toBe(9000);
  });
});

describe("who wrote a message, from its marker", () => {
  test("provider, model, reasoning and duration are read; an older marker has none", () => {
    expect(byOf("turn:x actor:y time:2026-11-17T10:00:00.000Z provider:calternal-codex model:gpt-6-luna reasoning:high duration:12500")).toEqual({ provider: "codex", model: "gpt-6-luna", reasoning: "high", ms: 12500 });
    expect(byOf("turn:x actor:y time:2026-11-17T10:00:00.000Z")).toBeUndefined();
    expect(byOf("provider:someone-else")).toBeUndefined();
  });
  test("an assistant message carries it, and the duration fills its steps' time", () => {
    const md = `---\ntitle: "x"\n---\n\n<!-- message:019a8c00-0000-7000-8000-0000000000a1 role:agent turn:j actor:y time:2026-11-17T12:04:00.000Z provider:calternal-claude-code model:sonnet duration:5000 -->\n> Tool Bash · Run ls (started)\n\n> Tool Bash · Run ls (finished)\n\nhello\n<!-- /message -->\n`;
    const [m] = parseMessages(md);
    expect(m!.by).toEqual({ provider: "claude", model: "sonnet", ms: 5000 });
    expect(m!.blocks[0]).toMatchObject({ type: "steps", ms: 5000 });
  });
});
