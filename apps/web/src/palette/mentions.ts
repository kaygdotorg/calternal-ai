/**
 * What `@` and `/` can pick in the thread composer. One picker, grouped by kind: people, agents, threads and items for `@`; skills for `/` at the start of a
 * line. The same typo-tolerant search as the palette ranks inside the groups. Fixture data; the real lists come from the space (members, installed
 * agents, threads and items the person can reach) and the person's skills.
 */
import { buildIndex, search, type Item } from "./search";

export type MentionKind = "person" | "agent" | "thread" | "item" | "skill" | "tag" | "block";
/** One pickable thing. `insert` is the text that replaces what was typed; `caret` is where the caret goes inside it (default: after it). */
export interface Mention { id: string; kind: MentionKind; label: string; sub?: string; insert: string; caret?: number }
/** A heading with its entries. */
export interface MentionGroup { kind: MentionKind; title: string; entries: Mention[] }

const TITLE: Record<MentionKind, string> = { person: "People", agent: "Agents", thread: "Threads", item: "Items", skill: "Skills", tag: "Tags", block: "Insert" };
const ORDER: MentionKind[] = ["person", "agent", "thread", "item", "skill", "tag", "block"];
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
const SKILL_POOL: Mention[] = SKILLS.map(([id, name]): Mention => ({ id: `s:${id}`, kind: "skill", label: name, sub: `/${id}`, insert: `/${id} ` }));
export const skillPool = (): Mention[] => SKILL_POOL; // one array, so its search index is built once

const indexes = new WeakMap<Mention[], { byId: Map<string, Mention>; ix: ReturnType<typeof buildIndex> }>();
/** The search index of a pool, built once per pool (the pool changes when the data does, never per key). */
function indexOf(pool: Mention[]) {
  let e = indexes.get(pool);
  if (!e) { e = { byId: new Map(pool.map((m) => [m.id, m])), ix: buildIndex(pool.map((m): Item => ({ id: m.id, kind: "note", title: m.label + " " + (m.sub ?? ""), space: "" }))) }; indexes.set(pool, e); }
  return e;
}
/** Builds a pool's index ahead of the first query, for an idle moment (the first key after `@` should cost what the others do). */
export const warmPool = (pool: Mention[]): void => { indexOf(pool); };

/** The matches for what was typed after the trigger, grouped by kind (an empty query lists a few of each). Cost: one pass over the pool. */
export function pick(pool: Mention[], query: string, perGroup = 4): MentionGroup[] {
  const q = query.trim();
  let hits: Mention[];
  if (!q) hits = pool;
  else {
    const { byId, ix } = indexOf(pool);
    hits = search(ix, q, 40).map((i) => byId.get(i.id)!);
  }
  return ORDER.map((kind) => ({ kind, title: TITLE[kind], entries: hits.filter((m) => m.kind === kind).slice(0, perGroup) })).filter((g) => g.entries.length);
}

/** What `[[` can pick in a note: the notes of the space, inserted as `[[Title]]`. */
export const notePool = (items: { id: string; title: string }[]): Mention[] => items.map((t): Mention => ({ id: `i:${t.id}`, kind: "item", label: t.title, sub: "Note", insert: `[[${t.title}]]` }));
/** What `#` can pick in a note: the tags in use, and, when no tag in use starts with the typed word, the word itself as a new tag. */
export function tagPool(tags: string[], typed = ""): Mention[] {
  const pool = tags.map((t): Mention => ({ id: `g:${t}`, kind: "tag", label: `#${t}`, sub: "Tag", insert: `#${t}` }));
  const w = typed.trim();
  return w && !tags.some((t) => t.toLowerCase().startsWith(w.toLowerCase())) ? [{ id: `g:${w}`, kind: "tag", label: `#${w}`, sub: "New tag", insert: `#${w}` }, ...pool] : pool;
}
/** What `/` offers in a note: the blocks that can be inserted. Each replaces the typed `/word` with Markdown, which is all a block is. */
const BLOCK_POOL: Mention[] = [
  { id: "b:h1", kind: "block", label: "Heading 1", sub: "#", insert: "# " },
  { id: "b:h2", kind: "block", label: "Heading 2", sub: "##", insert: "## " },
  { id: "b:h3", kind: "block", label: "Heading 3", sub: "###", insert: "### " },
  { id: "b:bullet", kind: "block", label: "Bulleted list", sub: "-", insert: "- " },
  { id: "b:task", kind: "block", label: "Task", sub: "- [ ]", insert: "- [ ] " },
  { id: "b:quote", kind: "block", label: "Quote", sub: ">", insert: "> " },
  { id: "b:code", kind: "block", label: "Code block", sub: "```", insert: "```\n\n```", caret: 4 },
  { id: "b:table", kind: "block", label: "Table", sub: "|", insert: "| Column | Column |\n| --- | --- |\n|  |  |", caret: 2 },
  { id: "b:rule", kind: "block", label: "Divider", sub: "---", insert: "---\n" },
];
export const blockPool = (): Mention[] => BLOCK_POOL;
