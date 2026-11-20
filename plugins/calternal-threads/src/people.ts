import type { AccountRef, ContributionIdentity, ItemRef, RevisionRef } from "../generated/wire.js";
import { inlineBody, kind, publish, read } from "./files.ts";
import { agentParticipant, checkedTitle, encode, encodeMessage, folder, MAX_FILE_BYTES, messageView, step, type Message, type Thread } from "./format.ts";
import { checkedText, type HarnessOutput } from "./harnesses-v1.ts";
import type { Context } from "./interface.ts";

type Target = { item: ItemRef };
type MessageTarget = Target & { message: string };
export type Scope = { message: string } | { from: string; through: string } | { whole: true };
type Invitation = Target & { revision: RevisionRef; inviter: AccountRef; contribution_identity: ContributionIdentity; scope: Scope; consent: readonly string[] };
export type InvitedInput = { prompt: string; messages: { id: string; text: string }[] };

export type PeopleContext = Context & {
  // Fixture ports for checked participants, account-private storage and invocation.
  checkedPeople?: readonly AccountRef[];
  privateState?: {
    read(item: ItemRef): Promise<string | undefined>;
    write(item: ItemRef, body: string, expected: string | undefined): Promise<void>;
  };
  invitation?: Invitation;
  runInvited?: (grant: Invitation, input: InvitedInput) => Promise<HarnessOutput>;
};

type State = { readThrough?: string; draft: string; muted: boolean };

async function member(ctx: Context, item: ItemRef) {
  const { revision } = await ctx.call("item_head", { item });
  const { thread, size } = await read(ctx, item, revision);
  if (thread.conversation !== "people" || !thread.participants.some(p => p.kind === "person" && p.id === ctx.author.actor)) {
    throw new Error("Conversation unavailable.");
  }
  return { thread, revision, size };
}

function find(thread: Thread, id: string) {
  const found = thread.messages.find(message => message.id === id);
  if (!found) throw new Error("Message unavailable.");
  return found;
}

async function save(ctx: Context, item: ItemRef, revision: RevisionRef, thread: Thread, purpose: string) {
  const saved = await ctx.call("item_save", {
    request_id: step(ctx.author.request_id, purpose), item, expected: revision,
    patch: { title: thread.title }, body: inlineBody(Buffer.from(encode(thread))),
  });
  if ("code" in saved) throw new Error(saved.code);
  await publish(ctx, item, saved.revision, thread);
}

async function create(input: { title: string; participants: AccountRef[] }, ctx: PeopleContext) {
  const ids = [...new Set([ctx.author.actor, ...input.participants])];
  if (ids.length < 2 || ids.length > 64 || ids.some(id => id !== ctx.author.actor && !ctx.checkedPeople?.includes(id))) {
    throw new Error("Participants unavailable.");
  }
  const thread: Thread = {
    id: step(ctx.author.request_id, `thread-${ctx.author.actor}`),
    title: checkedTitle(input.title), created: ctx.author.authored_at, conversation: "people",
    participants: ids.map(id => ({ kind: "person", id })), messages: [],
  };
  const head = await ctx.call("item_create", {
    request_id: step(ctx.author.request_id, "people-create"), kind, folders: [folder(thread)], name: "Conversation.md", attempt_discovery: "consent_only",
    metadata: { title: thread.title }, body: inlineBody(Buffer.from(encode(thread))),
  });
  await publish(ctx, head.item, head.revision, thread);
  return { created: head.item };
}

async function post({ item, text }: Target & { text: string }, ctx: PeopleContext) {
  const { thread, revision, size } = await member(ctx, item);
  const id = step(ctx.author.request_id, `person-${ctx.author.actor}`);
  const existing = thread.messages.find(message => message.id === id);
  if (existing) {
    if (!existing.edited && !existing.deleted && existing.text !== text) throw new Error("Conflicting replay.");
    await publish(ctx, item, revision, thread);
    return { message: id };
  }
  const message = { id, author: { kind: "person" as const, id: ctx.author.actor }, text: checkedText(text) };
  const bytes = Buffer.from(encodeMessage(message));
  if (size + bytes.length > MAX_FILE_BYTES) throw new Error("Thread is full.");
  const saved = await ctx.call("item_append", {
    request_id: step(ctx.author.request_id, "person-post"), item, expected_revision: revision, bytes: bytes.toString("base64"),
  });
  thread.messages.push(message);
  await publish(ctx, item, saved.revision, thread);
  return { message: id };
}

async function message({ item, message }: MessageTarget, ctx: PeopleContext) {
  const { thread } = await member(ctx, item);
  return messageView(find(thread, message));
}

async function change({ item, message }: MessageTarget, ctx: PeopleContext, text?: string) {
  const { thread, revision } = await member(ctx, item);
  const entry = find(thread, message);
  if (entry.author.kind !== "person" || entry.author.id !== ctx.author.actor || (entry.deleted && text !== undefined)) {
    throw new Error("Message unavailable.");
  }
  if (entry.deleted || entry.text === text) {
    await publish(ctx, item, revision, thread);
    return;
  }
  if (text === undefined) {
    entry.text = "";
    entry.deleted = true;
    delete entry.edited;
  } else {
    entry.text = checkedText(text);
    entry.edited = true;
  }
  await save(ctx, item, revision, thread, text === undefined ? "person-delete" : "person-edit");
}

function stateOf(body: string | undefined): State {
  if (body === undefined) return { draft: "", muted: false };
  const fields = /^---\nread-through: (.*)\ndraft: (.*)\nmuted: (true|false)\n---\n$/u.exec(body);
  if (!fields) throw new Error("Unreadable private thread state.");
  const readThrough = JSON.parse(fields[1]);
  return { ...(readThrough && { readThrough }), draft: JSON.parse(fields[2]), muted: fields[3] === "true" };
}

async function getState({ item }: Target, ctx: PeopleContext) {
  await member(ctx, item);
  if (!ctx.privateState) throw new Error("Private state unavailable.");
  return stateOf(await ctx.privateState.read(item));
}

async function setState({ item, draft, muted, readThrough }: Target & State, ctx: PeopleContext) {
  const { thread } = await member(ctx, item);
  if (!ctx.privateState) throw new Error("Private state unavailable.");
  if (Buffer.byteLength(draft) > 16 * 1024) throw new Error("Draft exceeds 16 KiB.");
  const body = await ctx.privateState.read(item);
  const prior = stateOf(body);
  const next = readThrough !== undefined ? thread.messages.findIndex(message => message.id === readThrough) : -1;
  if (readThrough !== undefined && next < 0) throw new Error("Message unavailable.");
  const last = thread.messages.findIndex(message => message.id === prior.readThrough);
  const anchor = JSON.stringify(thread.messages[Math.max(last, next)]?.id ?? null);
  await ctx.privateState.write(item, `---\nread-through: ${anchor}\ndraft: ${JSON.stringify(draft)}\nmuted: ${muted}\n---\n`, body);
}

function scoped(messages: Message[], scope: Scope) {
  if ("whole" in scope) return messages;
  const from = "message" in scope ? scope.message : scope.from;
  const through = "message" in scope ? scope.message : scope.through;
  const start = messages.findIndex(entry => entry.id === from);
  const end = messages.findIndex(entry => entry.id === through);
  if (start < 0 || end < start) throw new Error("Message scope unavailable.");
  return messages.slice(start, end + 1);
}

async function callAgent({ item, prompt }: Target & { prompt: string }, ctx: PeopleContext) {
  const grant = ctx.invitation;
  if (!grant || !ctx.runInvited || grant.item !== item || grant.inviter !== ctx.author.actor) {
    throw new Error("A checked agent invitation is required.");
  }
  const { thread, revision } = await member(ctx, item);
  if (grant.revision !== revision) throw new Error("The invitation needs a current revision.");
  const readable = scoped(thread.messages, grant.scope)
    .filter(message => !message.deleted && message.author.kind === "person"
      && (message.author.id === ctx.author.actor || grant.consent.includes(message.id)))
    .map(({ id, text }) => ({ id, text }));
  if (!readable.length) throw new Error("There are no readable messages in this invitation.");
  const payload = { prompt: checkedText(prompt), messages: readable };
  if (Buffer.byteLength(JSON.stringify(payload)) > 16 * 1024) throw new Error("The invitation exceeds 16 KiB.");

  const agent = agentParticipant(thread.id, grant.contribution_identity);
  if (!thread.participants.some(p => p.kind === agent.kind && p.id === agent.id)) {
    if (thread.participants.length >= 64) throw new Error("Thread participant limit reached.");
    thread.participants.push(agent);
  }
  const id = step(ctx.author.request_id, `invited-${ctx.author.actor}`);
  // Refuse before dispatch when the fixture cannot save the reply inline.
  inlineBody(Buffer.concat([Buffer.from(encode(thread)), Buffer.alloc(18 * 1024)]));
  const result = await ctx.runInvited(grant, payload);
  thread.messages.push({ id, author: agent, text: checkedText(result.text), log: result.log });
  await save(ctx, item, revision, thread, "invited-reply");
  return { message: id };
}

/** Fixture-only entry points for people threads pending shared custody and consent APIs. */
export const peopleHandlers = {
  create, post, message, getState, setState, callAgent,
  edit: (input: MessageTarget & { text: string }, ctx: PeopleContext) => change(input, ctx, input.text),
  remove: (input: MessageTarget, ctx: PeopleContext) => change(input, ctx),
};
