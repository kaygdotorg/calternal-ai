/**
 * The block caret covers the letter it is on, exactly (#631).
 *
 * Vim's own block is a box holding a copy of the letter, sized by that copy. The copy forgets what
 * the page does to the letter (tabular figures in a log time, a wide emoji, a pill's padding), so
 * the box came out narrower than the glyph. Here the box takes the rect the browser painted for
 * the letter (`coordsForChar`), and wears the letter's own font settings. Where the page has no
 * rect for the spot (end of line, a tab, hidden syntax), Vim's own box stays: one space wide.
 *
 * It is measured with Vim's own measuring (once per selection change, scroll-free, or resize),
 * never per frame: Vim's `measureReq` is wrapped, its read gets one more rect, its write one more
 * set of styles.
 */

import type { EditorView } from '@codemirror/view'
import type { CM } from './commands'

type Fit = { left: number; top: number; width: number; height: number; text: string; style: CSSStyleDeclaration | null }
type Req = { read: (v: EditorView) => { cursors: unknown[] } & { fit?: Fit | null }; write: (m: { cursors: unknown[]; fit?: Fit | null }, v: EditorView) => void; fitted?: boolean }

const segmenter = typeof Intl !== 'undefined' && 'Segmenter' in Intl ? new Intl.Segmenter(undefined, { granularity: 'grapheme' }) : null

/** The letter under the block, a whole character: an emoji with its joiners, a letter with its marks. */
function clusterAt(view: EditorView, head: number): string {
  const s = view.state.sliceDoc(head, head + 32)
  if (!s) return ''
  if (segmenter) return segmenter.segment(s)[Symbol.iterator]().next().value?.segment ?? ''
  return String.fromCodePoint(s.codePointAt(0)!)
}

/** What Vim's block sits on: the main selection's head, its way (on the letter, not past it in visual mode). */
function headOf(view: EditorView): number {
  const { anchor, head } = view.state.selection.main
  const doc = view.state.doc
  let h = head
  if (anchor < head && (head >= doc.length || doc.sliceString(head, head + 1) !== '\n')) h--
  if (h > 1 && /[\uDC00-\uDFFF]/.test(doc.sliceString(h, h + 1))) h--
  return Math.max(0, h)
}

/** The rect, the text and the font settings to give the main block; null leaves Vim's own. */
function measure(view: EditorView): Fit | null {
  const head = headOf(view)
  if (head >= view.state.doc.length) return null
  const text = clusterAt(view, head)
  if (!text || text === '\n' || text === '\r' || text === '\t' || text === '\r\n') return null
  const r = view.coordsForChar(head)
  if (!r || r.right - r.left < 0.5 || r.bottom - r.top < 0.5) return null
  const at = view.domAtPos(head, 1)
  const el = at.node instanceof HTMLElement ? at.node : at.node.parentElement
  const box = view.scrollDOM.getBoundingClientRect()
  const sx = view.scaleX
  const sy = view.scaleY
  return {
    left: (r.left - (box.left - view.scrollDOM.scrollLeft * sx)) / sx,
    top: (r.top - (box.top - view.scrollDOM.scrollTop * sy)) / sy,
    width: (r.right - r.left) / sx,
    height: (r.bottom - r.top) / sy,
    text,
    style: el ? getComputedStyle(el) : null,
  }
}

/** Sets the block's box and letter to the fit. */
function apply(layer: Element, f: Fit) {
  const el = layer.querySelector<HTMLElement>('.cm-cursor-primary')
  if (!el) return
  const s = el.style
  s.left = `${f.left}px`
  s.top = `${f.top}px`
  s.width = `${f.width}px`
  s.height = `${f.height}px`
  s.lineHeight = `${f.height}px`
  s.overflow = 'hidden'
  s.boxSizing = 'border-box'
  if (el.textContent !== f.text) el.textContent = f.text
  if (f.style) {
    s.fontFamily = f.style.fontFamily
    s.fontSize = f.style.fontSize
    s.fontWeight = f.style.fontWeight
    s.fontStyle = f.style.fontStyle
    s.fontVariantNumeric = f.style.fontVariantNumeric
    s.fontFeatureSettings = f.style.fontFeatureSettings
    s.fontKerning = f.style.fontKerning
    s.letterSpacing = f.style.letterSpacing
  }
}

/** Vim rewrites its position and font, but leaves our extra glyph styles on a reused box. */
function reset(layer: Element) {
  const el = layer.querySelector<HTMLElement>('.cm-cursor-primary')
  if (!el) return
  for (const name of ['width', 'overflow', 'box-sizing', 'font-style', 'font-variant-numeric', 'font-feature-settings', 'font-kerning', 'letter-spacing']) el.style.removeProperty(name)
}

/** Wraps Vim's block measuring so it fits the letter. Safe to call again; wraps once. */
export function fitBlockCursor(cm: CM, view: EditorView) {
  const bc = (cm.state.vimPlugin as { blockCursor?: { measureReq?: Req; cursorLayer?: Element } } | undefined)?.blockCursor
  const req = bc?.measureReq
  if (!bc || !req || req.fitted) return
  req.fitted = true
  const read = req.read
  const write = req.write
  req.read = (v) => {
    const out = read(v)
    out.fit = measure(v)
    return out
  }
  req.write = (m, v) => {
    if (!m.fit && bc.cursorLayer) reset(bc.cursorLayer)
    write(m, v)
    if (m.fit && bc.cursorLayer) apply(bc.cursorLayer, m.fit)
  }
  view.requestMeasure(req as never)
}
