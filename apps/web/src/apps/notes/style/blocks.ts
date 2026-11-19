/**
 * Block kinds as text: what kind of block a line is, the changes that turn the lines under the selection into another kind, and the bold toggle. Pure over `EditorState`, so the Style sheet's block tab
 * is tested without a view. Ported from the existing app's `lib/editing.svelte.ts` (`typeOfLine`, `setType`, `toggleMark`; origin/dev) with the same rules: a heading, list, to-do, quote or code fence
 * is only a line prefix (a code block wraps the lines in a fence), a list item keeps its indent, and bold on a bare caret opens an empty pair that a second press takes back.
 */
import { EditorSelection, type ChangeSpec, type EditorState, type TransactionSpec } from "@codemirror/state";
import { blockEnd } from "./frontmatter";

/** The kinds the Style sheet offers. */
export type BlockType = "h1" | "h2" | "h3" | "p" | "li" | "ol" | "todo" | "quote" | "code" | "divider";

const LIST = /^(\s*)([-*+]|\d+[.)])( +)(\[([ xX/-])\] ?)?/;

/** What kind of block a line starts. */
export function typeOfLine(text: string): BlockType {
  const t = text.trimStart();
  if (t.startsWith("### ")) return "h3";
  if (t.startsWith("## ")) return "h2";
  if (t.startsWith("# ")) return "h1";
  if (t.startsWith("> ")) return "quote";
  if (t.startsWith("```") || t.startsWith("~~~")) return "code";
  if (/^(---|\*\*\*|___)\s*$/.test(t)) return "divider";
  const m = LIST.exec(text);
  if (m) return m[4] ? "todo" : /\d/.test(m[2]!) ? "ol" : "li";
  return "p";
}

const PREFIX: Record<BlockType, string> = { h1: "# ", h2: "## ", h3: "### ", p: "", li: "- ", ol: "1. ", todo: "- [ ] ", quote: "> ", code: "", divider: "" };

/** The kind of the block under the caret, and whether the selection (or the caret's empty pair) is bold. Reads one line. */
export function caretFacts(state: EditorState): { type: BlockType; bold: boolean } {
  const sel = state.selection.main, d = state.doc;
  const bold = d.sliceString(Math.max(0, sel.from - 2), sel.from) === "**" && d.sliceString(sel.to, sel.to + 2) === "**";
  return { type: typeOfLine(d.lineAt(sel.head).text), bold };
}

/** The changes that make the lines under the selection (never the frontmatter) into `type`. */
export function convertChanges(state: EditorState, type: BlockType): ChangeSpec[] {
  const doc = state.doc, body = blockEnd(doc), sel = state.selection.main;
  const from = Math.max(doc.lineAt(sel.from).from, body), to = doc.lineAt(Math.max(sel.to, from)).to;
  if (type === "code") {
    const text = doc.sliceString(from, to).replace(/^\s*([-*+]|\d+[.)])\s+(\[[ xX/-]\]\s+)?|^#{1,6}\s+|^>\s?/gm, "");
    return [{ from, to, insert: "```\n" + text + "\n```" }];
  }
  if (type === "divider") return [{ from, to, insert: "---" }];
  const changes: ChangeSpec[] = [];
  for (let pos = from; pos <= to; ) {
    const line = doc.lineAt(pos);
    const lead = /^(\s*)(#{1,6} |> ?|([-*+]|\d+[.)]) +(\[[ xX/-]\] ?)?)?/.exec(line.text)!;
    const keepIndent = type === "li" || type === "ol" || type === "todo" ? (lead[1] ?? "") : "";
    changes.push({ from: line.from, to: line.from + lead[0].length, insert: keepIndent + PREFIX[type] });
    if (line.to >= to) break;
    pos = line.to + 1;
  }
  return changes;
}

/** Toggles bold on the selection (on a bare caret: opens an empty `****` pair, which a second press removes). */
export function toggleBold(state: EditorState): TransactionSpec {
  const { from, to } = state.selection.main, d = state.doc, m = "**";
  const around = d.sliceString(Math.max(0, from - 2), from) === m && d.sliceString(to, to + 2) === m;
  if (from === to) return around ? { changes: { from: from - 2, to: from + 2 }, selection: EditorSelection.cursor(from - 2) } : { changes: { from, insert: m + m }, selection: EditorSelection.cursor(from + 2) };
  if (around) return { changes: [{ from: from - 2, to: from }, { from: to, to: to + 2 }], selection: EditorSelection.range(from - 2, to - 2) };
  return { changes: [{ from, insert: m }, { from: to, insert: m }], selection: EditorSelection.range(from + 2, to + 2) };
}
