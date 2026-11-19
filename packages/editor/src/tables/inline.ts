/**
 * A cell's markdown, drawn: `**bold**`, `*italic*`, `~~strike~~`, `==mark==`, `` `code` `` and links. Only while the
 * cell is not being typed in; with the caret in it the cell shows its markdown as it is in the file, so what is typed
 * is exactly what is saved. Built from DOM nodes, never from an HTML string.
 *
 * A link written as `[title](<path>)` is drawn when the cell loses focus; cell input stays inside the Grid's row patch.
 */

const TOKEN =
  /`([^`]+)`|\[([^\]]+)\]\((?:<([^>]*)>|([^)\s]*))\)|\*\*(.+?)\*\*|~~(.+?)~~|==(.+?)==|\*([^*\s](?:[^*]*[^*\s])?)\*|(?<![A-Za-z0-9])_([^_\s](?:[^_]*[^_\s])?)_(?![A-Za-z0-9])/

/** Whether the text has anything to draw (most cells are plain words). */
export const hasInline = (md: string): boolean => TOKEN.test(md)

function build(md: string, into: ParentNode): void {
  let rest = md
  for (let m = TOKEN.exec(rest); m; m = TOKEN.exec(rest)) {
    if (m.index) into.append(rest.slice(0, m.index))
    const wrap = (tag: string, text: string, cls?: string) => {
      const el = document.createElement(tag)
      if (cls) el.className = cls
      build(text, el)
      into.append(el)
      return el
    }
    if (m[1] !== undefined) {
      const code = document.createElement('code')
      code.textContent = m[1]
      into.append(code)
    } else if (m[2] !== undefined) wrap('a', m[2], 'tw-link').dataset.href = m[3] ?? m[4] ?? ''
    else if (m[5] !== undefined) wrap('strong', m[5])
    else if (m[6] !== undefined) wrap('s', m[6])
    else if (m[7] !== undefined) wrap('mark', m[7])
    else wrap('em', (m[8] ?? m[9])!)
    rest = rest.slice(m.index + m[0].length)
  }
  if (rest) into.append(rest)
}

/** Draws `md` in the cell, in place of whatever it held. */
export function paintInline(el: HTMLElement, md: string): void {
  const frag = document.createDocumentFragment()
  build(md, frag)
  el.replaceChildren(frag)
}
