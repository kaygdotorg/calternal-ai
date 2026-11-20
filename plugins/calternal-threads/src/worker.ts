import type { ItemViewRead, ItemRef, Timestamp } from "../generated/wire.js";
import { encodeContributionFrame } from "../generated/contribution-frame.mjs";
import { checkedText, type HarnessInput, type HarnessOutput } from "./harnesses-v1.ts";
import { agentParticipant, attest, checkedTitle, encode, folder, MAX_FILE_BYTES, messageId, messageRecord, page, personParticipant, step, threadHeader, messageView, type Thread } from "./format.ts";
import type { Context, RunInput, Send } from "./interface.ts";
import { appendRecord, inlineBody, kind, publish, read } from "./files.ts";
import { requireConversationWrite, type PeopleContext } from "./people.ts";

export { peopleHandlers } from "./people.ts";

function requireAgentThread(thread: Thread, ctx: Context) {
  if (thread.conversation === "people") throw new Error("This conversation requires an agent invitation.");
  const people = thread.participants.filter(participant => participant.kind === "person");
  if (people.length !== 1 || people[0].id !== ctx.author.actor) {
    throw new Error("Only the person who started this conversation can send to its agent.");
  }
}

async function checkSend(ctx: Context, item: ItemRef) {
  const head = await ctx.call("item_head", { item });
  const { thread } = await read(ctx, item, head.revision);
  requireAgentThread(thread, ctx);
}

async function create({ title }: { title: string }, ctx: Context) {
  const thread: Thread = {
    id: step(ctx.author.request_id, `thread-${ctx.author.actor}`),
    title: checkedTitle(title),
    created: ctx.author.authored_at,
    participants: [personParticipant(ctx.author.actor, ctx.author.actor_label)],
    messages: [],
  };
  const attested = title === thread.title ? attest(threadHeader(thread), { kind: "input" }) : undefined;
  const head = await ctx.call("item_create", {
    request_id: step(ctx.author.request_id, "create"),
    kind, folders: [folder(thread)], name: "Conversation.md",
    metadata: { title: thread.title },
    ...(attested ? { attested, attempt_discovery: "creator_only" } : { body: inlineBody(Buffer.from(encode(thread))) }),
  });
  await publish(ctx, head.item, head.revision, thread);
  return { created: head.item };
}

async function send({ item, text, harness, placement }: Send, ctx: Context) {
  await checkSend(ctx, item);
  return ctx.call("start_job", { job: "run", item, harness, placement, input: {}, contribution_input: { text: checkedText(text) } });
}

async function rename({ item, title }: { item: ItemRef; title: string }, ctx: PeopleContext) {
  const { revision } = await ctx.call("item_head", { item });
  const { thread } = await read(ctx, item, revision);
  if (thread.conversation === "people") await requireConversationWrite(ctx, item);
  const nextTitle = checkedTitle(title);
  if (thread.title === nextTitle) {
    await publish(ctx, item, revision, thread);
    return {};
  }
  thread.title = nextTitle;
  const saved = await ctx.call("item_save", {
    request_id: step(ctx.author.request_id, "rename"), item, expected: revision,
    patch: { title: thread.title }, body: inlineBody(Buffer.from(encode(thread))),
  });
  if ("code" in saved) throw new Error(saved.code);
  await publish(ctx, item, saved.revision, thread);
  return {};
}

async function run(input: RunInput, ctx: Context) {
  if (!ctx.job || ctx.job.item !== input.item) throw new Error("This agent run is unavailable.");
  if (ctx.alarm?.missed && input.if_missed === "skip") return {};
  const { item, harness } = ctx.job;
  const head = await ctx.call("item_head", { item });
  let { thread, size } = await read(ctx, item, head.revision);
  requireAgentThread(thread, ctx);
  const agent = agentParticipant(thread.id, ctx.job.contribution_identity);
  const person = personParticipant(ctx.author.actor, ctx.author.actor_label ?? thread.participants.find(p => p.id === ctx.author.actor)?.name);
  const promptId = messageId(ctx.author.request_id, `prompt-${ctx.author.actor}`);
  const pinnedInput = ctx.job.contribution_input as HarnessInput;
  const text = checkedText(pinnedInput.text);
  let revision = head.revision;
  let currentSize = size;
  if (!thread.messages.some(message => message.id === promptId)) {
    if (!thread.participants.some(participant => participant.id === agent.id) && thread.participants.length >= 64) {
      throw new Error("Thread participant limit reached.");
    }
    const prompt = { id: promptId, author: person, text, posted_at: ctx.author.authored_at };
    thread.messages.push(prompt);
    const record = messageRecord(prompt, thread.participants);
    // Reserve one reply and its bounded message metadata before admitting execution.
    if (size + Buffer.byteLength(record.prefix + record.text + record.suffix) + 18 * 1024 > MAX_FILE_BYTES) {
      throw new Error("Thread is full.");
    }
    const request_id = step(ctx.author.request_id, "prompt");
    const attested = ctx.alarm ? undefined : attest(record, { kind: "input" });
    const saved = await appendRecord(ctx, item, revision, request_id, size, thread, record, attested);
    revision = saved.revision;
    currentSize = saved.end_offset;
  }
  await publish(ctx, item, revision, thread);
  const attempt = await ctx.call("execution_start", { request_id: step(ctx.author.request_id, "start"), harness });
  const contribution = {
    request_id: step(ctx.author.request_id, "contribution"), execution: attempt.execution, contribution: harness,
    input: pinnedInput,
  };
  encodeContributionFrame(contribution);
  const { call } = await ctx.call("contribution_call", contribution);
  let after: number | undefined;
  while (true) {
    const { frames } = await ctx.call("contribution_read", { call, after, wait_ms: 1000 });
    for (const frame of frames) {
      if (frame.seq !== (after ?? 0) + 1) throw new Error("A reply from the agent arrived out of order.");
      after = frame.seq;
      if (frame.kind === "terminal") {
        if (frame.state !== "ok") throw new Error(`The agent run ended with status ${frame.state}.`);
        return {};
      }
      const id = messageId(ctx.author.request_id, `reply-${call}-${frame.seq}`);
      // Another actor may have renamed the thread while the contributor was running.
      const latest = await ctx.call("item_head", { item });
      if (latest.revision !== revision) {
        ({ thread, size } = await read(ctx, item, latest.revision));
        currentSize = size;
        revision = latest.revision;
      }
      if (thread.messages.some(message => message.id === id)) continue;
      // The contribution broker validates the data against both manifests.
      const output = frame.data as HarnessOutput;
      if (!thread.participants.some(participant => participant.id === agent.id)) thread.participants.push(agent);
      const message = { id, author: agent, text: checkedText(output.text), posted_at: ctx.author.authored_at, log: output.log };
      const record = messageRecord(message, thread.participants, true);
      const attested = attest(record, { kind: "output", seq: frame.seq });
      thread.messages.push(message);
      const saved = await appendRecord(ctx, item, revision, step(ctx.author.request_id, `append-${call}-${frame.seq}-${revision}`),
        currentSize, thread, record, attested);
      currentSize = saved.end_offset;
      revision = saved.revision;
      await publish(ctx, item, revision, thread);
    }
  }
}

async function arm(input: Omit<Send, "placement"> & { at: Timestamp; if_missed: "run" | "skip"; grace_seconds?: number }, ctx: Context) {
  const { item, harness, at, if_missed, grace_seconds } = input;
  await checkSend(ctx, item);
  return ctx.call("alarm_set", {
    request_id: step(ctx.author.request_id, "alarm"), job: "run", item, at, harness, grace_seconds,
    input: { if_missed }, contribution_input: { text: checkedText(input.text) },
  });
}

async function refresh(input: ItemViewRead, ctx: Context) {
  const { thread } = await read(ctx, input.item, input.source_revision);
  if (thread.conversation === "people") {
    const head = await ctx.call("item_head", { item: input.item });
    if (head.revision !== input.source_revision) {
      if (input.anchor !== undefined) return { anchor_unavailable: true as const };
      throw new Error("This conversation page needs a fresh revision.");
    }
  }
  if (input.anchor !== undefined && !thread.messages.some(message => message.id === input.anchor && !message.deleted)) {
    return { anchor_unavailable: true as const };
  }
  return page(thread, input.older, input.anchor);
}

async function message(input: { item: ItemRef; message: string }, ctx: Context) {
  const head = await ctx.call("item_head", { item: input.item });
  const { thread } = await read(ctx, input.item, head.revision);
  return messageView(thread, input.message);
}

/** Draft application handlers. Protected Stop and archive use host controls. */
export const handlers = {
  "threads.create": create, "threads.send": send, "threads.rename": rename,
  "threads.run": run, "threads.arm": arm, "threads.refresh": refresh, "threads.message": message,
};
