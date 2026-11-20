/** One piece of a message line in the closed inline subset. */
export type Seg = { t: "text" | "em" | "strong" | "code"; text: string } | { t: "link"; text: string; href: string };

const RE = /`([^`\n]+)`|\*\*([^*\n]+?)\*\*|(?<![\w*])\*([^*\n]+?)\*(?![\w*])|(?<![\w_])_([^_\n]+?)_(?![\w_])|\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/g;

/**
 * Splits a line into text, emphasis, strong text, code spans and explicit http(s) links (the contract's closed inline subset). Nothing else is interpreted: no HTML, no images, no bare URLs, no nesting. The result is
 * plain strings for the renderer to draw as text nodes, so a message can never inject markup.
 */
export function parseInline(s: string): Seg[] {
  const out: Seg[] = []; let at = 0;
  for (const m of s.matchAll(RE)) {
    if (m.index! > at) out.push({ t: "text", text: s.slice(at, m.index) });
    if (m[1] !== undefined) out.push({ t: "code", text: m[1] });
    else if (m[2] !== undefined) out.push({ t: "strong", text: m[2] });
    else if (m[3] !== undefined || m[4] !== undefined) out.push({ t: "em", text: (m[3] ?? m[4])! });
    else out.push({ t: "link", text: m[5]!, href: m[6]! });
    at = m.index! + m[0].length;
  }
  if (at < s.length) out.push({ t: "text", text: s.slice(at) });
  return out;
}
