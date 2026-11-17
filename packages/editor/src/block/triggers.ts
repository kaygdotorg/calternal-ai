/**
 * Inline pickers, host-drawn: the editor only says where a trigger is. `@name`, `[[title`, `#tag` and a `/` at the start of a line are recognised at the caret (only when the caret
 * moved by the person's own typing or navigation, never because remote text arrived); the host shows its picker (the shell's palette engine) and calls `apply` to insert. Quoted text and
 * code blocks never trigger. The package draws no UI.
 */
import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";
import { EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { origin } from "./origin";

export interface Trigger { kind: "mention" | "link" | "tag" | "skill"; query: string; from: number; to: number; rect: { left: number; right: number; top: number; bottom: number } | null }

const RULES: [Trigger["kind"], RegExp][] = [
  ["link", /\[\[([^\]\n]{0,60})$/u],
  ["mention", /(?:^|[\s(])@([^\s@[\]]{0,40})$/u],
  ["tag", /(?:^|[\s(])#([\p{L}\d_/-]{0,40})$/u],
  ["skill", /(?:^|\n)\/([\p{L}\d_-]{0,30})$/u],
];

/** The trigger at the caret, or null. Pure (the rectangle is added by the plugin). */
export function triggerAt(state: EditorState): Omit<Trigger, "rect"> | null {
  const sel = state.selection.main;
  if (!sel.empty) return null;
  const line = state.doc.lineAt(sel.head);
  const before = line.text.slice(0, sel.head - line.from);
  const node = syntaxTree(state).resolveInner(sel.head, -1);
  if (/Code|URL/.test(node.name)) return null;
  for (const [kind, re] of RULES) {
    const m = re.exec(kind === "skill" ? `\n${before}` : before);
    if (m) {
      const q = m[1]!;
      const lead = kind === "link" ? 2 : 1;
      return { kind, query: q, from: sel.head - q.length - lead, to: sel.head };
    }
  }
  return null;
}

/** Calls `on(trigger | null)` whenever the trigger at the caret changes. */
export function triggers(on: (t: Trigger | null) => void) {
  return ViewPlugin.fromClass(class {
    last = "";
    constructor(readonly view: EditorView) {}
    update(u: ViewUpdate) {
      if (!u.docChanged && !u.selectionSet) return;
      const remote = u.transactions.every((t) => t.docChanged && ["remote", "replay"].includes(t.annotation(origin) ?? ""));
      const t = remote ? null : triggerAt(u.state);
      const key = t ? `${t.kind}:${t.from}:${t.query}` : "";
      if (key === this.last) return;
      this.last = key;
      // The rectangle needs layout, so it is read after the update.
      queueMicrotask(() => on(t ? { ...t, rect: this.view.coordsAtPos(t.to) } : null));
    }
  });
}
