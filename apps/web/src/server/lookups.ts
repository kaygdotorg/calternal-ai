/**
 * The search and picker operations as the thread UI uses them: message search over `search.query`, and the composer's `@` and `/` pickers over `people.lookup`, `threads.lookup`, `items.lookup` and `skills.list`. Everything goes through
 * `ServerClient.operate`; rows are mapped to the shapes the UI already has (`MessageHit`, picker rows), so nothing downstream knows the wire. Each call is one request; the pickers ask once per pause (the composer debounces).
 */
import type { PeopleLookupResult, SearchHit, SearchResult, SkillsResult } from "../../../../packages/contracts/src/index";
import type { MessageHit } from "../apps/threads/model";
import type { Lookups, PickerHit } from "../apps/threads/client";
import { server } from "./client";

const idOf = (address: string): string => address.split("?")[0]!.split("#")[0]!.split("/").filter(Boolean).pop() ?? "";
const textOf = (h: SearchHit) => h.excerpt.map((s) => s.text).join("");

/** A search hit as a result row: the thread it is in (the address names it), the excerpt as the line with the first match marked, and, when the server says which message matched, its id and time (`threads.get` with `message_id` opens that record; the list scrolls to it). Without them the thread opens at its end. */
export function hitOf(h: SearchHit): MessageHit {
  const line = textOf(h).trim();
  let start = 0, end = 0, pos = 0;
  for (const s of h.excerpt) { if (s.matched) { start = pos; end = pos + s.text.length; break; } pos += s.text.length; }
  const lead = textOf(h).length - textOf(h).trimStart().length;
  const when = h.message_at ? Date.parse(h.message_at) : NaN;
  return { threadId: idOf(h.address), threadTitle: h.title, line, start: Math.max(0, start - lead), end: Math.max(0, end - lead), ...(h.message_id ? { messageId: h.message_id } : {}), ...(Number.isFinite(when) ? { at: when } : {}) };
}
const pickerOf = (h: SearchHit): PickerHit => ({ id: idOf(h.address), title: h.title, kind: h.kind, space: h.space_id });

/** The wire-backed lookups for a space (`space` is read per call: the person may switch). */
export function lookups(space: () => string): Lookups {
  return {
    async searchMessages(query, limit = 50) {
      const r = await server.operate<SearchResult>("search.query", { query, kinds: ["threads"], limit: Math.min(50, limit) });
      return { hits: r.hits.map(hitOf), partial: !!r.partial };
    },
    async people(prefix) { return (await server.operate<PeopleLookupResult>("people.lookup", { space_id: space(), prefix })).people.map((p) => ({ id: p.id, name: p.name, kind: p.kind })); },
    async skills(prefix) { return (await server.operate<SkillsResult>("skills.list", { space_id: space(), prefix })).skills; },
    async threads(prefix) { return (await server.operate<SearchResult>("threads.lookup", { prefix, limit: 8 })).hits.map(pickerOf); },
    async items(prefix) { return (await server.operate<SearchResult>("items.lookup", { prefix, limit: 8 })).hits.map(pickerOf); },
  };
}
