import { createHash } from "node:crypto";
import type { AttestedBody, ContributionIdentity, ItemSave, ItemSpansPage, ItemSnapshotView, ItemLocator, SnapshotNode, Timestamp } from "../generated/wire.js";

type Person = { kind: "person"; id: string; name: string };
type Agent = { kind: "agent"; id: string; name: string; contribution: ContributionIdentity };
export type Participant = Person | Agent;

/** Store the current audience-checked label with the message author. */
export function personParticipant(id: string, label?: string): Person {
  return { kind: "person", id, name: displayName(label, "Person") };
}

/** Keep local message authors stable when the host issues a fresh harness binding. */
export function agentParticipant(thread: string, identity: ContributionIdentity): Agent {
  const { package: pkg, contribution } = identity;
  const label = contribution.split(/[-_.]+/u).map(word => word.charAt(0).toUpperCase() + word.slice(1)).join(" ");
  return { kind: "agent", id: step(thread, JSON.stringify([pkg, contribution])),
    name: displayName(label, "Agent"), contribution: { package: pkg, contribution } };
}

function displayName(value: string | undefined, fallback: string): string {
  const name = value?.replace(/[\s\p{Cc}]+/gu, " ").trim();
  return name ? [...name].slice(0, 100).join("") : fallback;
}

export type Message = {
  id: string;
  author: Participant;
  text: string;
  posted_at: Timestamp;
  replyTo?: string;
  replyToName?: string;
  askedBy?: Person;
  mentions?: string[];
  log?: ItemLocator;
  consultation?: ItemLocator;
  edited?: true;
  deleted?: true;
};
export type Thread = {
  id: string;
  title: string;
  created: string;
  conversation?: "people";
  participants: Participant[];
  messages: Message[];
};

export const MAX_FILE_BYTES = 2 * 1024 * 1024;

// Request and step together give the same key on delivery replay.
export function step(request: string, purpose: string): string {
  return createHash("sha256").update(JSON.stringify([request, purpose])).digest("hex").slice(0, 32);
}

/** Allocate a compact, thread-scoped Markdown anchor independent of message text. */
export function messageId(request: string, purpose: string): string {
  return step(request, purpose).slice(0, 16);
}

export function checkedTitle(title: string): string {
  const trimmed = title.trim();
  if (!trimmed || /[\r\n\u0000-\u001f]/u.test(trimmed) || Buffer.byteLength(trimmed) > 120) {
    throw new Error("Use a single-line title of up to 120 UTF-8 bytes.");
  }
  return trimmed;
}

export function folder(thread: Thread): string {
  const readable = thread.title.replace(/[^\p{L}\p{N} -]/gu, "_");
  let short = "";
  for (const character of [...readable].slice(0, 40)) {
    if (Buffer.byteLength(short + character) > 109) break;
    short += character;
  }
  return `${thread.created.slice(0, 10)} ${short}`;
}

const messageBoundary = /^### [^\r\n]+ \{#m-[a-f0-9]{16}(?: [^{}\r\n]+)?\}$/u;
const messageHeading = /^### (.+) \{#m-([a-f0-9]{16}) (by=p[1-9][0-9]*(?: [a-z]+(?:-[a-z]+)*=[^ {}]+)*)\}$/u;

function escapeBodyHeadings(text: string, encode: boolean): string {
  return text.split("\n").map(line => {
    if (!encode && !line.startsWith("\\")) return line;
    if (!messageBoundary.test(line.replace(/^\\+/u, "").replace(/\r$/u, ""))) return line;
    return encode ? `\\${line}` : line.slice(1);
  }).join("\n");
}

function escapeLabel(value: string): string {
  return value.replace(/[\\|·{}]/gu, "\\$&");
}

function unescapeLabel(value: string): string {
  return value.replace(/\\([\\|·{}])/gu, "$1");
}

function utc(timestamp: Timestamp): string {
  return new Date(timestamp).toISOString().replace("T", " ").replace(/:00\.000Z$/u, "Z").replace(/\.000Z$/u, "Z").replace("Z", " UTC");
}

function timestamp(value: string): Timestamp {
  const match = /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2})(:\d{2}(?:\.\d{3})?)? UTC$/u.exec(value);
  if (!match) throw new Error("Unreadable message date.");
  const iso = `${match[1]}T${match[2]}${match[3] ?? ":00"}Z`;
  const date = new Date(iso);
  if (Number.isNaN(date.valueOf()) || utc(iso) !== value) throw new Error("Unreadable message date.");
  return date.toISOString().replace(/\.000Z$/u, "Z");
}

type FramedText = { prefix: string; text: string; suffix: string };
const maxFramingBytes = 512;

/** Select a host source only when each framing operand fits the host byte bound. */
export function attest(parts: FramedText, source: AttestedBody["source"]): AttestedBody | undefined {
  const { prefix, suffix } = parts;
  if (Buffer.byteLength(prefix) > maxFramingBytes || Buffer.byteLength(suffix) > maxFramingBytes) return;
  return { source, prefix, suffix };
}

/** Frame one message around the exact original text. Inline agent identities let replies append without rewriting frontmatter. */
export function messageRecord(message: Message, participants: Participant[], inlineAgent = false): FramedText {
  const alias = (id: string) => {
    const index = participants.findIndex(person => person.id === id);
    if (index < 0) throw new Error("Message participant unavailable.");
    return `p${index + 1}`;
  };
  const heading = [escapeLabel(message.author.name), utc(message.posted_at)];
  const attrs = [`#m-${message.id}`, `by=${alias(message.author.id)}`];
  if (message.deleted) {
    return { prefix: `### ${[...heading, "deleted"].join(" · ")} {${attrs.join(" ")}}\n\n`, text: "", suffix: "" };
  }
  if (inlineAgent && message.author.kind === "agent") {
    const { package: pkg, contribution } = message.author.contribution;
    attrs.push(`agent-plugin=${encodeURIComponent(pkg)}/${encodeURIComponent(contribution)}`);
  }
  if (message.edited) heading.push("edited");
  if (message.replyTo && message.replyToName) {
    heading.push(`reply to ${escapeLabel(message.replyToName)}`);
    attrs.push(`reply-to=m-${message.replyTo}`);
  }
  if (message.askedBy) {
    heading.push(`asked by ${escapeLabel(message.askedBy.name)}`);
    attrs.push(`asked-by=${alias(message.askedBy.id)}`);
  }
  if (message.mentions?.length) attrs.push(`mentions=${message.mentions.map(alias).join(",")}`);
  const links: string[] = [];
  for (const [name, link] of [["run-log", message.log], ["consultation", message.consultation]] as const) {
    if (link) links.push(`${name}=${encodeURIComponent(link.space)}/${encodeURIComponent(link.id)}`);
  }
  if (links.length) attrs.push("links=1");
  return { prefix: `### ${heading.join(" · ")} {${attrs.join(" ")}}\n\n`, text: message.text,
    suffix: links.length ? `\n\n<!-- ${links.join(" ")} -->\n\n` : "\n\n" };
}

/** Render the opaque Markdown for a record that cannot use host composition. */
export function encodeRecord({ prefix, text, suffix }: FramedText): string {
  return prefix + escapeBodyHeadings(text, true) + suffix;
}

/** Identify a previously stored middle that needs escaping if its span index is discarded. */
export function needsOpaqueEscape(text: string): boolean {
  return escapeBodyHeadings(text, true) !== text;
}

/** Append a tagged heading and one plain body span. */
export function encodeMessage(message: Message, participants: Participant[]): string {
  return encodeRecord(messageRecord(message, participants));
}

/** Frame an empty conversation around its checked title. */
export function threadHeader(thread: Thread): FramedText {
  const { title, created, id, participants, conversation } = thread;
  const header = [`created: ${utc(created)}`];
  if (conversation) header.push("conversation: people");
  header.push("participants:");
  participants.forEach((person, index) => header.push(`  - p${index + 1} | ${escapeLabel(person.name)} | ${person.kind} | ${person.id}`));
  participants.forEach((person, index) => {
    if (person.kind === "agent") {
      header.push(`agent-p${index + 1}-plugin: ${person.contribution.package}`,
        `agent-p${index + 1}-harness: ${person.contribution.contribution}`);
    }
  });
  return { prefix: `---\nformat: calternal-thread/v2\nid: ${id}\ntitle: `, text: title,
    suffix: `\n${header.join("\n")}\n---\n\n` };
}

/** Serialize the transcript in the same format as the append stream. */
export function encode(thread: Thread): string {
  const { prefix, text, suffix } = threadHeader(thread);
  return prefix + text + suffix + thread.messages.map(message => encodeMessage(message, thread.participants)).join("");
}

type SpanRef = NonNullable<ItemSave["replace_span"]>;
type IndexedSpan = ItemSpansPage["spans"][number];
type MessageRecord = { heading: string; body: string[]; indexed?: { span: IndexedSpan; text: string } };

function decodeMessage({ heading: line, body, indexed }: MessageRecord, people: Map<string, Participant>, threadId: string): Message {
  const match = messageHeading.exec(line);
  if (!match) throw new Error("Unreadable message heading.");
  const [, label, id, raw] = match;
  const [authorName, date, ...markers] = label.split(" · ");
  const attrs = new Map<string, string>();
  for (const field of raw.split(" ")) {
    const [key, value] = field.split("=", 2);
    if (!["by", "reply-to", "asked-by", "mentions", "links", "agent-plugin"].includes(key) || !value || attrs.has(key)) {
      throw new Error("Could not read the details after the message heading.");
    }
    attrs.set(key, value);
  }
  const inlineAgent = attrs.get("agent-plugin");
  if (inlineAgent) {
    const [pkg, contribution, ...extra] = inlineAgent.split("/");
    if (!pkg || !contribution || extra.length) throw new Error("Could not read which plugin runs this agent.");
    const agent = agentParticipant(threadId, { package: decodeURIComponent(pkg), contribution: decodeURIComponent(contribution) });
    const alias = attrs.get("by")!;
    const previous = people.get(alias);
    if (previous && previous.id !== agent.id) throw new Error("Conflicting agent participant.");
    people.set(alias, agent);
  }
  const participant = (alias: string | undefined) => {
    const person = alias && people.get(alias);
    if (!person) throw new Error("Message participant unavailable.");
    return person;
  };
  const author = { ...participant(attrs.get("by")), name: unescapeLabel(authorName) };
  const attribution = indexed?.span.attribution;
  if (attribution?.kind === "person" && (author.kind !== "person" || author.id !== attribution.actor) ||
      attribution?.kind === "attempt" && author.kind !== "agent") throw new Error("The saved author does not match the message heading.");
  const posted_at = timestamp(date);
  const start = body[0] === "" ? 1 : 0;
  let end = start < body.length && body.at(-1) === "" ? body.length - 1 : body.length;
  if (markers.includes("deleted")) {
    if (body.some(line => line !== "") || attrs.size !== 1 || markers.length !== 1 ||
        (indexed && (indexed.text !== "" || attribution?.kind !== "empty"))) throw new Error("Could not read the deleted message marker.");
    return { id, author, posted_at, text: "", deleted: true };
  }
  const refs = new Map<string, string>();
  if (attrs.has("links")) {
    const trailer = /^<!-- ((?:run-log|consultation)=[^ <>]+(?: (?:run-log|consultation)=[^ <>]+)?) -->$/u.exec(body[end - 1]);
    if (attrs.get("links") !== "1" || !trailer || body[end - 2] !== "") throw new Error("Could not read the links after the message.");
    for (const ref of trailer[1].split(" ")) {
      const [key, value] = ref.split("=", 2);
      if (refs.has(key)) throw new Error("Duplicate message reference.");
      refs.set(key, value);
    }
    end -= 2;
  }
  const framingBody = body.slice(start, end).join("\n");
  if (indexed && (framingBody !== "" || attribution?.kind === "empty")) throw new Error("Could not read where the message text starts and ends.");
  const message: Message = { id, author, posted_at, text: indexed?.text ?? escapeBodyHeadings(framingBody, false) };
  const reply = attrs.get("reply-to");
  const replyLabel = markers.find(marker => marker.startsWith("reply to "));
  if (reply || replyLabel) {
    if (!reply || !/^m-[a-f0-9]{16}$/u.test(reply) || !replyLabel) throw new Error("Unreadable reply reference.");
    message.replyTo = reply.slice(2);
    message.replyToName = unescapeLabel(replyLabel.slice("reply to ".length));
  }
  const asked = attrs.get("asked-by");
  const askLabel = markers.find(marker => marker.startsWith("asked by "));
  if (asked || askLabel) {
    const asker = participant(asked);
    if (asker.kind !== "person" || !askLabel) throw new Error("Could not read who asked the agent.");
    message.askedBy = { ...asker, name: unescapeLabel(askLabel.slice("asked by ".length)) };
  }
  const mentions = attrs.get("mentions");
  if (mentions) message.mentions = mentions.split(",").map(alias => participant(alias).id);
  for (const [attribute, field] of [["run-log", "log"], ["consultation", "consultation"]] as const) {
    const value = refs.get(attribute);
    if (!value) continue;
    const [space, id, ...rest] = value.split("/");
    if (!space || !id || rest.length) throw new Error("Could not read the saved link.");
    message[field] = { space: decodeURIComponent(space), id: decodeURIComponent(id) };
  }
  if (markers.includes("edited")) message.edited = true;
  return message;
}

function decodeRecords(frontmatter: string[], records: Iterable<MessageRecord>) {
  const fields = new Map<string, string>();
  const listed = new Map<string, { name: string; kind: string; id: string }>();
  for (const line of frontmatter) {
    if (line === "participants:") continue;
    const item = /^  - (p[1-9][0-9]*) \| (.+) \| (person|agent) \| (\S+)$/u.exec(line);
    if (item) {
      if (listed.has(item[1])) throw new Error("A participant label is listed twice.");
      listed.set(item[1], { name: unescapeLabel(item[2]), kind: item[3], id: item[4] });
      continue;
    }
    const field = /^([a-z0-9-]+): (.+)$/u.exec(line);
    if (!field || fields.has(field[1])) throw new Error("Could not read the conversation details at the top of the file.");
    fields.set(field[1], field[2]);
  }
  const id = fields.get("id");
  const title = fields.get("title");
  const created = fields.get("created");
  if (fields.get("format") !== "calternal-thread/v2" || !id || !/^[a-f0-9]{32}$/u.test(id) || !title || !created) {
    throw new Error("Could not read the conversation details at the top of the file.");
  }
  const people = new Map<string, Participant>();
  for (const [alias, entry] of listed) {
    if (entry.kind === "person") people.set(alias, { ...entry, kind: "person" });
    else {
      const pkg = fields.get(`agent-${alias}-plugin`);
      const contribution = fields.get(`agent-${alias}-harness`);
      if (!pkg || !contribution) throw new Error("Could not read which plugin runs this agent.");
      people.set(alias, { ...entry, kind: "agent", contribution: { package: pkg, contribution } });
    }
  }
  const spans = new Map<string, SpanRef>();
  const anchors = new Set<string>();
  const messages: Message[] = [];
  for (const record of records) {
    const message = decodeMessage(record, people, id);
    if (anchors.has(message.id)) throw new Error("Two messages use the same ID.");
    anchors.add(message.id);
    if (record.indexed) spans.set(message.id, record.indexed.span.span);
    messages.push(message);
  }
  const thread: Thread = { id, title, created: timestamp(created), participants: [...people.values()],
    ...(fields.get("conversation") === "people" && { conversation: "people" }), messages };
  return { thread, spans };
}

function documentLines(document: string): string[] {
  const lines = document.split("\n");
  if (document.endsWith("\n")) lines.pop();
  return lines;
}

function* opaqueRecords(lines: string[]): Iterable<MessageRecord> {
  let heading: string | undefined;
  let content: string[] = [];
  for (const line of lines) {
    if (messageBoundary.test(line)) {
      if (heading !== undefined) yield { heading, body: content };
      heading = line;
      content = [];
    } else if (heading !== undefined) content.push(line);
    else if (line !== "") throw new Error("Expected a message heading.");
  }
  if (heading !== undefined) yield { heading, body: content };
}

/** Recover an imported or hand-edited file using its tagged headings. */
export function decode(body: string): Thread {
  const document = body.startsWith("---\r\n") ? body.replaceAll("\r\n", "\n") : body;
  const lines = documentLines(document);
  const close = lines.indexOf("---", 1);
  if (lines[0] !== "---" || close < 0) throw new Error("Unreadable thread document.");
  return decodeRecords(lines.slice(1, close), opaqueRecords(lines.slice(close + 1))).thread;
}

/** Parse message boundaries at host-pinned UTF-8 byte ranges and retain their writer refs. */
export function decodeIndexed(bytes: Buffer, index: readonly IndexedSpan[], provenance: ItemSpansPage["body_provenance"]) {
  const utf8 = new TextDecoder("utf-8", { fatal: true });
  const close = bytes.indexOf("\n---\n", 3);
  if (bytes.subarray(0, 4).toString() !== "---\n" || close < 4) throw new Error("Unreadable thread document.");
  const headerEnd = close + 5;
  function* records(): Iterable<MessageRecord> {
    let cursor = headerEnd;
    const refs = new Set<SpanRef>();
    for (const [position, entry] of index.entries()) {
      const { offset, length, span, middle_offset, middle_length } = entry;
      const end = offset + length;
      const middleEnd = middle_offset + middle_length;
      if (![offset, length, middle_offset, middle_length].every(Number.isSafeInteger) || length <= 0 || end > bytes.length ||
          middle_offset < offset || middle_length < 0 || middleEnd > end || refs.has(span)) {
        throw new Error("The saved message ranges are invalid.");
      }
      refs.add(span);
      // An attested create records the frontmatter as the first span.
      if (position === 0 && offset === 0 && (end === headerEnd || (end === headerEnd + 1 && bytes[headerEnd] === 10))) {
        cursor = end;
        continue;
      }
      if (offset < cursor) throw new Error("The saved message ranges overlap.");
      if (offset > cursor) {
        if (provenance === "attested") throw new Error("Some saved message ranges are missing.");
        yield* opaqueRecords(documentLines(utf8.decode(bytes.subarray(cursor, offset))));
      }
      const prefix = utf8.decode(bytes.subarray(offset, middle_offset));
      const suffix = utf8.decode(bytes.subarray(middleEnd, end));
      if (!prefix.endsWith("\n\n")) throw new Error("Could not read where the message text starts and ends.");
      const [heading, ...body] = documentLines(prefix + suffix);
      yield { heading, body, indexed: { span: entry, text: utf8.decode(bytes.subarray(middle_offset, middleEnd)) } };
      cursor = end;
    }
    if (cursor < bytes.length) {
      if (provenance === "attested") throw new Error("Some saved message ranges are missing.");
      yield* opaqueRecords(documentLines(utf8.decode(bytes.subarray(cursor))));
    }
  }
  return decodeRecords(documentLines(utf8.decode(bytes.subarray(4, close))), records());
}

/** Resolve a visible message and keep reply context only while its target is visible. */
export function messageView(thread: Thread, id: string): Omit<Message, "deleted"> {
  const message = thread.messages.find(entry => entry.id === id);
  if (!message || message.deleted) throw new Error("Message unavailable.");
  const { replyTo, replyToName, deleted, ...fields } = message;
  const target = replyTo && thread.messages.find(entry => entry.id === replyTo && !entry.deleted);
  return { ...fields, ...(target && { replyTo, replyToName }) };
}

export function page(thread: Thread, older?: string, anchor?: string): { view: ItemSnapshotView; older?: string; anchor?: string } {
  if (anchor !== undefined && older !== undefined) throw new Error("Open either a linked message or an older page at a time.");
  const visible = thread.messages.filter(message => !message.deleted);
  const index = anchor === undefined ? -1 : visible.findIndex(message => message.id === anchor);
  if (anchor !== undefined && index === -1) throw new Error("Message unavailable.");
  const end = anchor !== undefined ? index + 1 : older === undefined ? visible.length : Number(older);
  if (!Number.isSafeInteger(end) || end < 0 || end > visible.length) {
    throw new Error("Invalid thread page.");
  }
  let start = end;
  let children: SnapshotNode[] = [];
  while (start > 0 && end - start < 10) {
    const message = visible[start - 1];
    const nodes: SnapshotNode[] = [
      { type: "snapshot-heading", anchor: message.id, title: (message.author.kind === "person" ? "Person" : "Agent") + (message.edited ? " (edited)" : "") },
    ];
    const text = [...message.text];
    for (let offset = 0; offset < text.length; offset += 2000) {
      nodes.push({ type: "snapshot-text", text: text.slice(offset, offset + 2000).join("") });
    }
    const next = [...nodes, ...children];
    // Reserve the envelope and cursor overhead within the 64 KiB bound.
    if (next.length > 24 || Buffer.byteLength(JSON.stringify(next)) > 65000) break;
    children = next;
    start--;
  }
  const [first, ...rest] = children;
  return {
    view: { content: first ? { type: "stack", children: [first, ...rest] } : { type: "snapshot-text", text: "" } },
    ...(start ? { older: String(start) } : {}),
    ...(anchor !== undefined && { anchor }),
  };
}
