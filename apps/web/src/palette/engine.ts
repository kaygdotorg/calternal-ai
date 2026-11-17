/**
 * The palette's engine: one line in, rows out. It parses the line with the ported recogniser (no model, no network), searches every space, matches the
 * commands plugins gave as data, and says in one line what Enter will do. The thread composer will call the same `plan`. Nothing here touches the DOM.
 */
import { allCommands, type CommandDef } from "./manifest";
import { buildIndex, search, type Index, type Item } from "./search";
import { recognise } from "./phrases";
import type { Parsed } from "./recognise";

/** What running a row does. The host (Palette.svelte) turns it into navigation or an operation. */
export interface Action { go?: string; op?: string; ask?: string; text?: string }
/** One row of the list. */
export interface Row {
  id: string;
  kind: "open" | "command" | "ask" | "task" | "new";
  label: string;
  sub?: string;
  /** The space, when it is not the current one. */
  space?: string;
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

const KIND_ORDER = { command: 0, thread: 1, note: 2 } as const;

/** Plans one line. `provider` names the agent an "Ask" row would send to. */
export function plan(text: string, provider = "the agent"): Plan {
  const parsed = recognise(text);
  const q = text.trim();
  if (!q) return { parsed, intent: "", rows: [] };
  const found = search(index, q, 40).sort((a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind]).slice(0, 12);
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
  // Sending to an agent is outward: always its own row, last, never the default when something matches.
  rows.push({ id: "ask", kind: "ask", label: `Ask ${provider}: ${q}`, sub: "Send to the agent", action: { ask: q } });
  return { parsed, intent, rows };
}
