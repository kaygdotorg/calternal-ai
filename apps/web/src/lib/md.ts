/**
 * A small Markdown subset for messages: paragraphs, headings, bullet and numbered lists, fenced code, and inline code, bold, italic
 * and http(s) links. It returns a typed tree the components draw (no HTML strings, so nothing in a message can inject markup).
 * Cost: one pass over the text; a streaming message re-parses only its own text.
 */

/** Inline content. */
export type Inline = { t: "text"; v: string } | { t: "code"; v: string } | { t: "b"; v: string } | { t: "i"; v: string } | { t: "a"; v: string; href: string };

/** A block. */
export type MdBlock =
  | { t: "p"; v: Inline[] }
  | { t: "h"; v: Inline[] }
  | { t: "ul"; v: Inline[][] }
  | { t: "ol"; v: Inline[][] }
  | { t: "code"; lang: string; v: string }
  | { t: "img"; alt: string; src: string };

const INLINE = /(`[^`\n]+`)|(\*\*[^*\n]+\*\*)|(\*[^*\n]+\*)|(\[[^\]\n]+\]\(https?:\/\/[^)\s]+\))/g;

/** Parses one line of inline Markdown. */
export function inline(text: string): Inline[] {
  const out: Inline[] = [];
  let at = 0;
  for (const m of text.matchAll(INLINE)) {
    if (m.index! > at) out.push({ t: "text", v: text.slice(at, m.index) });
    const s = m[0];
    if (m[1]) out.push({ t: "code", v: s.slice(1, -1) });
    else if (m[2]) out.push({ t: "b", v: s.slice(2, -2) });
    else if (m[3]) out.push({ t: "i", v: s.slice(1, -1) });
    else {
      const k = s.indexOf("](");
      out.push({ t: "a", v: s.slice(1, k), href: s.slice(k + 2, -1) });
    }
    at = m.index! + s.length;
  }
  if (at < text.length) out.push({ t: "text", v: text.slice(at) });
  return out;
}

/** Parses Markdown into blocks. */
export function parse(src: string): MdBlock[] {
  const lines = src.split("\n");
  const out: MdBlock[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (!line.trim()) { i++; continue; }
    const img = /^!\[([^\]\n]*)\]\(((?:https?:\/\/|data:image\/(?:png|jpeg|webp|svg\+xml)[;,])[^)\s]+)\)\s*$/.exec(line);
    if (img) { out.push({ t: "img", alt: img[1]!, src: img[2]! }); i++; continue; }
    const fence = /^```(\w*)\s*$/.exec(line);
    if (fence) {
      const body: string[] = [];
      i++;
      while (i < lines.length && !/^```\s*$/.test(lines[i]!)) body.push(lines[i++]!);
      i++;
      out.push({ t: "code", lang: fence[1]!, v: body.join("\n") });
    } else if (/^#{1,4}\s/.test(line)) {
      out.push({ t: "h", v: inline(line.replace(/^#+\s+/, "")) });
      i++;
    } else if (/^[-*]\s/.test(line)) {
      const items: Inline[][] = [];
      while (i < lines.length && /^[-*]\s/.test(lines[i]!)) items.push(inline(lines[i++]!.replace(/^[-*]\s+/, "")));
      out.push({ t: "ul", v: items });
    } else if (/^\d+\.\s/.test(line)) {
      const items: Inline[][] = [];
      while (i < lines.length && /^\d+\.\s/.test(lines[i]!)) items.push(inline(lines[i++]!.replace(/^\d+\.\s+/, "")));
      out.push({ t: "ol", v: items });
    } else {
      const para: string[] = [];
      while (i < lines.length && lines[i]!.trim() && !/^(```|#{1,4}\s|[-*]\s|\d+\.\s)/.test(lines[i]!)) para.push(lines[i++]!);
      out.push({ t: "p", v: inline(para.join(" ")) });
    }
  }
  return out;
}
