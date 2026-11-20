import type { ItemViewPage, SnapshotNode, Timestamp } from "../generated/wire.js";

const uuid = "[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}";
const marker = new RegExp(`^<!-- id:(${uuid}) role:(person|agent|automation) bytes:([0-9]+)(?: turn:(${uuid}) meta:(.+))? -->$`, "u");
const v1 = "format: calternal-thread/v1";
const noticePrefix = "> Sign-in was ";
const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const invalid = () => new Error("Could not read the saved version 1 thread.");

type LegacyRole = "person" | "agent" | "automation";
type LegacyMessage = {
  id: string;
  role: LegacyRole;
  heading: string;
  posted_at: Timestamp;
  text: string;
  turn?: string;
};
type LegacyThread = { messages: LegacyMessage[] };

function frontmatterEnd(bytes: Buffer) {
  return bytes.subarray(0, 4).equals(Buffer.from("---\n")) ? bytes.indexOf("\n---\n", 4) : -1;
}

/** Identify the old writer's format using the envelope only. */
export function isLegacy(bytes: Buffer) {
  const close = frontmatterEnd(bytes);
  return close !== -1 && utf8.decode(bytes.subarray(4, close)).split("\n").includes(v1);
}

function readLine(bytes: Buffer, start: number) {
  const end = bytes.indexOf(10, start);
  if (end < 0) throw invalid();
  return { line: utf8.decode(bytes.subarray(start, end)), next: end + 1 };
}

function after(bytes: Buffer, start: number, separator: string) {
  if (!bytes.subarray(start, start + separator.length).equals(Buffer.from(separator))) throw invalid();
  return start + separator.length;
}

function headingTime(heading: string, role: LegacyRole): Timestamp {
  const at = heading.split(" · ")[role === "agent" ? 3 : 1];
  const stamp = /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}:\d{2}) ([+-]\d{2}:\d{2})$/u.exec(at ?? "");
  if (!stamp) throw invalid();
  const wall = `${stamp[1]}T${stamp[2]}`;
  const instant = new Date(wall + stamp[3]);
  if (!Number.isFinite(+instant) || new Date(wall + "Z").toISOString().slice(0, 19) !== wall) throw invalid();
  return instant.toISOString().replace(".000Z", "Z");
}

/** Read the server's framed records. Names, roles and attempt refs are historical display evidence only. */
export function decodeLegacy(bytes: Buffer): LegacyThread {
  const close = frontmatterEnd(bytes);
  if (close < 0) throw invalid();
  const format = utf8.decode(bytes.subarray(4, close)).split("\n").filter(line => line.startsWith("format: "));
  if (format.length !== 1 || format[0] !== v1) throw invalid();

  const title = readLine(bytes, after(bytes, close + 5, "\n# "));
  let offset = after(bytes, title.next, "\n");
  const end = bytes.length - 1;
  if (bytes[end] !== 10) throw invalid();
  const sessionPrefix = "<!-- provider-session:";
  if (bytes.subarray(offset, offset + sessionPrefix.length).toString("ascii") === sessionPrefix) {
    // The opaque private binding can be on disk. Keep it out of every returned view.
    const stop = bytes.indexOf(" -->\n\n", offset + sessionPrefix.length);
    if (stop < 0) throw invalid();
    offset = stop + 6;
  }
  let notices = 0;
  while (bytes.subarray(offset, offset + noticePrefix.length).toString("ascii") === noticePrefix) {
    if (++notices > 16) throw invalid();
    offset = after(bytes, readLine(bytes, offset).next, "\n");
  }
  if (offset > end) throw invalid();
  const messages: LegacyMessage[] = [];
  const ids = new Set<string>();
  if (offset + 2 <= end && bytes[offset] === 10 && bytes[offset + 1] === 10) offset += 2;
  while (offset < end) {
    const { line: heading, next } = readLine(bytes, after(bytes, offset, "## "));
    const record = readLine(bytes, next);
    const [, id, role, length, turn, meta] = marker.exec(record.line) ?? [];
    if ((role !== "person" && role !== "agent" && role !== "automation") ||
        ids.has(id) || (role === "agent") !== (turn !== undefined)) throw invalid();
    if (meta !== undefined) {
      const details: unknown = JSON.parse(meta);
      if (!details || typeof details !== "object" || !("provider_id" in details) || typeof details.provider_id !== "string") throw invalid();
    }
    const start = after(bytes, record.next, "\n");
    const stop = start + Number(length);
    if (!Number.isSafeInteger(stop) || stop > end) throw invalid();
    const text = utf8.decode(bytes.subarray(start, stop));
    const posted_at = headingTime(heading, role);
    messages.push({ id, role, heading, posted_at, text, ...(turn && { turn }) });
    ids.add(id);
    if (stop === end) break;
    offset = after(bytes, stop, "\n\n");
    if (offset >= end) throw invalid();
  }
  return { messages };
}

const pageChars = 8000;
const headingChars = 2000;

function splitPoint(text: string, index: number) {
  const unit = text.charCodeAt(index);
  return unit < 0xdc00 || unit > 0xdfff || Number.isNaN(unit);
}

function displayLabel(heading: string) {
  return [...heading.split(" · ")[0]
    .replace(/^(.+?) via .+$/u, "$1")
    .replace(/&#183;/gu, "·").replace(/&#118;ia/gu, "via")
    .replace(/\\([\\#\[\]*_~<>&!()`])/gu, "$1")].slice(0, 120).join("");
}

function displayHeading(heading: string) {
  const limit = splitPoint(heading, headingChars) ? headingChars : headingChars - 1;
  return heading.slice(0, limit);
}

/** Read one linked legacy message in bounded text chunks using UTF-16 offsets. */
export function legacyMessage(thread: LegacyThread, id: string, offset = 0) {
  const message = thread.messages.find(entry => entry.id === id);
  if (!message) throw new Error("Message unavailable.");
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > message.text.length || !splitPoint(message.text, offset)) throw invalid();
  let end = Math.min(offset + pageChars, message.text.length);
  if (!splitPoint(message.text, end)) end--;
  return { ...message, heading: displayHeading(message.heading), text: message.text.slice(offset, end),
    ...(end < message.text.length && { next_offset: end }) };
}

/** Open the selected original UUID or the newest legacy page. Large messages page backward without losing bytes. */
export function legacyPage(thread: LegacyThread, older?: string, anchor?: string): ItemViewPage {
  if (anchor !== undefined && older !== undefined) throw invalid();
  const { messages } = thread;
  const cursor = older === undefined ? undefined : /^(\d+):(\d+)$/u.exec(older);
  if (older !== undefined && !cursor) throw invalid();
  const index = anchor !== undefined ? messages.findIndex(message => message.id === anchor)
    : cursor ? Number(cursor[1]) : messages.length - 1;
  const message = messages[index];
  if (!message) {
    if (anchor !== undefined) return { anchor_unavailable: true };
    if (cursor) throw invalid();
    return { view: { content: { type: "snapshot-text", text: "" } } };
  }
  const end = cursor ? Number(cursor[2]) : message.text.length;
  if (!Number.isSafeInteger(end) || end < 0 || end > message.text.length || !splitPoint(message.text, end)) throw invalid();
  let start = Math.max(0, end - pageChars);
  if (!splitPoint(message.text, start)) start++;
  const title = { person: "Person", agent: "Agent", automation: "Automation" }[message.role];
  const children: [SnapshotNode, ...SnapshotNode[]] = [
    { type: "snapshot-heading", ...(end === message.text.length && { anchor: message.id }), title,
      author: displayLabel(message.heading), timestamp: message.posted_at },
  ];
  const text = message.text.slice(start, end);
  const content = message.role === "agent" && end === message.text.length ? displayHeading(message.heading) + "\n\n" + text : text;
  const scalars = [...content];
  for (let from = 0; from < scalars.length; from += 2000) {
    children.push({ type: "snapshot-text", text: scalars.slice(from, from + 2000).join("") });
  }
  return {
    view: { content: { type: "stack", children } },
    ...(start > 0 ? { older: `${index}:${start}` } : index > 0 ? { older: `${index - 1}:${messages[index - 1].text.length}` } : {}),
    ...(anchor !== undefined && { anchor }),
  };
}
