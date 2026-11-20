import { createHash } from "node:crypto";
import type { ItemViewPage, SnapshotNode } from "../generated/wire.js";

type Participant = { kind: "person" | "agent"; id: string };
type Message = { id: string; author: Participant; text: string; log?: string };
export type Thread = {
  id: string;
  title: string;
  created: string;
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
  const short = [...readable].slice(0, 40).join("");
  return `${thread.created.slice(0, 10)} ${short}`;
}

export function encodeMessage(message: Message): string {
  const { id, author, text, log } = message;
  const frame = JSON.stringify({ id, author, log, length: text.length }).replaceAll(">", "\\u003e");
  return `\n<a id="message-${id}"></a>\n## ${author.kind === "person" ? "Person" : "Agent"}\n<!-- ${frame} -->\n\n${text}\n`;
}

export function encode(thread: Thread): string {
  const { title, created, id, participants, messages } = thread;
  return `---\nformat: calternal-thread-1\nid: ${id}\ntitle: ${JSON.stringify(title)}\ncreated: ${created}\nparticipants: ${JSON.stringify(participants)}\n---\n`
    + messages.map(encodeMessage).join("");
}

export function decode(body: string): Thread {
  const header = /^---\nformat: calternal-thread-1\nid: ([a-f0-9]{32})\ntitle: (.+)\ncreated: (.+)\nparticipants: (.+)\n---\n/u.exec(body);
  if (!header) throw new Error("Unreadable thread document.");
  const messages: Message[] = [];
  const frame = /\n<a id="message-([a-f0-9]{32})"><\/a>\n## (?:Person|Agent)\n<!-- (.+?) -->\n\n/uy;
  let offset = header[0].length;
  while (offset < body.length) {
    frame.lastIndex = offset;
    const record = frame.exec(body);
    if (!record) throw new Error("Unreadable message record.");
    const { id, author, log, length } = JSON.parse(record[2]);
    const end = frame.lastIndex + length;
    if (id !== record[1] || !Number.isSafeInteger(length) || length < 1 || body[end] !== "\n") {
      throw new Error("Unreadable message record.");
    }
    messages.push({ id, author, ...(log ? { log } : {}), text: body.slice(frame.lastIndex, end) });
    offset = end + 1;
  }
  return { id: header[1], title: JSON.parse(header[2]), created: header[3], participants: JSON.parse(header[4]), messages };
}

export function page(thread: Thread, older?: string): ItemViewPage {
  const end = older === undefined ? thread.messages.length : Number(older);
  if (!Number.isSafeInteger(end) || end < 0 || end > thread.messages.length) {
    throw new Error("Invalid thread page.");
  }
  let start = end;
  let children: SnapshotNode[] = [];
  while (start > 0 && end - start < 10) {
    const message = thread.messages[start - 1];
    const nodes: SnapshotNode[] = [
      { type: "snapshot-heading", title: message.author.kind === "person" ? "Person" : "Agent" },
    ];
    let text = "";
    let length = 0;
    for (const character of message.text) {
      const width = Buffer.byteLength(character);
      if (length + width > 2000) {
        nodes.push({ type: "snapshot-text", text });
        text = "";
        length = 0;
      }
      text += character;
      length += width;
    }
    if (text) nodes.push({ type: "snapshot-text", text });
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
  };
}
