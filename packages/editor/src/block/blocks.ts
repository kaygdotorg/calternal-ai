/**
 * Blocks: what a person moves, duplicates and deletes as one thing. A block is derived from the parse tree (it has no stored identity here; ids are `^id` markers in the text, owned by one
 * module in a later stage): a paragraph, a heading, a fenced code block, a quote, a horizontal rule, a table, an HTML block, or ONE list item (a nested item is its own block).
 * The host classifies blocks with `kinds` (the Notes app says which are tasks, say); the editor package knows no application kinds.
 */
import { syntaxTree } from "@codemirror/language";
import type { ChangeSpec, EditorState } from "@codemirror/state";

export interface Block { from: number; to: number; kind: string; depth: number }
/** The host's own classification: return a kind name to override the default, or null to keep it. */
export type Kinds = (block: Block, state: EditorState) => string | null;

const TOP = new Set(["Paragraph", "FencedCode", "CodeBlock", "Blockquote", "HorizontalRule", "Table", "HTMLBlock", "LinkReference"]);
const kindOf = (name: string): string => (name.startsWith("ATXHeading") || name.startsWith("SetextHeading") ? `heading${name.at(-1)}` : name === "FencedCode" || name === "CodeBlock" ? "code" : name === "HorizontalRule" ? "rule" : name === "Blockquote" ? "quote" : name === "Table" ? "table" : name === "HTMLBlock" ? "html" : name === "ListItem" ? "item" : "paragraph");

/** Every block of the document, in order, between `from` and `to` (the whole document by default). Cost: one walk of the parsed tree. */
export function blocksIn(state: EditorState, from = 0, to = state.doc.length, kinds?: Kinds): Block[] {
  const out: Block[] = [];
  syntaxTree(state).iterate({
    from, to,
    enter(n) {
      const name = n.name;
      if (name === "Paragraph" && n.node.parent?.name === "ListItem") return false; // an item's own text is the item's block
      const heading = /^(ATX|Setext)Heading[1-6]$/.test(name);
      if (name === "ListItem" || heading || TOP.has(name)) {
        let depth = 0;
        for (let p = n.node.parent; p; p = p.parent) if (p.name === "ListItem") depth++;
        const b: Block = { from: n.from, to: n.to, kind: kindOf(name), depth };
        // A list item's own block ends where its first child ends; nested items follow as blocks of their own.
        if (name === "ListItem") { const sub = n.node.getChild("BulletList") ?? n.node.getChild("OrderedList"); if (sub) b.to = sub.from > b.from ? Math.max(b.from, state.doc.lineAt(sub.from).from - 1) : b.to; }
        const k = kinds?.(b, state);
        if (k) b.kind = k;
        out.push(b);
        if (name === "ListItem") return; // nested items are blocks of their own
        return false;
      }
    },
  });
  return out;
}

/** The innermost block that contains `pos`. */
export function blockAt(state: EditorState, pos: number, kinds?: Kinds): Block | null {
  let best: Block | null = null;
  for (const b of blocksIn(state, Math.max(0, pos), Math.min(state.doc.length, pos), kinds)) if (b.from <= pos && pos <= b.to && (!best || b.to - b.from <= best.to - best.from)) best = b;
  return best;
}

const lineStart = (s: EditorState, p: number) => s.doc.lineAt(p).from;
const lineEnd = (s: EditorState, p: number) => s.doc.lineAt(p).to;
/** The whole lines a block covers (from the start of its first line to the end of its last, with the newline after it when there is one). */
export function blockLines(state: EditorState, b: Block): { from: number; to: number } {
  const from = lineStart(state, b.from);
  const end = lineEnd(state, Math.max(b.from, b.to - 1));
  return { from, to: Math.min(state.doc.length, end + 1) };
}

/** The change that deletes a block (its lines). */
export function deleteBlock(state: EditorState, b: Block): ChangeSpec {
  const { from, to } = blockLines(state, b);
  return { from: from === to && from > 0 ? from - 1 : from, to };
}
/** The change that duplicates a block right after itself. */
export function duplicateBlock(state: EditorState, b: Block): ChangeSpec {
  const { from, to } = blockLines(state, b);
  const text = state.doc.sliceString(from, to);
  return { from: to, insert: text.endsWith("\n") ? text : `\n${text}` };
}
/** The change that moves a block before `target` (a document position at a line start), or null when it would not move. One change set, so it is one undo step. */
export function moveBlock(state: EditorState, b: Block, target: number): ChangeSpec[] | null {
  const { from, to } = blockLines(state, b);
  if (target >= from && target <= to) return null;
  let text = state.doc.sliceString(from, to);
  if (!text.endsWith("\n")) text += "\n";
  const t = lineStart(state, Math.min(target, state.doc.length));
  const atEnd = target >= state.doc.length;
  return [{ from: atEnd ? state.doc.length : t, insert: atEnd && !state.doc.sliceString(state.doc.length - 1).endsWith("\n") ? `\n${text.replace(/\n$/, "")}` : text }, deleteBlock(state, b) as ChangeSpec];
}
