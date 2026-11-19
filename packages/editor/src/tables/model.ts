// A table as the file holds it: GFM pipe text (T1), nothing extra stored. This file is plain data in,
// plain data out (no DOM), so every edit is checked without a browser (`bun test src/tables`).
//
//   | Exercise | Sets | Reps |   kg |
//   | :------- | ---: | ---: | ---: |
//   | Bench    |    4 |    8 |   60 |
//
// Row 0 is the header. `align` is what the delimiter row says, exactly: "" is `---`, which the screen
// may still draw right-aligned for a column of numbers (T6), but the file keeps what the user wrote.

export type Align = "" | "l" | "c" | "r";
export type Tbl = { rows: string[][]; align: Align[] };

const DELIM_CELL = /^\s*:?-+:?\s*$/;

/** Splits one line at pipes that are not escaped; a leading and a trailing pipe are only the frame. */
export function splitRow(line: string): string[] {
  let s = line.trim();
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|") && !s.endsWith("\\|")) s = s.slice(0, -1);
  const out: string[] = [];
  let cur = "";
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (ch === "\\" && s[i + 1] === "|") {
      cur += "|";
      i++;
    } else if (ch === "|") {
      out.push(cur.trim());
      cur = "";
    } else cur += ch;
  }
  out.push(cur.trim());
  return out;
}

/** Replaces one raw cell, giving it one frame space while leaving every neighboring cell untouched. */
export function replaceCellLine(line: string, column: number, value: string): string {
  if (!Number.isInteger(column) || column < 0) return line
  const start = line.search(/\S|$/)
  const end = line.trimEnd().length
  const pipes: number[] = []
  for (let i = start; i < end; i++) {
    if (line[i] === '\\' && line[i + 1] === '|') {
      i++
    } else if (line[i] === '|') pipes.push(i)
  }
  const framedLeft = pipes[0] === start
  const framedRight = pipes.at(-1) === end - 1
  const cellAreaEnd = framedRight ? pipes.at(-1)! : end
  const separators = pipes.slice(framedLeft ? 1 : 0, framedRight ? -1 : undefined)
  let cellStart = framedLeft ? pipes[0]! + 1 : start
  for (let i = 0; i <= column; i++) {
    const cellEnd = separators[i] ?? cellAreaEnd
    if (i === column) {
      const leftGap = framedLeft || column > 0
      const rightGap = framedRight || column < separators.length
      return (
        line.slice(0, cellStart) +
        (leftGap ? ' ' : '') +
        value.replace(/\|/g, '\\|') +
        (rightGap ? ' ' : '') +
        line.slice(cellEnd)
      )
    }
    if (cellEnd === cellAreaEnd) return line
    cellStart = cellEnd + 1
  }
  return line
}

const isDelim = (cells: string[]) => cells.length > 0 && cells.every((c) => DELIM_CELL.test(c));
const alignOf = (c: string): Align => {
  const t = c.trim();
  const l = t.startsWith(":");
  const r = t.endsWith(":");
  return l && r ? "c" : r ? "r" : l ? "l" : "";
};

/** Tidies only the frame spaces around a valid delimiter row, keeping its dash counts and alignment markers. */
export function tidyDelimiterLine(line: string): string {
  const cells = splitRow(line)
  return isDelim(cells) ? `| ${cells.join(' | ')} |` : line
}

/** A table block's text → cells. Rows are made equally long; a missing delimiter row is no alignment. */
export function parseTable(text: string): Tbl {
  const lines = text.split("\n").filter((l) => l.trim() !== "");
  const raw = lines.map(splitRow);
  let align: Align[] = [];
  const rows: string[][] = [];
  raw.forEach((cells, i) => {
    if (i === 1 && isDelim(cells)) align = cells.map(alignOf);
    else rows.push(cells);
  });
  if (!rows.length) rows.push([""]);
  const n = Math.max(1, ...rows.map((r) => r.length), align.length);
  for (const r of rows) while (r.length < n) r.push("");
  while (align.length < n) align.push("");
  return { rows, align: align.slice(0, n) };
}

const esc = (c: string) => c.replace(/\|/g, "\\|");
const len = (s: string) => [...s].length;

/** Cells → the exact GFM text, columns padded so the raw file reads as a table too. */
export function serializeTable(t: Tbl): string {
  const n = t.align.length;
  const cells = t.rows.map((r) => r.map(esc));
  const w = Array.from({ length: n }, (_, c) => Math.max(3, ...cells.map((r) => len(r[c] ?? ""))));
  const pad = (s: string, c: number) => {
    const gap = w[c]! - len(s);
    const a = t.align[c];
    if (a === "r") return " ".repeat(gap) + s;
    if (a === "c") return " ".repeat(Math.floor(gap / 2)) + s + " ".repeat(Math.ceil(gap / 2));
    return s + " ".repeat(gap);
  };
  const line = (r: string[]) => `| ${r.map((s, c) => pad(s, c)).join(" | ")} |`;
  const delim = t.align.map((a, c) => {
    const d = "-".repeat(w[c]!);
    return a === "l" ? ":" + d.slice(1) : a === "r" ? d.slice(1) + ":" : a === "c" ? ":" + d.slice(2) + ":" : d;
  });
  const out = [line(cells[0]!), `| ${delim.join(" | ")} |`];
  for (let i = 1; i < cells.length; i++) out.push(line(cells[i]!));
  return out.join("\n");
}

/** T6: a column whose cells (below the header) are all numbers, with at least one filled. */
const NUMBER = /^[-+−]?(?:[$€£₹¥]\s?)?\d[\d,]*(?:\.\d+)?\s?%?$|^[-+−]?(?:[$€£₹¥]\s?)?\.\d+\s?%?$/;
export function isNumeric(rows: string[][], c: number): boolean {
  let any = false;
  for (let r = 1; r < rows.length; r++) {
    const v = (rows[r]![c] ?? "").trim();
    if (!v) continue;
    if (!NUMBER.test(v)) return false;
    any = true;
  }
  return any;
}
export const numericCols = (t: Tbl) => t.align.map((_, c) => isNumeric(t.rows, c));
/** How the screen aligns a column: what the file says, else right for numbers. */
export const shownAlign = (a: Align, numeric: boolean): "l" | "c" | "r" => (a ? a : numeric ? "r" : "l");

// ---- Edits. Each returns a new table; the old one is left alone.

const blank = (n: number) => Array.from({ length: n }, () => "");
const clone = (t: Tbl): Tbl => ({ rows: t.rows.map((r) => r.slice()), align: t.align.slice() });

/** A fresh table: a header and `body` empty rows (T3: 2 rows by 3 columns under the header). */
export const emptyTable = (cols = 3, body = 2, head?: string[]): Tbl => ({
  rows: [head ? head.slice() : blank(cols), ...Array.from({ length: body }, () => blank(cols))],
  align: blank(cols) as Align[],
});

export function insertRow(t: Tbl, at: number): Tbl {
  const o = clone(t);
  o.rows.splice(Math.max(1, Math.min(at, o.rows.length)), 0, blank(o.align.length));
  return o;
}
export function deleteRow(t: Tbl, r: number): Tbl {
  const o = clone(t);
  if (r < 1 || o.rows.length <= 2) return o; // the header stays, and one row to type in
  o.rows.splice(r, 1);
  return o;
}
export function insertCol(t: Tbl, at: number): Tbl {
  const o = clone(t);
  const i = Math.max(0, Math.min(at, o.align.length));
  for (const r of o.rows) r.splice(i, 0, "");
  o.align.splice(i, 0, "");
  return o;
}
export function deleteCol(t: Tbl, c: number): Tbl {
  const o = clone(t);
  if (o.align.length <= 1) return o;
  for (const r of o.rows) r.splice(c, 1);
  o.align.splice(c, 1);
  return o;
}
export function moveRow(t: Tbl, from: number, to: number): Tbl {
  const o = clone(t);
  if (from < 1 || to < 1 || from === to || to >= o.rows.length) return o;
  o.rows.splice(to, 0, o.rows.splice(from, 1)[0]!);
  return o;
}
export function moveCol(t: Tbl, from: number, to: number): Tbl {
  const o = clone(t);
  const n = o.align.length;
  if (from === to || to < 0 || to >= n) return o;
  for (const r of o.rows) r.splice(to, 0, r.splice(from, 1)[0]!);
  o.align.splice(to, 0, o.align.splice(from, 1)[0]!);
  return o;
}
export function setAlign(t: Tbl, c: number, a: Align): Tbl {
  const o = clone(t);
  o.align[c] = a;
  return o;
}

const num = (s: string) => parseFloat(s.replace(/[^\d.+\-−]/g, "").replace("−", "-"));
/** The order the rows under the header take when sorted by one column: row numbers, A→Z or 1→9 (numbers by value), empty cells last. */
export function sortIndex(t: Tbl, c: number, dir: 1 | -1): number[] {
  const numeric = isNumeric(t.rows, c);
  const idx = t.rows.slice(1).map((_, i) => i + 1);
  const cmp = (a: string, b: string) => (numeric ? num(a) - num(b) : a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" }));
  return idx.sort((x, y) => {
    const a = t.rows[x]![c]!.trim();
    const b = t.rows[y]![c]!.trim();
    if (!a || !b) return a === b ? x - y : !a ? 1 : -1;
    return cmp(a, b) * dir || x - y;
  });
}
export function sortBy(t: Tbl, c: number, dir: 1 | -1): Tbl {
  const o = clone(t);
  o.rows = [o.rows[0]!, ...sortIndex(t, c, dir).map((i) => o.rows[i]!)];
  return o;
}

// ---- Coming from elsewhere (T3): a spreadsheet's tab-separated text, or a web page's <table>.

/** Tab-separated text (Excel, Numbers, Sheets) → rows, or null when it isn't one. */
export function fromTSV(text: string): string[][] | null {
  if (!text.includes("\t")) return null;
  const rows: string[][] = [];
  let row: string[] = [];
  let cur = "";
  let quoted = false;
  const s = text.replace(/\r\n?/g, "\n").replace(/\n$/, "");
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (quoted) {
      if (ch === '"' && s[i + 1] === '"') (cur += '"'), i++;
      else if (ch === '"') quoted = false;
      else cur += ch === "\n" ? " " : ch;
    } else if (ch === '"' && cur === "") quoted = true;
    else if (ch === "\t") (row.push(cur), (cur = ""));
    else if (ch === "\n") (row.push(cur), rows.push(row), (row = []), (cur = ""));
    else cur += ch;
  }
  row.push(cur);
  rows.push(row);
  return rectangular(rows.map((r) => r.map((c) => c.trim())));
}

/** The rows of the first <table> in some HTML (a page, a spreadsheet's clipboard), or null. */
export function fromHTML(html: string): string[][] | null {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const table = doc.querySelector("table");
  if (!table) return null;
  const rows = [...table.querySelectorAll("tr")]
    .filter((tr) => tr.closest("table") === table)
    .map((tr) => [...tr.children].filter((c) => /^(TD|TH)$/.test(c.tagName)).map((c) => (c.textContent ?? "").replace(/\s+/g, " ").trim()));
  const full = rows.filter((r) => r.length);
  return full.length ? rectangular(full) : null;
}

function rectangular(rows: string[][]): string[][] {
  const n = Math.max(...rows.map((r) => r.length));
  return rows.map((r) => [...r, ...blank(n - r.length)]);
}

/** A line the user typed like `| a | b |`: its cells, or null (T3: Enter turns it into a table). */
export function pipeLine(text: string): string[] | null {
  const t = text.trim();
  if (!/^\|.*\|$/.test(t) || t.length < 3) return null;
  const cells = splitRow(t);
  return cells.length >= 2 || cells[0] !== "" ? cells : null;
}

/** Two tables with the same cells and alignment (so the file's own text can be left exactly as it is). */
export function sameTable(a: Tbl, b: Tbl): boolean {
  return (
    a.align.length === b.align.length &&
    a.align.every((x, i) => x === b.align[i]) &&
    a.rows.length === b.rows.length &&
    a.rows.every((r, i) => r.length === b.rows[i]!.length && r.every((v, j) => v === b.rows[i]![j]))
  )
}

/**
 * The text to write for an edited table. A table nobody changed keeps the file's own text, byte for byte
 * (spacing, `---` rows, ragged lines): the grid only ever rewrites what the person actually edited.
 */
export function textFor(original: string, t: Tbl): string {
  return sameTable(parseTable(original), t) ? original : serializeTable(t)
}
