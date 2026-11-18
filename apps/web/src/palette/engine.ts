/**
 * The palette's engine: one line in, rows out. It parses the line with the ported recogniser (no model, no network), searches every space, matches the
 * commands plugins gave as data, and says in one line what Enter will do. The thread composer will call the same `plan`. Nothing here touches the DOM.
 */
import { allCommands, type CommandDef } from "./manifest";
import { buildIndex, search, type Index, type Item } from "./search";
import { recognise } from "./phrases";
import type { Parsed } from "./recognise";
import { MENTION_RE, SKILL_RE } from "./mentions";

/** What running a row does. The host (Palette.svelte) turns it into navigation or an operation. */
export interface Action { go?: string; op?: string; ask?: string; text?: string }
/** One row of the list. */
export interface Row {
  /** Stable id of the thing or command the row stands for. */
  id: string;
  /** What the row does: open something, run a command, ask the agent, make a task, or make something new. */
  kind: "open" | "command" | "ask" | "task" | "new";
  /** The row's text (the title of what it opens). */
  label: string;
  /** A second line saying what kind of thing it is. */
  sub?: string;
  /** The space, when it is not the current one. */
  space?: string;
  /** What Enter does for this row. */
  action: Action;
}
/** The whole answer for one line. */
export interface Plan { parsed: Parsed; intent: string; rows: Row[] }

let index: Index = buildIndex([]);
let current = "";
/** Loads the searchable items (threads and notes of every space the person can reach) plus the commands. Call when data changes, not per key. */
export function setItems(items: Item[], currentSpace: string): void {
  const cmds: Item[] = allCommands().map((c: CommandDef) => ({ id: c.id, kind: "command", title: c.label, space: currentSpace, go: c.go, op: c.op, group: c.group, words: c.words }));
  index = buildIndex([...items, ...cmds]);
  current = currentSpace;
}

/** Plans one line. `provider` names the agent an "Ask" row would send to. */
export function plan(text: string, provider = "the agent"): Plan {
  const parsed = recognise(text);
  const q = text.trim();
  if (!q) return { parsed, intent: "", rows: [] };
  const found = search(index, q, 12);
  const rows: Row[] = found.map((i) => ({
    id: i.id,
    kind: i.kind === "command" ? "command" : "open",
    label: i.title,
    sub: i.group ?? (i.kind === "thread" ? "Thread" : "Note"),
    space: i.space !== current ? i.space : undefined,
    action: { go: i.go, op: i.op },
  }));
  // What the line says comes first: a task phrase is a task; otherwise the best match is opened; otherwise it is a question for the agent.
  const when = parsed.when ? ` · ${parsed.when}${parsed.time ? ` ${parsed.time}` : ""}` : "";
  let intent: string;
  if (parsed.guess === "task") {
    intent = `New task: ${parsed.title}${when}`;
    rows.unshift({ id: "task", kind: "task", label: intent, sub: "Tasks", action: { op: "task.new", text: q } });
  } else if (rows[0]) intent = rows[0].kind === "command" ? `Go to ${rows[0].label}` : `Open: ${rows[0].label}`;
  else intent = `Ask ${provider}: ${q}`;
  // Looking for the words inside threads is the thread search (the footer's search button and Mod+K start here): its own row, after what matches by title, before the agent.
  rows.push({ id: "messages", kind: "command", label: `Search messages for \u201c${q}\u201d`, sub: "Threads", action: { op: "messages.search", text: q } });
  // Sending to an agent is outward: always its own row, last, never the default when something matches.
  rows.push({ id: "ask", kind: "ask", label: `Ask ${provider}: ${q}`, sub: "Send to the agent", action: { ask: q } });
  return { parsed, intent, rows };
}

/** A highlighted range of the line: a recognised word (date, priority, tag ...), an `@mention` or a `/skill`. */
export interface Mark { start: number; end: number; kind: string }

/**
 * The ranges to highlight in a line, for the palette and the thread composer alike: the recogniser's spans, then `@mentions` and a leading `/skill` that
 * do not overlap them. Quoted or backticked text is never read (the recogniser keeps it out; the others skip it too).
 */
export function marksOf(text: string): Mark[] {
  if (!text) return [];
  const taken: Mark[] = recognise(text).spans.map((s) => ({ start: s.start, end: s.end, kind: s.type }));
  const quoted = [...text.matchAll(/"[^"\n]*"|`[^`\n]*`/g)].map((m) => [m.index!, m.index! + m[0].length] as const);
  const free = (a: number, b: number) => !taken.some((t) => a < t.end && b > t.start) && !quoted.some(([x, y]) => a < y && b > x);
  for (const m of text.matchAll(MENTION_RE)) if (free(m.index!, m.index! + m[0].length) && !taken.some((t) => t.kind === "person" && t.start === m.index)) taken.push({ start: m.index!, end: m.index! + m[0].length, kind: "mention" });
  for (const m of text.matchAll(SKILL_RE)) { const start = m.index! + m[0].length - m[1]!.length; if (free(start, start + m[1]!.length)) taken.push({ start, end: start + m[1]!.length, kind: "skill" }); }
  return taken.sort((a, b) => a.start - b.start);
}
