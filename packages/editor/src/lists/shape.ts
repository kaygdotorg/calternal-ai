/**
 * Bullet-journal tree lines for nested lists (#133): the shape only.
 *
 * Give it the lines of a list, top to bottom, and it says what to draw on
 * each one: how deep it is, which guides pass through it, and whether it
 * branches off its parent with `├` (more siblings follow) or `└` (the last
 * child). Siblings of the same kind of block (bullets, numbers or to-dos) at any level are joined by a guide through their markers. Nothing here touches the text; the markdown stays as typed.
 *
 * It knows nothing about CodeMirror or the DOM, so the real editor and the
 * mockup draw from the same shapes. The drawing is plain CSS (see
 * `packages/ui/src/styles/tree.css`): each line gets a few classes and an
 * inline style holding CSS custom properties, so the look can change
 * without touching this file.
 */

/** What a list item starts with. */
export type Marker = 'bullet' | 'task' | 'number'

/**
 * One line, as the caller sees it:
 * - `item`: starts a list item, indented by `indent` columns.
 * - `more`: belongs to the item above (a blank line, or text indented under it).
 * - `break`: not part of a list; the next item starts a new list.
 */
export type ListLine = { kind: 'item'; indent: number; marker: Marker } | { kind: 'more' } | { kind: 'break' }

export interface TreeLine {
  /** How deep the item that owns this line is (0 = top level). */
  depth: number
  /** True on the line that starts the item. */
  item: boolean
  /** On a nested item's own line: `tee` is `├`, `end` is `└`. */
  branch: 'tee' | 'end' | null
  /** Classes for the line: `tree`, plus `tree-item`, `tree-tee` or `tree-end`. */
  cls: string
  /** Inline style: the custom properties the CSS draws from. */
  style: string
}

interface Item {
  indent: number
  marker: Marker
  depth: number
  parent: Item | null
  /** No later sibling: the guide stops here. */
  last: boolean
  /** The sibling guide stops here: no later sibling, or the next one is another kind of block. */
  runEnd: boolean
  /** The sibling just above is the same kind of block: a guide arrives from it, ending short of the marker. */
  prev: boolean
  /** Has at least one child: a short stem runs down from its marker. */
  kids: boolean
  lastKid: Item | null
  /** Memo of `shapeKey` for the path down to this item, once `last` is final. */
  key?: string
}

/** Shapes for `lines`; `null` for lines outside any list. */
export function treeShape(lines: readonly ListLine[]): (TreeLine | null)[] {
  const owners: (Item | null)[] = []
  const starts: boolean[] = []
  let stack: Item[] = []
  let prevRoot: Item | null = null

  for (const line of lines) {
    if (line.kind === 'break') {
      stack = []
      prevRoot = null
      owners.push(null)
      starts.push(false)
      continue
    }
    if (line.kind === 'more') {
      owners.push(stack.at(-1) ?? null)
      starts.push(false)
      continue
    }
    // An item: everything at its indent or deeper is finished.
    while (stack.length && stack.at(-1)!.indent >= line.indent) stack.pop()
    const parent = stack.at(-1) ?? null
    const item: Item = {
      indent: line.indent,
      marker: line.marker,
      depth: stack.length,
      parent,
      last: true,
      runEnd: true,
      prev: false,
      kids: false,
      lastKid: null,
    }
    const before = parent ? parent.lastKid : prevRoot
    if (before) {
      // Only siblings of the same kind of block are joined; a change of kind starts a new guide.
      before.runEnd = before.marker !== item.marker
      item.prev = !before.runEnd
    }
    if (parent) {
      if (parent.lastKid) parent.lastKid.last = false
      parent.lastKid = item
      parent.kids = true
    } else {
      if (prevRoot) prevRoot.last = false
      prevRoot = item
    }
    stack.push(item)
    owners.push(item)
    starts.push(true)
  }

  const out: (TreeLine | null)[] = []
  const cache = new Map<string, TreeLine>()
  const path: Item[] = []
  for (let i = 0; i < owners.length; i++) {
    const owner = owners[i]
    if (!owner) {
      out.push(null)
      continue
    }
    const key = (starts[i] ? 's' : 'm') + pathKey(owner)
    let shape = cache.get(key)
    if (!shape) {
      path.length = 0
      for (let n: Item | null = owner; n; n = n.parent) path.push(n)
      path.reverse()
      cache.set(key, (shape = draw(path, starts[i]!)))
    }
    out.push(shape)
  }
  return out
}

/**
 * Lines with the same key look the same; most lines in a list share a few
 * keys. It spells out the path from the top: each item's marker and whether
 * it's the last child, then whether the owner has children.
 */
function pathKey(item: Item): string {
  return prefix(item) + (item.kids ? 'k' : '') + (item.prev ? 'p' : '')
}
function prefix(item: Item): string {
  return (item.key ??= `${item.parent ? prefix(item.parent) : ''}${item.marker[0]}${item.last ? 1 : 0}${item.runEnd ? 1 : 0}`)
}

// Where things sit, as CSS: every column is `--tree-step` wide; inside it a
// marker kind has its own centre (`--tree-x-*`), half-height (`--tree-r-*`)
// and left edge for the branch to stop short of (`--tree-e-*`). `col` is where a guide's left
// edge goes so that the guide, `--tree-w` wide, is centred on the marker.
const col = (j: number, m: Marker) => `calc(var(--tree-step) * ${j} + var(--tree-x-${m}) - var(--tree-w) / 2)`
// A guide's colour is `--tree-c<column>` (tree.css): the quiet ink, or the accent while the branch
// it belongs to is lit. Columns past 11 share the last one.
// A guide fades in where it leaves a marker and out where it arrives at the next one, over the
// last few pixels, so it never butts against a dot. It's in the gradient itself: no extra layers.
const FADE = '6px'
const rail = (j: number, x: string, top = '0px', height = '100%', fade: 'in' | 'out' | '' = '') => {
  const c = `var(--tree-c${Math.min(j, 11)})`
  const img = fade === 'in' ? `linear-gradient(#0000, ${c} min(${FADE}, 100%))` : fade === 'out' ? `linear-gradient(${c} calc(100% - ${FADE}), #0000)` : `linear-gradient(${c} 0 0)`
  return `${img} ${x} ${top} / var(--tree-w) ${height} no-repeat`
}
// The dotted look cuts those guides into dots with a mask: one column of dots per guide, a dot
// every seventh of a line height, so the rhythm runs on from line to line (tree.css).
const dots = (x: string) => `radial-gradient(circle at 50% calc(1lh / 14), #000 0.85px, #0000 1.1px) ${x} 0 / var(--tree-w) calc(1lh / 7) repeat-y`

function draw(path: readonly Item[], start: boolean): TreeLine {
  const owner = path.at(-1)!
  const d = owner.depth
  const rails: string[] = []
  const masks: string[] = []
  // An ancestor's guide passes through this line while it still has somewhere to go: the child on
  // this line's path has a later sibling, or the ancestor itself does (its guide runs on, past
  // its children, to the next sibling at its own level). On the item's own line, the parent's
  // column is also where the branch leaves.
  for (let j = 0; j < d; j++) {
    if (path[j]!.runEnd && path[j + 1]!.last) continue
    rails.push(rail(j, col(j, path[j]!.marker)))
    masks.push(dots(col(j, path[j]!.marker)))
  }
  let branch: TreeLine['branch'] = null
  if (start && d > 0) branch = owner.last ? 'end' : 'tee'
  if (owner.kids || !owner.runEnd) {
    // The stem: from just under this item's marker down to its children and on to its next
    // sibling (or, on a wrapped line, all the way).
    const top = start ? `calc(var(--tree-mid) + var(--tree-r-${owner.marker}) + var(--tree-gap))` : '0px'
    rails.push(rail(d, col(d, owner.marker), top, '100%', start ? 'in' : ''))
    masks.push(dots(col(d, owner.marker)))
  }
  if (start && owner.prev) {
    // The guide from the sibling above arrives and stops short of the marker, as far as the stem
    // starts below it.
    const x = col(d, owner.marker)
    rails.push(rail(d, x, '0px', `calc(var(--tree-mid) - var(--tree-r-${owner.marker}) - var(--tree-gap))`, 'out'))
    masks.push(dots(x))
  }
  let style = `--tree-depth:${d};--tree-rails:${rails.length ? rails.join(',') : 'none'};--tree-dots:${masks.length ? masks.join(',') : 'none'}`
  if (branch) {
    const parent = path[d - 1]!
    style += `;--tree-px:${col(d - 1, parent.marker)};--tree-ex:calc(var(--tree-step) * ${d} + var(--tree-e-${owner.marker}))`
  }
  const cls = `tree${start ? ' tree-item' : ''}${branch ? ` tree-${branch}` : ''}`
  return { depth: d, item: start, branch, cls, style }
}
