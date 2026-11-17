/**
 * Where a picker trigger is, in the text before the caret: `[[title` (a note), `@name` (a person, agent, thread or note), `#tag`, and `/word` at the start of a line (a skill, or the insert menu).
 * One set of rules for every text surface; a surface says which kinds it offers. Pure: no DOM, no picker UI (the host draws that). Quoted text and code never trigger: the block editor
 * asks the syntax tree after a match, which this module cannot know.
 */

/** Which picker. */
export type TriggerKind = "mention" | "link" | "tag" | "skill";
/** A match in the text before the caret: the kind, what was typed after the trigger characters, and how many trigger characters precede it. */
export interface Match { kind: TriggerKind; query: string; lead: number }

const RULES: [TriggerKind, RegExp, number][] = [
  ["link", /\[\[([^\]\n]{0,60})$/u, 2],
  ["mention", /(?:^|[\s(])@([^\s@[\]]{0,40})$/u, 1],
  ["tag", /(?:^|[\s(])#([\p{L}\d_/-]{0,40})$/u, 1],
  ["skill", /(?:^|\n)\/([\p{L}\d_-]{0,30})$/u, 1],
];

/** The trigger at the end of `before` (the text of the caret's line up to the caret), among `kinds`. Cheap when no trigger character is present. */
export function matchTrigger(before: string, kinds: readonly TriggerKind[] = ["link", "mention", "tag", "skill"]): Match | null {
  if (!/[@#/[]/.test(before)) return null;
  for (const [kind, re, lead] of RULES) {
    if (!kinds.includes(kind)) continue;
    const m = re.exec(kind === "skill" ? `\n${before}` : before);
    if (m) return { kind, query: m[1]!, lead };
  }
  return null;
}
