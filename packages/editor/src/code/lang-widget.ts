/**
 * The language button in a code block's top right corner (the approved mockup's CodeLang). It sits in
 * the place of the opening fence, which is never drawn, and says "Plain text", "Rust" and so on. A press
 * asks the app to open its language menu (the `codeLang` option); the menu writes the word after the
 * fence. It holds no menu itself, so no grammar or menu code loads with the editor.
 */

import { Facet } from '@codemirror/state'
import { type EditorView, WidgetType } from '@codemirror/view'
import { langLabel } from './langs'

export interface CodeLangPress {
  /** The editor the button is in. */
  view: EditorView

  /** The button, for placing the menu under it. */
  anchor: HTMLElement
  /** Where the opening fence line starts. */
  pos: number
  /** The word after the fence now ('' for none). */
  word: string
}

/** What a press does; the app gives it. */
export const codeLangMenu = Facet.define<(press: CodeLangPress) => void, ((press: CodeLangPress) => void) | null>({
  combine: (v) => v[0] ?? null,
})

/**
 * Where the fence line starts, asked of the page at the moment of the press. A widget keeps no position of
 * its own: the file can change under it (the server rewrites the properties after a save), and a stored
 * position would then point at another line.
 */
export function fenceLine(view: EditorView, button: HTMLElement): number {
  // The button sits inside the fence's line: that line's own start, never the button's position (which, for a
  // widget placed before a hidden range, may read as the end of the line above).
  const row = button.closest('.cm-line')
  return row ? view.state.doc.lineAt(view.posAtDOM(row, 0)).from : 0
}

const SVG = 'http://www.w3.org/2000/svg'

export class CodeLangWidget extends WidgetType {
  constructor(readonly word: string) {
    super()
  }
  override eq(other: CodeLangWidget): boolean {
    return other.word === this.word
  }
  toDOM(view: EditorView): HTMLElement {
    const b = document.createElement('button')
    b.type = 'button'
    b.className = 'cm-code-lang bounce'
    b.tabIndex = -1
    b.contentEditable = 'false'
    b.setAttribute('aria-haspopup', 'listbox')
    b.setAttribute('aria-expanded', 'false')
    const label = langLabel(this.word)
    b.setAttribute('aria-label', `Language: ${label}`)
    b.append(label)
    const svg = document.createElementNS(SVG, 'svg')
    svg.setAttribute('viewBox', '0 0 24 24')
    svg.setAttribute('aria-hidden', 'true')
    const path = document.createElementNS(SVG, 'path')
    path.setAttribute('d', 'M9 6l6 6-6 6')
    svg.append(path)
    b.append(svg)
    // The text keeps its caret and focus: a press must not take them.
    b.addEventListener('mousedown', (e) => e.preventDefault())
    b.addEventListener('click', () => {
      const open = view.state.facet(codeLangMenu)
      if (open) open({ view, anchor: b, pos: fenceLine(view, b), word: this.word })
    })
    return b
  }
  override ignoreEvent(): boolean {
    return true
  }
}
