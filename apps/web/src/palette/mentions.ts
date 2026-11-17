/**
 * What `@` and `/` can pick in the thread composer. One picker, grouped by kind: people, agents, threads and items for `@`; skills for `/` at the start of a
 * line. The same typo-tolerant search as the palette ranks inside the groups. Fixture data; the real lists come from the space (members, installed
 * agents, threads and items the person can reach) and the person's skills.
 */
import { buildIndex, search, type Item } from "./search";

export type MentionKind = "person" | "agent" | "thread" | "item" | "skill";
/** One pickable thing. `insert` is the text that replaces what was typed. */
export interface Mention { id: string; kind: MentionKind; label: string; sub?: string; insert: string }
/** A heading with its entries. */
export interface MentionGroup { kind: MentionKind; title: string; entries: Mention[] }

const TITLE: Record<MentionKind, string> = { person: "People", agent: "Agents", thread: "Threads", item: "Items", skill: "Skills" };
const ORDER: MentionKind[] = ["person", "agent", "thread", "item", "skill"];
const PEOPLE: [string, string][] = [["Kay", "Owner"], ["Priya", "Manager"], ["Sam", "Member"]];
const SKILLS: [string, string][] = [["review", "Code review"], ["release", "Release notes"], ["docs", "Write the docs"]];

/** A token that names someone or something: `@name`, or `@[a title with spaces]`. */
export const MENTION_RE = /@(?:\[[^\]\n]+\]|[\p{L}\d_-]+)/gu;
/** A skill at the start of a line: `/review`. */
export const SKILL_RE = /(?:^|\n)(\/[\p{L}\d_-]+)/gu;

const one = (s: string) => (/\s/.test(s) ? `@[${s}]` : `@${s}`);

/** Everything `@` can pick, from the open data. `agents` are the installed providers; `threads` and `items` come from the store and the notes. */
export function mentionPool(agents: string[], threads: { id: string; title: string }[], items: { id: string; title: string }[]): Mention[] {
  return [
    ...PEOPLE.map(([n, role]): Mention => ({ id: `p:${n}`, kind: "person", label: n, sub: role, insert: one(n) })),
    ...agents.map((n): Mention => ({ id: `a:${n}`, kind: "agent", label: n, sub: "Agent", insert: one(n) })),
    ...threads.map((t): Mention => ({ id: `t:${t.id}`, kind: "thread", label: t.title, sub: "Thread", insert: one(t.title) })),
    ...items.map((t): Mention => ({ id: `i:${t.id}`, kind: "item", label: t.title, sub: "Note", insert: one(t.title) })),
  ];
}
/** Every skill `/` can list. */
export const skillPool = (): Mention[] => SKILLS.map(([id, name]): Mention => ({ id: `s:${id}`, kind: "skill", label: name, sub: `/${id}`, insert: `/${id} ` }));

/** The matches for what was typed after the trigger, grouped by kind (an empty query lists a few of each). Cost: one pass over the pool. */
export function pick(pool: Mention[], query: string, perGroup = 4): MentionGroup[] {
  const q = query.trim();
  let hits: Mention[];
  if (!q) hits = pool;
  else {
    const byId = new Map(pool.map((m) => [m.id, m]));
    const ix = buildIndex(pool.map((m): Item => ({ id: m.id, kind: "note", title: m.label + " " + (m.sub ?? ""), space: "" })));
    hits = search(ix, q, 40).map((i) => byId.get(i.id)!);
  }
  return ORDER.map((kind) => ({ kind, title: TITLE[kind], entries: hits.filter((m) => m.kind === kind).slice(0, perGroup) })).filter((g) => g.entries.length);
}

/** The trigger the caret is in: `@` and what follows it, or a `/` at the start of a line, as a range of the text. */
export function triggerAt(text: string, caret: number): { kind: "mention" | "skill"; query: string; start: number } | null {
  const before = text.slice(0, caret);
  const at = /(?:^|[\s(])@([^\s@[\]]{0,40})$/u.exec(before);
  if (at) return { kind: "mention", query: at[1]!, start: caret - at[1]!.length - 1 };
  const sl = /(?:^|\n)\/([\p{L}\d_-]{0,30})$/u.exec(before);
  if (sl) return { kind: "skill", query: sl[1]!, start: caret - sl[1]!.length - 1 };
  return null;
}
