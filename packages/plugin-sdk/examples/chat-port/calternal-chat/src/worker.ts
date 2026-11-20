import { array, defineOperation, handlersOf, integer, LIMITS, literal, object, oneOf, oneOfValues, ref, streamContribution, string, type OperationContext } from "@calternal/plugin-sdk";
import type { ItemRef } from "@calternal/plugin-sdk/types";
import { checkedText, textSchema, type HarnessInput, type HarnessOutput } from "./harnesses-v1.ts";
import { agentParticipant, attest, checkedTitle, encode, MAX_FILE_BYTES, messageId, messageRecord, personParticipant, threadHeader, threadShape, messageView, type Thread } from "./format.ts";
import { viewerPatchSchema, viewerState } from "./viewer.ts";
import { ports } from "./interface.ts";
import { appendRecord, conversationPage, inlineBody, kind, prepareFiles, publish, read, readConversation } from "./files.ts";
import { legacyMessage, legacyPage } from "./legacy.ts";

export { peopleHandlers } from "./people.ts";

const item = ref("ItemRef");
const title = string({ minLength: 1, maxLength: 120 });
const ifMissed = oneOfValues("run", "skip");
const writeView = ["item_view_context", "item_view_publish"] as const;
const readBody = ["item_head", "item_range", "item_spans"] as const;
const write = { required_capability: "content.write", risk: "reversible", idempotency: "request_id" } as const;
const read_ = { required_capability: "space.read", risk: "read", idempotency: "read_only" } as const;
const execute = { required_capability: "plugins.use", risk: "external", idempotency: "request_id" } as const;
const messageIdPattern = "^(?:[0-9a-f]{16}|[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12})$";
const messageOffset = integer({ minimum: 0, maximum: 2 * 1024 * 1024 });
const author = object({ kind: oneOfValues("person", "agent"), id: string(), name: string(), contribution: ref("ContributionIdentity") }, "kind", "id", "name");
const message = object({
  id: string(), author, text: textSchema, posted_at: ref("Timestamp"),
  log: ref("ItemLocator"), consultation: ref("ItemLocator"), edited: literal(true), replyTo: string(), replyToName: string(),
  replyIn: ref("ItemLocator"), replyQuote: textSchema,
  askedBy: object({ kind: literal("person"), id: string(), name: string() }, "kind", "id", "name"),
  mentions: array(ref("AccountRef")),
}, "id", "author", "text", "posted_at");
const legacyMessageResult = object({
  id: string(), role: oneOfValues("person", "agent", "automation"), heading: string(),
  text: string({ maxLength: 8000 }), posted_at: ref("Timestamp"), turn: string(), next_offset: messageOffset,
}, "id", "role", "heading", "text", "posted_at");

type Reader = OperationContext<"item_head" | "item_range" | "item_spans">;

function requireAgentThread(thread: Thread, ctx: OperationContext<never>) {
  if (thread.conversation === "people") throw new Error("This conversation requires an agent invitation.");
  const people = thread.participants.filter(participant => participant.kind === "person");
  if (people.length !== 1 || people[0].id !== ctx.author.actor) {
    throw new Error("Only the person who started this conversation can send to its agent.");
  }
}

async function checkSend(ctx: Reader, item: ItemRef) {
  const head = await ctx.call("item_head", { item });
  const { thread } = await read(ctx, item, head.revision);
  requireAgentThread(thread, ctx);
}

const create = defineOperation({
  ...write, id: "chat.create", title: "New chat", creates: "chats", methods: ["item_create", ...writeView],
  write_origins: { host_input: "title" }, input_schema: object({ title }, "title"), output_schema: object({ created: item }, "created"),
  async handler({ title }, ctx) {
    const thread: Thread = {
      id: ctx.key(`thread-${ctx.author.actor}`), title: checkedTitle(title), created: ctx.author.authored_at,
      participants: [personParticipant(ctx.author.actor, ctx.author.actor_label)], messages: [],
    };
    const attested = title === thread.title ? attest(threadHeader(thread), { kind: "input" }) : undefined;
    const head = await ctx.call("item_create", {
      request_id: ctx.key("create"), kind, name: "Conversation.md",
      metadata: { title: thread.title, shape: threadShape(thread) },
      ...(attested ? { attested, attempt_discovery: "creator_only" } : { body: inlineBody(Buffer.from(encode(thread))) }),
    });
    await publish(ctx, head.item, head.revision, thread);
    return { created: head.item };
  },
});

const send = defineOperation({
  ...execute, id: "chat.send", title: "Send", target: "chats", methods: [...readBody, "start_job"],
  material_inputs: { placement: "Machine and login" }, write_origins: { host_input: "text" },
  input_schema: object({ item, text: textSchema, harness: ref("HarnessRef"), placement: ref("ChoiceRef") }, "item", "text", "harness", "placement"),
  output_schema: ref("ExecutionJobReceipt"),
  async handler(input, ctx) {
    const { item, text, harness, placement } = input;
    // Proposed attachment selection (#78); not in the 14.44 input schema.
    const { attachments = [] } = input as typeof input & { attachments?: readonly string[] };
    await checkSend(ctx, item);
    const job = { job: "run", item, harness, placement, input: {}, contribution_input: { text: checkedText(text) } };
    if (!attachments.length) return ctx.call("start_job", job);
    await prepareFiles(ctx, item, attachments);
    return ports(ctx).attachments!.startJob(job, attachments);
  },
});

const run = defineOperation({
  ...execute, id: "chat.run", title: "Run agent", target: "chats", write_origins: { attempt_output: "text" },
  methods: [...readBody, "item_append", "item_save", "item_save_begin", "item_save_chunk", "item_save_seal", "execution_start",
    "contribution_call", "contribution_read", ...writeView],
  input_schema: object({ item, if_missed: ifMissed }, "item"), output_schema: object({}),
  async handler(input, ctx) {
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
      const staged = ports(ctx).attachments?.jobFiles;
      const files = staged?.length ? await prepareFiles(ctx, item, staged) : undefined;
      const prompt = { id: promptId, author: person, text, posted_at: ctx.author.authored_at, ...(files && { files }) };
      thread.messages.push(prompt);
      const record = messageRecord(prompt, thread.participants);
      // Reserve a reply, its quote of this prompt and bounded message metadata.
      const quoteBytes = Buffer.byteLength(text) + 2 * text.split("\n").length + 2;
      if (size + Buffer.byteLength(record.prefix + record.text + record.suffix) + quoteBytes + 18 * 1024 > MAX_FILE_BYTES) {
        throw new Error("Thread is full.");
      }
      const attested = ctx.alarm ? undefined : attest(record, { kind: "input" });
      const saved = await appendRecord(ctx, item, revision, "prompt", size, thread, record, attested, staged);
      revision = saved.revision;
      currentSize = saved.end_offset;
      if ("span" in saved && saved.span) spans.set(promptId, saved.span);
      else if (!attested) spans.clear();
    }
    await publish(ctx, item, revision, thread);
    const attempt = await ctx.call("execution_start", { request_id: ctx.key("start"), harness });
    const replies = streamContribution(ctx, { purpose: "contribution", execution: attempt.execution, contribution: harness, input: pinnedInput });
    for await (const frame of replies) {
      const id = messageId(ctx.author.request_id, `reply-${frame.call}-${frame.seq}`);
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
      const saved = await appendRecord(ctx, item, revision, `append-${frame.call}-${frame.seq}-${revision}`, currentSize, thread, record, attested);
      currentSize = saved.end_offset;
      revision = saved.revision;
      if (!attested) spans.clear();
      await publish(ctx, item, revision, thread);
    }
    return {};
  },
});

const arm = defineOperation({
  ...execute, id: "chat.arm", title: "Schedule a run", target: "chats", methods: [...readBody, "alarm_set"],
  input_schema: object({ item, text: textSchema, harness: ref("HarnessRef"), at: ref("Timestamp"), if_missed: ifMissed,
    grace_seconds: integer({ minimum: 0, maximum: 86400 }) }, "item", "text", "harness", "at", "if_missed"),
  output_schema: ref("AlarmReceipt"),
  async handler({ item, harness, at, if_missed, grace_seconds, text }, ctx) {
    await checkSend(ctx, item);
    return ctx.call("alarm_set", {
      request_id: ctx.key("alarm"), job: "run", item, at, harness, grace_seconds: grace_seconds ?? LIMITS.ALARM_GRACE_SECONDS_DEFAULT,
      input: { if_missed }, contribution_input: { text: checkedText(text) },
    });
  },
});

const state = defineOperation({
  ...write, required_capability: "space.read", id: "chat.state", title: "Update my thread preferences", target: "chats",
  methods: ["viewer_state_read", "viewer_state_write"],
  input_schema: object({ item, ...viewerPatchSchema }, "item"), output_schema: ref("ViewerItemState"),
  handler: viewerState,
});

const refresh = defineOperation({
  ...read_, id: "chat.refresh", title: "Read thread page", target: "chats", methods: [...readBody],
  input_schema: ref("ItemViewRead"), output_schema: ref("ItemViewPage"),
  async handler(input, ctx) {
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
  },
});

const readMessage = defineOperation({
  ...read_, id: "chat.message", title: "Read message", target: "chats", methods: [...readBody],
  input_schema: object({ item, message: string({ pattern: messageIdPattern }), offset: messageOffset }, "item", "message"),
  output_schema: oneOf(message, legacyMessageResult),
  async handler(input, ctx) {
    const head = await ctx.call("item_head", { item: input.item });
    const saved = await readConversation(ctx, input.item, head.revision);
    if (saved.legacy) return legacyMessage(saved.thread, input.message, input.offset);
    if (input.offset !== undefined) throw new Error("Message paging is only available for version 1 chats.");
    return messageView(saved.thread, input.message);
  },
});

/** Every declared Chat operation. Protected Stop and archive use host controls. */
export const operations = [create, send, run, arm, state, refresh, readMessage];

/** Handlers by operation ID for the worker runtime. */
export const handlers = handlersOf(operations);
