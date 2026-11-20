import type { ItemViewRead, ItemRef, Timestamp } from "../generated/wire.js";
import { checkedText } from "./harnesses-v1.ts";
import { checkedTitle, encode, encodeMessage, folder, MAX_FILE_BYTES, page, step, messageView, type Thread } from "./format.ts";
import type { Context, RunInput, Send } from "./interface.ts";
import { inlineBody, kind, publish, read } from "./files.ts";

export { peopleHandlers } from "./people.ts";

async function requireAgentThread(ctx: Context, item: ItemRef) {
  const head = await ctx.call("item_head", { item });
  const { thread } = await read(ctx, item, head.revision);
  if (thread.conversation === "people") throw new Error("This conversation requires an agent invitation.");
}

async function create({ title }: { title: string }, ctx: Context) {
  const thread: Thread = {
    id: step(ctx.request, `thread-${ctx.actor}`),
    title: checkedTitle(title),
    created: ctx.now,
    participants: [{ kind: "person", id: ctx.actor }],
    messages: [],
  };
  const head = await ctx.call("item_save", {
    request_id: step(ctx.request, "create"),
    kind, path: `Threads/${folder(thread)}/Conversation.md`,
    metadata: { title: thread.title }, bytes: inlineBody(Buffer.from(encode(thread))),
  });
  await publish(ctx, head.item, head.revision, thread);
  return { created: head.item };
}

async function send({ item, text, harness, placement }: Send, ctx: Context) {
  await requireAgentThread(ctx, item);
  return ctx.call("start_job", { job: "run", item, harness, placement, input: { item, text: checkedText(text) } });
}

async function rename({ item, title }: { item: ItemRef; title: string }, ctx: Context) {
  const { revision } = await ctx.call("item_head", { item });
  const { thread } = await read(ctx, item, revision);
  const nextTitle = checkedTitle(title);
  if (thread.title === nextTitle) {
    await publish(ctx, item, revision, thread);
    return {};
  }
  thread.title = nextTitle;
  const saved = await ctx.call("item_save", {
    request_id: step(ctx.request, "rename"), item, expected_revision: revision,
    metadata: { title: thread.title }, bytes: inlineBody(Buffer.from(encode(thread))),
  });
  await publish(ctx, item, saved.revision, thread);
  return {};
}

async function run(input: RunInput, ctx: Context) {
  if (!ctx.job || ctx.job.item !== input.item) throw new Error("No checked execution job.");
  if (ctx.alarm && input.if_missed === "skip" && Date.parse(ctx.alarm.actual_at) > Date.parse(ctx.alarm.scheduled_at)) return {};
  if (!ctx.harnessResult) throw new Error("This host does not expose a contributed harness result route.");
  const { item, harness } = ctx.job;
  const head = await ctx.call("item_head", { item });
  let { thread, size } = await read(ctx, item, head.revision);
  if (thread.conversation === "people") throw new Error("This conversation requires an agent invitation.");
  const agent = { kind: "agent" as const, id: harness };
  const person = { kind: "person" as const, id: ctx.actor };
  const promptId = step(ctx.request, `prompt-${ctx.actor}`);
  const replyId = step(ctx.request, `reply-${ctx.actor}`);
  if (thread.messages.some(message => message.id === replyId)) {
    await publish(ctx, item, head.revision, thread);
    return {};
  }
  const text = checkedText(input.text);
  let revision = head.revision;
  let currentSize = size;
  if (!thread.messages.some(message => message.id === promptId)) {
    const added = [person, agent].filter(participant => !thread.participants.some(saved =>
      saved.kind === participant.kind && saved.id === participant.id));
    if (thread.participants.length + added.length > 64) throw new Error("Thread participant limit reached.");
    thread.participants.push(...added);
    const prompt = { id: promptId, author: person, text };
    thread.messages.push(prompt);
    const body = Buffer.from(added.length ? encode(thread) : encodeMessage(prompt));
    currentSize = added.length ? body.length : size + body.length;
    // Reserve one reply and its bounded message metadata before admitting execution.
    if (currentSize + 18 * 1024 > MAX_FILE_BYTES) throw new Error("Thread is full.");
    const request_id = step(ctx.request, "prompt");
    const saved = added.length
      ? await ctx.call("item_save", {
          request_id, item, expected_revision: revision,
          metadata: { title: thread.title }, bytes: inlineBody(body),
        })
      : await ctx.call("item_append", {
          request_id, item, expected_revision: revision, bytes: body.toString("base64"),
        });
    revision = saved.revision;
  }
  await publish(ctx, item, revision, thread);
  const attempt = await ctx.call("execution_start", { request_id: step(ctx.request, "start"), harness });
  const result = await ctx.harnessResult(attempt.execution, { text });
  const message = { id: replyId, author: agent, text: checkedText(result.reply), log: result.log };
  // A rename can commit while the harness is running. Use its new head and
  // build the snapshot from that revision, without repeating the provider call.
  const latest = await ctx.call("item_head", { item });
  if (latest.revision !== revision) {
    ({ thread, size } = await read(ctx, item, latest.revision));
    currentSize = size;
    revision = latest.revision;
  }
  const entry = encodeMessage(message);
  if (currentSize + Buffer.byteLength(entry) > MAX_FILE_BYTES) throw new Error("Thread is full.");
  const saved = await ctx.call("item_append", {
    request_id: step(ctx.request, "reply"), item, expected_revision: revision, bytes: Buffer.from(entry).toString("base64"),
  });
  thread.messages.push(message);
  await publish(ctx, item, saved.revision, thread);
  return {};
}

async function arm(input: Omit<Send, "placement"> & { at: Timestamp; if_missed: "run" | "skip" }, ctx: Context) {
  const { item, harness, at, if_missed } = input;
  await requireAgentThread(ctx, item);
  return ctx.call("alarm_set", {
    request_id: step(ctx.request, "alarm"), job: "run", at, harness,
    input: { item, text: checkedText(input.text), if_missed },
  });
}

async function refresh(input: ItemViewRead, ctx: Context) {
  const { thread } = await read(ctx, input.item, input.source_revision);
  return page(thread, input.older);
}

async function message(input: { item: ItemRef; message: string }, ctx: Context) {
  const head = await ctx.call("item_head", { item: input.item });
  const { thread } = await read(ctx, input.item, head.revision);
  const found = thread.messages.find(entry => entry.id === input.message);
  if (!found) throw new Error("Message unavailable.");
  return messageView(found);
}

/** Draft application handlers. Protected Stop and archive use host controls. */
export const handlers = {
  "threads.create": create, "threads.send": send, "threads.rename": rename,
  "threads.run": run, "threads.arm": arm, "threads.refresh": refresh, "threads.message": message,
};
