import { createHash } from "node:crypto";
import type { ContributionIdentity, ItemSnapshotView, ItemLocator, SnapshotNode } from "../generated/wire.js";

export type Participant = { kind: "person"; id: string } | { kind: "agent"; id: string; contribution: ContributionIdentity };

/** Keep local message authors stable when the host issues a fresh harness binding. */
export function agentParticipant(thread: string, identity: ContributionIdentity): Extract<Participant, { kind: "agent" }> {
  const { package: pkg, contribution } = identity;
  return { kind: "agent", id: step(thread, JSON.stringify([pkg, contribution])), contribution: { package: pkg, contribution } };
}
export type Message = { id: string; author: Participant; text: string; log?: ItemLocator; edited?: true; deleted?: true };
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

export function encodeMessage(message: Message): string {
  const { id, author, text, log, edited, deleted } = message;
  const frame = JSON.stringify({ id, author, log, edited, deleted, length: text.length }).replaceAll(">", "\\u003e");
  return `\n<a id="message-${id}"></a>\n## ${author.kind === "person" ? "Person" : "Agent"}\n<!-- ${frame} -->\n\n${deleted ? "[Message deleted]" : text}\n`;
}

export function encode(thread: Thread): string {
  const { title, created, id, participants, messages, conversation } = thread;
  return `---\nformat: calternal-thread-1\nid: ${id}\ntitle: ${JSON.stringify(title)}\ncreated: ${created}\nparticipants: ${JSON.stringify(participants)}\n${conversation ? "conversation: people\n" : ""}---\n`
    + messages.map(encodeMessage).join("");
}

export function decode(body: string): Thread {
  const header = /^---\nformat: calternal-thread-1\nid: ([a-f0-9]{32})\ntitle: (.+)\ncreated: (.+)\nparticipants: (.+)\n(conversation: people\n)?---\n/u.exec(body);
  if (!header) throw new Error("Unreadable thread document.");
  const messages: Message[] = [];
  const frame = /\n<a id="message-([a-f0-9]{32})"><\/a>\n## (?:Person|Agent)\n<!-- (.+?) -->\n\n/uy;
  let offset = header[0].length;
  while (offset < body.length) {
    frame.lastIndex = offset;
    const record = frame.exec(body);
    if (!record) throw new Error("Unreadable message record.");
    const { id, author, log, edited, deleted, length } = JSON.parse(record[2]);
    const tombstone = "[Message deleted]";
    const end = frame.lastIndex + (deleted === true ? tombstone.length : length);
    if (id !== record[1] || !Number.isSafeInteger(length) || body[end] !== "\n"
      || (deleted === true ? length !== 0 || body.slice(frame.lastIndex, end) !== tombstone : length < 1)) {
      throw new Error("Unreadable message record.");
    }
    messages.push({ id, author, ...(log && { log }), ...(edited === true && { edited }), ...(deleted === true && { deleted }),
      text: deleted === true ? "" : body.slice(frame.lastIndex, end) });
    offset = end + 1;
  }
  return { id: header[1], title: JSON.parse(header[2]), created: header[3], participants: JSON.parse(header[4]), ...(header[5] && { conversation: "people" }), messages };
}

/** Return a tombstone at the original anchor without carrying the deleted text. */
export function messageView({ text, ...message }: Message): Omit<Message, "text"> & { text?: string } {
  return message.deleted ? message : { ...message, text };
}

export function page(thread: Thread, older?: string, anchor?: string): { view: ItemSnapshotView; older?: string; anchor?: string } {
  if (anchor !== undefined && older !== undefined) throw new Error("An anchored page cannot use a cursor.");
  const index = anchor === undefined ? -1 : thread.messages.findIndex(message => message.id === anchor);
  if (anchor !== undefined && index === -1) throw new Error("Message unavailable.");
  const end = anchor !== undefined ? index + 1 : older === undefined ? thread.messages.length : Number(older);
  if (!Number.isSafeInteger(end) || end < 0 || end > thread.messages.length) {
    throw new Error("Invalid thread page.");
  }
  let start = end;
  let children: SnapshotNode[] = [];
  while (start > 0 && end - start < 10) {
    const message = thread.messages[start - 1];
    const nodes: SnapshotNode[] = [
      { type: "snapshot-heading", anchor: message.id, title: (message.author.kind === "person" ? "Person" : "Agent") + (message.edited && !message.deleted ? " (edited)" : "") },
    ];
    const text = [...(message.deleted ? "[Message deleted]" : message.text)];
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
