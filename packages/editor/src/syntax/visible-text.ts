/** A hidden source range, in JavaScript string offsets. */
export type HiddenTextRange = { from: number; to: number }

/** Removes block ID markers from text shown by the app. */
export function visibleText(text: string, hidden?: HiddenTextRange[]): string {
  let out = ''
  let copied = 0
  let cursor = 0

  while (true) {
    const caret = text.indexOf('^', cursor)
    if (caret < 0) break
    const before = caret === 0 ? '' : text[caret - 1]!
    if (before && !/\s|[\[,{]/u.test(before)) {
      cursor = caret + 1
      continue
    }

    let end = caret + 1
    while (end < text.length && /[A-Za-z0-9-]/.test(text[end]!)) end++
    if (end - caret < 3 || !tokenEnd(text, end)) {
      cursor = caret + 1
      continue
    }

    let start = caret
    const suffix = text.slice(end).split('\n', 1)[0]!
    if (/\s/u.test(before) && !suffix.trim()) {
      const lineStart = text.lastIndexOf('\n', caret - 1) + 1
      while (start > lineStart && /[ \t]/.test(text[start - 1]!)) start--
    }

    out += text.slice(copied, start)
    hidden?.push({ from: start, to: end })
    copied = end
    cursor = end
  }

  return copied ? out + text.slice(copied) : text
}

function tokenEnd(text: string, end: number): boolean {
  const next = text[end]
  return next === undefined || /\s|[\]},).;:]/u.test(next)
}
