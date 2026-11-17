/**
 * Tables, drawn as a grid. The note keeps plain GFM text; a table the caret is not in is replaced by a real `<table>` (header row, alignment from the delimiter row, a small inline
 * clean-up of `code` and `**bold**` marks), and the moment the caret enters it (an arrow key, a click on the grid) the raw rows come back to be edited. This is the lazy piece, loaded by
 * `features.ts` when the note holds a table. Editing inside the grid, column widths and the table bar are later stages; not built here.
 * Cost: one walk of the top-level blocks when the parse tree or the selection changes (no walk into paragraphs, code or cells), one small DOM per drawn table.
 */
import { syntaxTree } from "@codemirror/language";
import { type EditorState, type Extension, RangeSet, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, WidgetType } from "@codemirror/view";

type Align = "left" | "center" | "right" | null;

/** Splits one table row into its cells (a `\|` stays inside a cell). */
export function cellsOf(row: string): string[] {
  const t = row.trim().replace(/^\|/, "").replace(/\|$/, "");
  const out: string[] = [];
  let cur = "";
  for (let i = 0; i < t.length; i++) {
    if (t[i] === "\\" && t[i + 1] === "|") { cur += "|"; i++; }
    else if (t[i] === "|") { out.push(cur.trim()); cur = ""; }
    else cur += t[i];
  }
  out.push(cur.trim());
  return out;
}
/** The alignment of each column from the delimiter row (`:--`, `:-:`, `--:`). */
export function alignsOf(row: string): Align[] {
  return cellsOf(row).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : c.startsWith(":") ? "left" : null));
}
const plain = (s: string) => s.replace(/`([^`]*)`/g, "$1").replace(/\*\*([^*]+)\*\*/g, "$1").replace(/(^|[^*])\*([^*]+)\*/g, "$1$2").replace(/~~([^~]+)~~/g, "$1");

class Grid extends WidgetType {
  constructor(readonly text: string, readonly from: number) { super(); }
  eq(o: Grid) { return o.text === this.text && o.from === this.from; }
  toDOM() {
    const lines = this.text.split("\n").filter((l) => l.trim());
    const wrap = document.createElement("div");
    wrap.className = "cm-grid";
    wrap.dataset.from = String(this.from);
    const table = document.createElement("table");
    const head = cellsOf(lines[0] ?? "");
    const aligns = alignsOf(lines[1] ?? "");
    const row = (cells: string[], tag: "th" | "td") => {
      const tr = document.createElement("tr");
      for (let i = 0; i < head.length; i++) { const c = document.createElement(tag); c.textContent = plain(cells[i] ?? ""); if (aligns[i]) c.style.textAlign = aligns[i]!; tr.append(c); }
      return tr;
    };
    const thead = document.createElement("thead"); thead.append(row(head, "th"));
    const tbody = document.createElement("tbody");
    for (const l of lines.slice(2)) tbody.append(row(cellsOf(l), "td"));
    table.append(thead, tbody);
    wrap.append(table);
    return wrap;
  }
  ignoreEvent() { return false; }
}

function build(state: EditorState): DecorationSet {
  const sel = state.selection.main;
  const ranges: ReturnType<Decoration["range"]>[] = [];
  syntaxTree(state).iterate({
    enter(n) {
      if (n.name === "Table") {
        // The raw rows stay while the caret or selection touches the table.
        if (!(sel.to >= n.from && sel.from <= n.to + 1)) ranges.push(Decoration.replace({ block: true, widget: new Grid(state.doc.sliceString(n.from, n.to), n.from) }).range(n.from, n.to));
        return false;
      }
      return /^(Document|Blockquote|BulletList|OrderedList|ListItem)$/.test(n.name);
    },
  });
  return Decoration.set(ranges, true);
}

const field = StateField.define<DecorationSet>({
  create: build,
  update: (d, tr) => (tr.docChanged || tr.selection || syntaxTree(tr.startState) !== syntaxTree(tr.state) ? build(tr.state) : d),
  provide: (f) => [EditorView.decorations.from(f), EditorView.atomicRanges.of((v) => v.state.field(f, false) ?? RangeSet.empty)],
});

const look = EditorView.baseTheme({
  ".cm-grid": { margin: "var(--s-3) 0", overflowX: "auto", cursor: "text" },
  ".cm-grid table": { borderCollapse: "collapse", fontSize: "var(--text-ui)", minWidth: "50%" },
  ".cm-grid th, .cm-grid td": { padding: "var(--s-2) var(--s-5)", borderBottom: "1px solid var(--line)", textAlign: "left", verticalAlign: "top" },
  ".cm-grid th": { fontWeight: "var(--strong-weight)", color: "var(--ink-2)" },
});

/** Clicking the grid puts the caret at the table's start, which brings the raw rows back. */
const reveal = EditorView.domEventHandlers({
  mousedown(e, view) {
    const g = (e.target as HTMLElement).closest?.(".cm-grid") as HTMLElement | null;
    if (!g) return false;
    view.dispatch({ selection: { anchor: Number(g.dataset.from) }, scrollIntoView: true });
    view.focus();
    e.preventDefault();
    return true;
  },
});

/** The table grid extension. */
export const tables: Extension = [field, look, reveal];
