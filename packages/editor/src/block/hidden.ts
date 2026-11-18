/**
 * The drawn document: Markdown syntax is never shown. The note stays plain Markdown in the text; `# `, `**`, backticks, `> `, list markers and `[ ]` are hidden or drawn as a bullet or a
 * checkbox, link syntax around a link's words is hidden (the words become a chip), and fenced code and rules draw as blocks. Hidden ranges are atomic (the caret jumps over them, Backspace
 * removes one whole). Decorations are built from a parsed tree for the visible ranges, so a note never flashes raw syntax and then reflows (see parse.ts).
 * Simplified from the existing editor's `syntax/hidden.ts`: frontmatter, embeds and the table grid are not here.
 */
import { syntaxTree } from "@codemirror/language";
import { IterMode } from "@lezer/common";
import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from "@codemirror/view";

class Bullet extends WidgetType {
  toDOM() { const s = document.createElement("span"); s.className = "cm-bullet"; s.textContent = "•"; return s; }
  eq() { return true; }
}
class Check extends WidgetType {
  constructor(readonly done: boolean, readonly pos: number) { super(); }
  toDOM() { const s = document.createElement("span"); s.className = `cm-check${this.done ? " on" : ""}`; s.setAttribute("role", "checkbox"); s.setAttribute("aria-checked", String(this.done)); s.dataset.pos = String(this.pos); return s; }
  eq(o: Check) { return o.done === this.done; }
  ignoreEvent(e: Event) { return e.type !== "mousedown"; }
}
class Rule extends WidgetType {
  toDOM() { const s = document.createElement("div"); s.className = "cm-rule"; return s; }
  eq() { return true; }
}

const hide = Decoration.replace({});
const marks = { Emphasis: Decoration.mark({ class: "cm-em" }), StrongEmphasis: Decoration.mark({ class: "cm-strong" }), Strikethrough: Decoration.mark({ class: "cm-strike" }), InlineCode: Decoration.mark({ class: "cm-code" }) } as const;
const line = (cls: string) => Decoration.line({ class: cls });
const TASK = /^(\s*)([-*+]|\d+[.)])( +)(\[([ xX])\]( |$))?/;

/** What the formatting-hiding pass produced for the viewport: the decorations to draw and the subset that the caret treats as atomic (skipped over). */
export interface Drawn { decorations: DecorationSet; atomic: DecorationSet }

/** Builds the decorations for the given ranges of the document. */
export function buildDrawn(state: EditorState, ranges: readonly { from: number; to: number }[] = [{ from: 0, to: state.doc.length }]): Drawn {
  const deco: Range<Decoration>[] = [];
  const atomic: Range<Decoration>[] = [];
  const doc = state.doc;
  const replace = (from: number, to: number, d: Decoration = hide) => { if (to > from) { deco.push(d.range(from, to)); atomic.push(d.range(from, to)); } };
  const spaceAfter = (p: number) => (p < doc.length && doc.sliceString(p, p + 1) === " " ? 1 : 0);
  const eachLine = (from: number, to: number, cls: string) => { for (let p = from; p <= to; ) { const l = doc.lineAt(p); deco.push(line(cls).range(l.from)); p = l.to + 1; } };

  // A blank line between blocks is a small gap, not a full empty line.
  for (const r of ranges) for (let p = r.from; p <= r.to; ) { const l = doc.lineAt(p); if (l.length === 0) deco.push(line("cm-blank").range(l.from)); p = l.to + 1; }
  for (const r of ranges) {
    syntaxTree(state).iterate({
    mode: IterMode.IgnoreMounts, // never walk into a code fence's own grammar: nothing here looks at it, and a nested tree is many more nodes per key
      from: r.from, to: r.to,
      enter(n) {
        const { name, from, to } = n;
        if (/^(ATX|Setext)Heading[1-6]$/.test(name)) { deco.push(line(`cm-h cm-h${name.at(-1)}`).range(doc.lineAt(from).from)); return; }
        switch (name) {
          case "HeaderMark": { const l = doc.lineAt(from); if (from === l.from) replace(from, to + spaceAfter(to)); else replace(from - (doc.sliceString(from - 1, from) === " " ? 1 : 0), to); return; }
          case "EmphasisMark": case "StrikethroughMark": replace(from, to); return;
          case "Emphasis": case "StrongEmphasis": case "Strikethrough": deco.push(marks[name].range(from, to)); return;
          case "InlineCode": { deco.push(marks.InlineCode.range(from, to)); for (let c = n.node.firstChild; c; c = c.nextSibling) if (c.name === "CodeMark") replace(c.from, c.to); return false; }
          case "FencedCode": {
            const first = doc.lineAt(from), last = doc.lineAt(to);
            eachLine(first.from, last.to, "cm-codeblock");
            deco.push(line("cm-fence").range(first.from));
            replace(first.from, first.to);
            if (last.number > first.number && /^\s*(```|~~~)/.test(last.text)) { deco.push(line("cm-fence").range(last.from)); replace(last.from, last.to); }
            return false;
          }
          case "Link": {
            const marksAt = [] as { from: number; to: number; name: string }[];
            for (let c = n.node.firstChild; c; c = c.nextSibling) marksAt.push({ from: c.from, to: c.to, name: c.name });
            const first = marksAt.find((m) => m.name === "LinkMark");
            const close = marksAt.filter((m) => m.name === "LinkMark")[1];
            if (first && close) {
              const url = n.node.getChild("URL");
              replace(first.from, first.to);
              deco.push(Decoration.mark({ class: "cm-link", attributes: { "data-href": url ? doc.sliceString(url.from, url.to) : "" } }).range(first.to, close.from));
              replace(close.from, to);
            }
            return false;
          }
          case "Image": return false;
          case "Blockquote": eachLine(from, to, "cm-quote"); return;
          case "QuoteMark": replace(from, to + spaceAfter(to)); return;
          case "HorizontalRule": replace(from, to, Decoration.replace({ widget: new Rule() })); return false;
          case "ListItem": {
            const l = doc.lineAt(from);
            const m = TASK.exec(l.text);
            if (!m) return;
            const mark = m[2]!;
            const markFrom = l.from + m[1]!.length;
            if (/^\d/.test(mark)) { deco.push(Decoration.mark({ class: "cm-num" }).range(markFrom, markFrom + mark.length)); }
            else if (m[4]) {
              const done = m[5] !== " ";
              replace(markFrom, l.from + m[0].length - (m[6] ? 1 : 0), Decoration.replace({ widget: new Check(done, l.from + m[1]!.length + m[2]!.length + m[3]!.length) }));
              if (done) deco.push(Decoration.mark({ class: "cm-done" }).range(l.from + m[0].length, l.to));
            } else replace(markFrom, markFrom + mark.length + m[3]!.length, Decoration.replace({ widget: new Bullet() }));
            return;
          }
          case "Table": eachLine(from, to, "cm-table"); return false;
        }
      },
    });
  }
  const sort = (a: Range<Decoration>[]) => Decoration.set(a, true);
  return { decorations: sort(deco), atomic: sort(atomic) };
}

/** The view plugin that keeps the drawn decorations current for the visible ranges, and makes the hidden parts atomic. Clicking a checkbox toggles it (the person's own change). */
export const drawn = [
  ViewPlugin.fromClass(class {
    decorations: DecorationSet;
    atomic: DecorationSet;
    constructor(view: EditorView) { ({ decorations: this.decorations, atomic: this.atomic } = buildDrawn(view.state, view.visibleRanges)); }
    update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) ({ decorations: this.decorations, atomic: this.atomic } = buildDrawn(u.state, u.view.visibleRanges)); }
  }, {
    decorations: (v) => v.decorations,
    provide: (p) => EditorView.atomicRanges.of((view) => view.plugin(p)?.atomic ?? Decoration.none),
    eventHandlers: {
      mousedown(e, view) {
        const t = e.target as HTMLElement;
        if (!t.classList?.contains("cm-check")) return false;
        const pos = Number(t.dataset.pos);
        const l = view.state.doc.lineAt(pos);
        const m = /\[([ xX])\]/.exec(l.text);
        if (!m) return false;
        const i = l.from + m.index + 1;
        view.dispatch({ changes: { from: i, to: i + 1, insert: m[1] === " " ? "x" : " " }, userEvent: "input" });
        e.preventDefault();
        return true;
      },
    },
  }),
];
