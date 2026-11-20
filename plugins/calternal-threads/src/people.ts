import type { AccountRef, ContributionIdentity, ItemContentReceipt, ItemCreate, ItemLocator, ItemRef, RevisionRef } from "../generated/wire.js";
import { appendRecord, inlineBody, kind, prepareFiles, publish, read, saveOpaque } from "./files.ts";
import { agentParticipant, attest, checkedTitle, encode, MAX_FILE_BYTES, messageId, messageRecord, messageView, personParticipant, step, type Message, type Thread } from "./format.ts";
import { checkedText, type HarnessOutput } from "./harnesses-v1.ts";
import type { Context } from "./interface.ts";

type Target = { item: ItemRef };
type MessageTarget = Target & { message: string };
export type Scope = { message: string } | { from: string; through: string } | { whole: true };
type Invitation = Target & { revision: RevisionRef; inviter: AccountRef; contribution_identity: ContributionIdentity; scope: Scope; call: string };
export type InvitedInput = { prompt: string; messages: { id: string; text: string }[]; tools: true };
export type ConsultationTurn = { role: "lead" | "consultant" | "summary"; text: string };
type Consultation = {
  lead: ContributionIdentity;
  consultant: ContributionIdentity;
  signal: AbortSignal;
  run(prompt: string): AsyncIterable<ConsultationTurn>;
};

export type PeopleContext = Context & {
  // Fixture port for the host account directory (ADR 0060). An omitted filter lists picker candidates.
  visiblePeople?: (ids?: readonly AccountRef[]) => Promise<readonly { id: AccountRef; name: string }[]>;
  // The host rechecks current visibility when committing named grants (ADRs 0058 and 0060).
  createShared?: (input: ItemCreate, people: readonly AccountRef[]) => Promise<ItemContentReceipt>;
  // Checks current common membership using the host's item grants and account directory.
  canWriteConversation?: (item: ItemRef) => Promise<boolean>;
  // Fixture binding to the captured composer or editor text.
  messageWrite?: { item: ItemRef; text: string };
  privateState?: {
    read(item: ItemRef): Promise<string | undefined>;
    write(item: ItemRef, body: string, expected: string | undefined): Promise<void>;
  };
  privatePreferences?: {
    read(): Promise<string | undefined>;
    write(body: string, expected: string | undefined): Promise<void>;
  };
  invitation?: Invitation;
  canDisclose?: (grant: Invitation, author: AccountRef) => boolean;
  waitForConsent?: (grant: Invitation, authors: readonly AccountRef[], seconds: number) => Promise<void>;
  consultation?: Consultation;
  runInvited?: (grant: Invitation, input: InvitedInput) => Promise<HarnessOutput>;
};

type State = { readThrough?: string; draft: string; muted: boolean };
type Preferences = {
  receiptsAndTyping: boolean;
  push: { enabled: boolean; iosLocked: boolean; iosUnlocked: boolean };
};
const editWindowMs = 48 * 60 * 60 * 1000;
const consentWaitSeconds = 60;

async function member(ctx: Context, item: ItemRef) {
  const { revision, locator } = await ctx.call("item_head", { item });
  const { thread, size, spans, quotes } = await read(ctx, item, revision);
  if (thread.conversation !== "people" || !thread.participants.some(p => p.kind === "person" && p.id === ctx.author.actor)) {
    throw new Error("Conversation unavailable.");
  }
  return { thread, revision, size, locator, spans, quotes };
}

/** Refuse shared mutations and agent calls unless the host confirms current common group membership. */
export async function requireConversationWrite(ctx: PeopleContext, item: ItemRef) {
  if (!await ctx.canWriteConversation?.(item)) throw new Error("Conversation is read-only.");
}

async function writableMember(ctx: PeopleContext, item: ItemRef) {
  const current = await member(ctx, item);
  await requireConversationWrite(ctx, item);
  return current;
}

function find(thread: Thread, id: string) {
  const found = thread.messages.find(message => message.id === id);
  if (!found) throw new Error("Message unavailable.");
  return found;
}

async function save(ctx: Context, item: ItemRef, revision: RevisionRef, thread: Thread, purpose: string) {
  const saved = await saveOpaque(ctx, {
    request_id: step(ctx.author.request_id, purpose), item, expected: revision, patch: { title: thread.title },
  }, Buffer.from(encode(thread)));
  await publish(ctx, item, saved.revision, thread);
}

async function create(input: { title: string; participants: AccountRef[] }, ctx: PeopleContext) {
  const ids = [...new Set([ctx.author.actor, ...input.participants])];
  if (ids.length < 2 || ids.length > 64) throw new Error("Participants unavailable.");
  if (!ctx.visiblePeople || !ctx.createShared) throw new Error("Shared conversation creation unavailable.");
  const names = new Map((await ctx.visiblePeople(ids)).map(({ id, name }) => [id, name]));
  if (ids.some(id => !names.has(id))) throw new Error("Participants unavailable.");
  const thread: Thread = {
    id: step(ctx.author.request_id, `thread-${ctx.author.actor}`),
    title: checkedTitle(input.title), created: ctx.author.authored_at, conversation: "people",
    participants: ids.map(id => personParticipant(id, (id === ctx.author.actor ? ctx.author.actor_label : undefined) ?? names.get(id))), messages: [],
  };
  const head = await ctx.createShared({
    request_id: step(ctx.author.request_id, "people-create"), kind, name: "Conversation.md", attempt_discovery: "consent_only",
    metadata: { title: thread.title }, body: inlineBody(Buffer.from(encode(thread))),
  }, ids);
  await publish(ctx, head.item, head.revision, thread);
  return { created: head.item };
}

async function post({ item, text, replyTo, replyIn, mentions = [], attachments = [] }: Target & {
  text: string; replyTo?: string; replyIn?: ItemRef; mentions?: AccountRef[]; attachments?: readonly string[];
}, ctx: PeopleContext) {
  const destination = await writableMember(ctx, item);
  const { thread, revision, size, locator } = destination;
  const external = replyIn !== undefined && replyIn !== item;
  if (external && replyTo === undefined) throw new Error("Choose a message to quote.");
  const origin = external ? await member(ctx, replyIn) : destination;
  if (origin.locator.space !== locator.space) throw new Error("Quotes must come from the same space.");
  const files = attachments.length ? await prepareFiles(ctx, item, attachments) : undefined;
  const id = messageId(ctx.author.request_id, `person-${ctx.author.actor}`);
  const recipients = [...new Set(mentions)].sort();
  if (recipients.some(id => !thread.participants.some(person => person.kind === "person" && person.id === id))) {
    throw new Error("Mention unavailable.");
  }
  const existing = thread.messages.find(message => message.id === id);
  if (existing) {
    if (!existing.edited && !existing.deleted && (
      existing.text !== text || existing.replyTo !== replyTo || existing.replyIn?.id !== (external ? origin.locator.id : undefined) ||
      JSON.stringify(existing.files) !== JSON.stringify(files) || JSON.stringify(existing.mentions ?? []) !== JSON.stringify(recipients)
    )) throw new Error("Conflicting replay.");
    await publish(ctx, item, revision, thread);
    return { message: id };
  }
  const target = replyTo === undefined ? undefined : messageView(origin.thread, replyTo);
  const sender = thread.participants.find(person => person.id === ctx.author.actor)!;
  const message = {
    id, author: personParticipant(ctx.author.actor, ctx.author.actor_label ?? sender.name), text: checkedText(text), posted_at: ctx.author.authored_at,
    ...(target && { replyTo, replyToName: target.author.name, replyQuote: checkedText(target.text) }),
    ...(external && { replyIn: origin.locator }), ...(files && { files }), ...(recipients.length && { mentions: recipients }),
  };
  const record = messageRecord(message, thread.participants);
  const captured = ctx.messageWrite;
  const sourceSpan = replyTo === undefined ? undefined : origin.spans.get(replyTo);
  const quote = sourceSpan === undefined ? undefined : { item: replyIn ?? item, revision: origin.revision, span: sourceSpan };
  const attested = captured?.item === item && captured.text === text ? attest(record, { kind: "input" }, quote) : undefined;
  if (external && !attested) throw new Error("A quote from another thread requires a checked source and a checked reply.");
  thread.messages.push(message);
  const saved = await appendRecord(ctx, item, revision, step(ctx.author.request_id, "person-post"), size, thread, record, attested, attachments);
  await publish(ctx, item, saved.revision, thread);
  return { message: id };
}

async function message({ item, message }: MessageTarget, ctx: PeopleContext) {
  const { thread } = await member(ctx, item);
  return messageView(thread, message);
}

async function change({ item, message }: MessageTarget, ctx: PeopleContext, text?: string, requote = false) {
  const { thread, revision, spans, quotes } = await writableMember(ctx, item);
  const entry = find(thread, message);
  if (entry.author.kind !== "person" || entry.author.id !== ctx.author.actor || (entry.deleted && text !== undefined)) {
    throw new Error("Message unavailable.");
  }
  if (text !== undefined) {
    const elapsed = Date.parse(ctx.author.authored_at) - Date.parse(entry.posted_at);
    if (!(elapsed >= 0 && elapsed < editWindowMs)) throw new Error("Messages can be edited for 48 hours.");
  }
  if (entry.deleted || (entry.text === text && !requote)) {
    await publish(ctx, item, revision, thread);
    return;
  }
  let quote;
  if (requote) {
    if (entry.replyIn) throw new Error("Choose a new checked source to change a quote from another thread.");
    const target = thread.messages.find(candidate => candidate.id === entry.replyTo && !candidate.deleted);
    const sourceSpan = target && spans.get(target.id);
    if (!target || sourceSpan === undefined) throw new Error("The original message is unavailable for a new quote.");
    entry.replyQuote = checkedText(target.text);
    quote = { item, revision, span: sourceSpan };
  }
  const hadFiles = entry.files?.length;
  const quotedBy = text === undefined ? thread.messages.filter(reply => !reply.replyIn && reply.replyTo === message && reply.replyQuote !== undefined) : [];
  if (text === undefined) {
    entry.text = "";
    entry.deleted = true;
    delete entry.edited;
    delete entry.replyTo;
    delete entry.replyToName;
    delete entry.replyIn;
    delete entry.replyQuote;
    delete entry.mentions;
    delete entry.files;
    for (const reply of quotedBy) delete reply.replyQuote;
  } else {
    entry.text = checkedText(text);
    entry.edited = true;
  }
  const purpose = text === undefined ? "person-delete" : "person-edit";
  const captured = ctx.messageWrite;
  const replace_span = spans.get(message);
  if (quotedBy.every(reply => quotes.has(reply.id)) && replace_span !== undefined &&
      (text === undefined || (captured?.item === item && captured.text === text))) {
    const record = messageRecord(entry, thread.participants);
    // Omit the quote when replacing a checked reply so the host keeps its exact existing block.
    const framing = !requote && quotes.has(message) ? { ...record, quote: undefined } : record;
    const attested = attest(framing, { kind: text === undefined ? "empty" : "input" }, quote);
    if (attested) {
      const result = await ctx.call("item_save", { request_id: step(ctx.author.request_id, purpose),
        item, expected: revision, replace_span, attested });
      if ("code" in result) throw new Error(result.code);
      await publish(ctx, item, result.revision, thread);
      return;
    }
  }
  if (hadFiles) throw new Error("Use a checked write to change a message with attachments.");
  if (text !== undefined && entry.replyTo !== undefined) throw new Error("This reply cannot be edited without a checked write.");
  await save(ctx, item, revision, thread, purpose);
}

function stateOf(body: string | undefined): State {
  if (body === undefined) return { draft: "", muted: false };
  const fields = /^---\nlast-read-message: (.*)\ndraft: (.*)\nmuted: (true|false)\n---\n$/u.exec(body);
  if (!fields) throw new Error("Unreadable private thread state.");
  const readThrough = JSON.parse(fields[1]);
  return { ...(readThrough && { readThrough }), draft: JSON.parse(fields[2]), muted: fields[3] === "true" };
}

async function readState(item: ItemRef, ctx: PeopleContext) {
  if (!ctx.privateState) throw new Error("Private state unavailable.");
  return stateOf(await ctx.privateState.read(item));
}

async function getState({ item }: Target, ctx: PeopleContext) {
  await member(ctx, item);
  return readState(item, ctx);
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
  await ctx.privateState.write(item, `---\nlast-read-message: ${anchor}\ndraft: ${JSON.stringify(draft)}\nmuted: ${muted}\n---\n`, body);
}

async function getPreferences(ctx: PeopleContext): Promise<Preferences> {
  if (!ctx.privatePreferences) throw new Error("Private preferences unavailable.");
  const body = await ctx.privatePreferences.read();
  if (body === undefined) return { receiptsAndTyping: true, push: { enabled: false, iosLocked: true, iosUnlocked: true } };
  const match = /^---\nread-receipts-and-typing: (true|false)\npush-notifications: (true|false)\npush-when-ios-locked: (true|false)\npush-when-ios-unlocked: (true|false)\n---\n$/u.exec(body);
  if (!match) throw new Error("Unreadable preferences.");
  return {
    receiptsAndTyping: match[1] === "true",
    push: { enabled: match[2] === "true", iosLocked: match[3] === "true", iosUnlocked: match[4] === "true" },
  };
}

async function setPreferences(preferences: Preferences, ctx: PeopleContext) {
  if (!ctx.privatePreferences) throw new Error("Private preferences unavailable.");
  const expected = await ctx.privatePreferences.read();
  const { receiptsAndTyping, push: { enabled, iosLocked, iosUnlocked } } = preferences;
  await ctx.privatePreferences.write(
    `---\nread-receipts-and-typing: ${receiptsAndTyping}\npush-notifications: ${enabled}\npush-when-ios-locked: ${iosLocked}\npush-when-ios-unlocked: ${iosUnlocked}\n---\n`,
    expected,
  );
}

async function row({ item }: Target, ctx: PeopleContext) {
  const { thread } = await member(ctx, item);
  const state = await readState(item, ctx);
  const actor = ctx.author.actor;
  const after = thread.messages.findIndex(entry => entry.id === state.readThrough);
  const own = new Set(thread.messages.filter(entry => !entry.deleted && entry.author.id === actor).map(entry => entry.id));
  const counts = { unread: 0, mentions: 0, replies: 0, muted: state.muted, attention: 0 };
  for (const message of thread.messages.slice(after + 1)) {
    if (message.deleted || message.author.id === actor) continue;
    const mentioned = message.mentions?.includes(actor) ?? false;
    const replied = !message.replyIn && message.replyTo !== undefined && own.has(message.replyTo);
    counts.unread++;
    counts.mentions += Number(mentioned);
    counts.replies += Number(replied);
    counts.attention += Number(mentioned || replied);
  }
  return counts;
}

async function delivery(input: MessageTarget & ({ device: "ios"; locked: boolean } | { device: "other" }), ctx: PeopleContext) {
  const { thread } = await member(ctx, input.item);
  const message = find(thread, input.message);
  if (message.deleted || message.author.id === ctx.author.actor || (await readState(input.item, ctx)).muted) {
    return { ring: false, push: false };
  }
  const { push } = await getPreferences(ctx);
  const deviceAllowed = input.device !== "ios" || (input.locked ? push.iosLocked : push.iosUnlocked);
  return { ring: true, push: push.enabled && deviceAllowed };
}

export function scoped(messages: Message[], scope: Scope) {
  if ("whole" in scope) return messages;
  const from = "message" in scope ? scope.message : scope.from;
  const through = "message" in scope ? scope.message : scope.through;
  const start = messages.findIndex(entry => entry.id === from);
  const end = messages.findIndex(entry => entry.id === through);
  if (start < 0 || end < start) throw new Error("Message scope unavailable.");
  return messages.slice(start, end + 1);
}

function addAgent(thread: Thread, identity: ContributionIdentity) {
  const agent = agentParticipant(thread.id, identity);
  if (!thread.participants.some(person => person.id === agent.id)) {
    if (thread.participants.length >= 64) throw new Error("Thread participant limit reached.");
    thread.participants.push(agent);
  }
  return agent;
}

function resultMessage(thread: Thread, ctx: PeopleContext, id: string, identity: ContributionIdentity, result: string) {
  const inviter = thread.participants.find(person => person.id === ctx.author.actor)!;
  return {
    id, author: addAgent(thread, identity), text: checkedText(result), posted_at: ctx.author.authored_at,
    askedBy: personParticipant(ctx.author.actor, ctx.author.actor_label ?? inviter.name),
  };
}

async function callAgent({ item, prompt }: Target & { prompt: string }, ctx: PeopleContext) {
  const grant = ctx.invitation;
  const canDisclose = ctx.canDisclose;
  if (!grant || !ctx.runInvited || !canDisclose || grant.item !== item || grant.inviter !== ctx.author.actor) {
    throw new Error("A checked agent invitation is required.");
  }
  let live = await writableMember(ctx, item);
  const id = messageId(ctx.author.request_id, `invited-${ctx.author.actor}`);
  if (live.thread.messages.some(message => message.id === id)) return { message: id };
  const original = live.revision === grant.revision ? live.thread : (await read(ctx, item, grant.revision)).thread;
  const selected = scoped(original.messages, grant.scope);
  const pending = [...new Set(selected.filter(message => message.author.kind === "person" && !message.deleted &&
    !canDisclose(grant, message.author.id)).map(message => message.author.id))];
  if (pending.length) {
    if (!ctx.waitForConsent) throw new Error("This host cannot wait for consent.");
    await ctx.waitForConsent(grant, pending, consentWaitSeconds);
    live = await writableMember(ctx, item);
    if (live.thread.messages.some(message => message.id === id)) return { message: id };
  }
  const { thread } = live;
  const current = new Map(thread.messages.map(message => [message.id, message]));
  const readable = selected
    .filter(message => {
      const live = current.get(message.id);
      return !message.deleted && message.author.kind === "person" && live !== undefined && !live.deleted
        && live.text === message.text && live.author.id === message.author.id && canDisclose(grant, message.author.id);
    })
    .map(({ id, text }) => ({ id, text }));
  const payload: InvitedInput = { prompt: checkedText(prompt), messages: readable, tools: true };
  if (Buffer.byteLength(JSON.stringify(payload)) > 16 * 1024) throw new Error("The invitation exceeds 16 KiB.");

  addAgent(thread, grant.contribution_identity);
  if (live.size + 18 * 1024 > MAX_FILE_BYTES) throw new Error("Thread is full.");
  const result = await ctx.runInvited(grant, payload);
  const latest = await writableMember(ctx, item);
  latest.thread.messages.push({
    ...resultMessage(latest.thread, ctx, id, grant.contribution_identity, result.text), log: result.log,
  });
  await save(ctx, item, latest.revision, latest.thread, "invited-reply");
  return { message: id };
}

async function consult({ item, prompt }: Target & { prompt: string }, ctx: PeopleContext) {
  const agents = ctx.consultation;
  if (!agents) throw new Error("A checked consultation is required.");
  const { thread, locator, size: threadSize } = await writableMember(ctx, item);
  const id = messageId(ctx.author.request_id, `consultation-${ctx.author.actor}`);
  const prior = thread.messages.find(message => message.id === id);
  if (prior?.consultation) return { message: id, consultation: prior.consultation };
  addAgent(thread, agents.lead);
  if (threadSize + 18 * 1024 > MAX_FILE_BYTES) throw new Error("Thread is full.");
  const question = checkedText(prompt);
  agents.signal.throwIfAborted();
  const title = "Agent consultation";
  const headings = { lead: "Lead agent", consultant: "Consulted agent", summary: "Summary" };
  const body = [
    "---", `title: ${title}`, `conversation: ${locator.space}/${locator.id}`,
    `lead-agent: ${agents.lead.package}/${agents.lead.contribution}`,
    `consulted-agent: ${agents.consultant.package}/${agents.consultant.contribution}`, "---", "",
  ].join("\n");
  const saved = await ctx.call("item_create", {
    request_id: step(ctx.author.request_id, "consultation-file"), kind: "thread-consultations",
    name: "Consultation.md", metadata: { title }, body: inlineBody(Buffer.from(body)),
  });
  let revision = saved.revision;
  let sequence = 0;
  let size = Buffer.byteLength(body);
  let summary: string | undefined;
  for await (const turn of agents.run(question)) {
    agents.signal.throwIfAborted();
    await requireConversationWrite(ctx, item);
    const text = checkedText(turn.text);
    const bytes = Buffer.from(`\n## ${headings[turn.role]}\n\n${text}\n`);
    size += bytes.length;
    if (size > MAX_FILE_BYTES) throw new Error("The consultation log has reached the file size limit.");
    const appended = await ctx.call("item_append", {
      request_id: step(ctx.author.request_id, `consultation-turn-${++sequence}`),
      item: saved.item, expected_revision: revision, bytes: bytes.toString("base64"),
    });
    revision = appended.revision;
    if (turn.role === "summary") {
      summary = text;
      break;
    }
  }
  agents.signal.throwIfAborted();
  if (summary === undefined) throw new Error("The consultation ended without a result.");
  const latest = await writableMember(ctx, item);
  agents.signal.throwIfAborted();
  const result = resultMessage(latest.thread, ctx, id, agents.lead, summary);
  const consultation: ItemLocator = saved.locator;
  latest.thread.messages.push({ ...result, consultation });
  await save(ctx, item, latest.revision, latest.thread, "consultation-result");
  return { message: id, consultation };
}

/** Fixture-only entry points for people threads pending shared custody and consent APIs. */
export const peopleHandlers = {
  create, post, message, getState, setState, getPreferences, setPreferences, row, delivery, callAgent, consult,
  edit: (input: MessageTarget & { text: string; requote?: boolean }, ctx: PeopleContext) => change(input, ctx, input.text, input.requote),
  remove: (input: MessageTarget, ctx: PeopleContext) => change(input, ctx),
};
