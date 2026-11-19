/**
 * A note's title as its Markdown says it: the first level-one heading (`# Title`) that is real text, that is, not inside the YAML frontmatter at the top (a `# comment` there is YAML), not inside a fenced
 * code block (a `# comment` in a shell fence), and not a hashtag (`#tag` has no space). Line breaks may be `\n` or `\r\n`; a closing run of `#` is not part of the title. Reads lines until the first
 * heading, never the whole document twice. `fallback` is returned when the note has none.
 */
export function titleOf(markdown: string, fallback: string): string {
  const at = headingAt(markdown.split(/\r?\n/));
  return at ? at.title : fallback;
}

/** The text with its title heading rewritten to `title` (line break kept); unchanged when it has no heading. */
export function withTitle(markdown: string, title: string): string {
  const lines = markdown.split(/(\r?\n)/); // the breaks kept as their own elements, so each one comes back as it was
  const at = headingAt(lines.filter((_, n) => n % 2 === 0));
  if (!at) return markdown;
  lines[at.line * 2] = `# ${title}`;
  return lines.join("");
}

/** Where the title heading is (its line, zero-based) and what it says. */
function headingAt(lines: string[]): { line: number; title: string } | null {
  let i = 0;
  if (lines[0]?.trimEnd() === "---") {
    // Frontmatter runs to the next line that is only `---` (or `...`); without one it is not frontmatter and the lines are text.
    const end = lines.findIndex((l, n) => n > 0 && (l.trimEnd() === "---" || l.trimEnd() === "..."));
    if (end > 0) i = end + 1;
  }
  let fence: { ch: string; len: number } | null = null;
  for (; i < lines.length; i++) {
    const line = lines[i]!;
    const f = /^ {0,3}(`{3,}|~{3,})/.exec(line);
    if (f) {
      const ch = f[1]![0]!, len = f[1]!.length;
      if (!fence) fence = { ch, len };
      else if (ch === fence.ch && len >= fence.len && /^ {0,3}[`~]+\s*$/.test(line)) fence = null;
      continue;
    }
    if (fence) continue;
    const h = /^ {0,3}# +(.*?)(?: +#+)?\s*$/.exec(line);
    if (h && h[1]!.trim()) return { line: i, title: h[1]!.trim() };
  }
  return null;
}
