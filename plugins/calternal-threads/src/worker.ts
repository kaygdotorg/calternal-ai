import type { ItemViewRead, ItemRef, Timestamp } from "../generated/wire.js";
import { encodeContributionFrame } from "../generated/contribution-frame.mjs";
import { checkedText, type HarnessInput, type HarnessOutput } from "./harnesses-v1.ts";
import { agentParticipant, attest, checkedTitle, encode, MAX_FILE_BYTES, messageId, messageRecord, personParticipant, step, threadHeader, messageView, type Thread } from "./format.ts";
import type { Context, RunInput, Send } from "./interface.ts";
import { appendRecord, conversationPage, inlineBody, kind, prepareFiles, publish, read, readConversation } from "./files.ts";
import { legacyMessage, legacyPage } from "./legacy.ts";

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
    kind, name: "Conversation.md",
    metadata: { title: thread.title },
    ...(attested ? { attested, attempt_discovery: "creator_only" } : { body: inlineBody(Buffer.from(encode(thread))) }),
  });
  await publish(ctx, head.item, head.revision, thread);
  return { created: head.item };
}

async function send({ item, text, harness, placement, attachments = [] }: Send, ctx: Context) {
  await checkSend(ctx, item);
  const input = { job: "run", item, harness, placement, input: {}, contribution_input: { text: checkedText(text) } };
  if (!attachments.length) return ctx.call("start_job", input);
  await prepareFiles(ctx, item, attachments);
  return ctx.attachments!.startJob(input, attachments);
}

async function run(input: RunInput, ctx: Context) {
  if (!ctx.job || ctx.job.item !== input.item) throw new Error("This agent run is unavailable.");
  if (ctx.alarm?.missed && input.if_missed === "skip") return {};
  const { item, harness } = ctx.job;
  const head = await ctx.call("item_head", { item });
  let { thread, size, spans } = await read(ctx, item, head.revision);
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
    const staged = ctx.attachments?.jobFiles;
    const files = staged?.length ? await prepareFiles(ctx, item, staged) : undefined;
    const prompt = { id: promptId, author: person, text, posted_at: ctx.author.authored_at, ...(files && { files }) };
    thread.messages.push(prompt);
    const record = messageRecord(prompt, thread.participants);
    // Reserve a reply, its quote of this prompt and bounded message metadata.
    const quoteBytes = Buffer.byteLength(text) + 2 * text.split("\n").length + 2;
    if (size + Buffer.byteLength(record.prefix + record.text + record.suffix) + quoteBytes + 18 * 1024 > MAX_FILE_BYTES) {
      throw new Error("Thread is full.");
    }
    const request_id = step(ctx.author.request_id, "prompt");
    const attested = ctx.alarm ? undefined : attest(record, { kind: "input" });
    const saved = await appendRecord(ctx, item, revision, request_id, size, thread, record, attested, staged);
    revision = saved.revision;
    currentSize = saved.end_offset;
    if ("span" in saved && saved.span) spans.set(promptId, saved.span);
    else if (!attested) spans.clear();
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
        ({ thread, size, spans } = await read(ctx, item, latest.revision));
        currentSize = size;
        revision = latest.revision;
      }
      if (thread.messages.some(message => message.id === id)) continue;
      // The contribution broker validates the data against both manifests.
      const output = frame.data as HarnessOutput;
      if (!thread.participants.some(participant => participant.id === agent.id)) thread.participants.push(agent);
      const prompt = thread.messages.find(entry => entry.id === promptId && !entry.deleted);
      const message = { id, author: agent, text: checkedText(output.text), posted_at: ctx.author.authored_at, log: output.log,
        ...(prompt && { replyTo: prompt.id, replyToName: prompt.author.name, replyQuote: text }) };
      const record = messageRecord(message, thread.participants, true);
      const promptSpan = prompt?.text === text ? spans.get(promptId) : undefined;
      const quote = promptSpan === undefined ? undefined : { item, revision, span: promptSpan };
      const attested = attest(record, { kind: "output", seq: frame.seq }, quote);
      thread.messages.push(message);
      const saved = await appendRecord(ctx, item, revision, step(ctx.author.request_id, `append-${call}-${frame.seq}-${revision}`),
        currentSize, thread, record, attested);
      currentSize = saved.end_offset;
      revision = saved.revision;
      if (!attested) spans.clear();
      await publish(ctx, item, revision, thread);
    }
  }
}

const defaultGraceSeconds = 60;

async function arm(input: Omit<Send, "placement" | "attachments"> & { at: Timestamp; if_missed: "run" | "skip"; grace_seconds?: number }, ctx: Context) {
  const { item, harness, at, if_missed, grace_seconds = defaultGraceSeconds } = input;
  await checkSend(ctx, item);
  return ctx.call("alarm_set", {
    request_id: step(ctx.author.request_id, "alarm"), job: "run", item, at, harness, grace_seconds,
    input: { if_missed }, contribution_input: { text: checkedText(input.text) },
  });
}

async function refresh(input: ItemViewRead, ctx: Context) {
  const saved = await readConversation(ctx, input.item, input.source_revision);
  if (saved.legacy) return legacyPage(saved.thread, input.older, input.anchor);
  const { thread } = saved;
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
  return conversationPage(ctx, input.item, input.source_revision, thread, input.older, input.anchor);
}

async function message(input: { item: ItemRef; message: string; offset?: number }, ctx: Context) {
  const head = await ctx.call("item_head", { item: input.item });
  const saved = await readConversation(ctx, input.item, head.revision);
  if (saved.legacy) return legacyMessage(saved.thread, input.message, input.offset);
  if (input.offset !== undefined) throw new Error("Message paging is only available for version 1 threads.");
  return messageView(saved.thread, input.message);
}

/** Draft application handlers. Protected Stop and archive use host controls. */
export const handlers = {
  "threads.create": create, "threads.send": send,
  "threads.run": run, "threads.arm": arm, "threads.refresh": refresh, "threads.message": message,
};
