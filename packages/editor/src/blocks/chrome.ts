/**
 * Block controls in the margins (ADR 0036): a drag handle on the left and a
 * small pill with → (make it a note) and ⋯ (more) on the right. By touch, where
 * nothing hovers, a swipe or a tap on the block's edge selects it (touch.ts, swipe-hold.ts).
 *
 * They are positioned absolutely in the editor's margins, outside the text,
 * so showing or hiding them can never move a single character. They show
 * instantly on hover, never on the title or on blank lines. This module
 * draws plain, unstyled buttons; the app styles them and decides what the
 * buttons do.
 */

import type { Extension } from '@codemirror/state'
import { EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import { type Block, blockAt } from './blocks'
import { pickedBlock } from './picked'
import { swipeHold } from './swipe-hold'
import { touchSelect } from './touch'

export type BlockAction = 'drag' | 'toggle' | 'note' | 'more'

export interface ChromeOptions {
  onAction?: (action: BlockAction, block: Block, anchor: HTMLElement, view: EditorView) => void
  /** Extra classes for the handle and the pill, such as the app's glass (#450). */
  surface?: string
}

function button(cls: string, label: string, action: BlockAction): HTMLButtonElement {
  const b = document.createElement('button')
  b.type = 'button'
  b.className = cls
  b.setAttribute('aria-label', label)
  b.dataset.action = action
  b.tabIndex = -1
  return b
}

class Chrome {
  private readonly handle = button('cm-block-handle', 'Drag to move', 'drag')
  private readonly pill = document.createElement('div')
  private block: Block | null = null
  private pointerY: number | null = null
  // A phone has no hover: a swipe, or a tap on the edge or bullet, selects instead (touch.ts).
  private readonly untouch: () => void

  constructor(
    private readonly view: EditorView,
    private readonly opts: ChromeOptions,
  ) {
    this.pill.className = 'cm-block-pill'
    this.pill.append(button('cm-block-note', 'Make it a note', 'note'), button('cm-block-more', 'More', 'more'))
    if (opts.surface) for (const el of [this.handle, this.pill]) el.classList.add(...opts.surface.split(/\s+/).filter(Boolean))
    for (const el of [this.handle, this.pill] as HTMLElement[]) {
      el.style.display = 'none'
      el.style.position = 'absolute'
      el.addEventListener('mousedown', this.onPress)
      view.scrollDOM.appendChild(el)
    }
    view.scrollDOM.addEventListener('mousemove', this.onMove)
    view.scrollDOM.addEventListener('mouseleave', this.onLeave)
    this.untouch = touchSelect(view, (block, toggle) => opts.onAction?.(toggle ? 'toggle' : 'drag', block, view.contentDOM, view))
  }

  update(u: ViewUpdate) {
    if (u.docChanged || u.geometryChanged || u.viewportChanged) this.refresh()
  }

  destroy() {
    this.untouch()
    this.handle.remove()
    this.pill.remove()
    this.view.scrollDOM.removeEventListener('mousemove', this.onMove)
    this.view.scrollDOM.removeEventListener('mouseleave', this.onLeave)
  }

  private onMove = (ev: MouseEvent) => {
    if (ev.target instanceof Node && (this.handle.contains(ev.target) || this.pill.contains(ev.target))) return
    this.pointerY = ev.clientY
    this.refresh()
  }

  private onLeave = (ev: MouseEvent) => {
    const to = ev.relatedTarget
    if (to instanceof Node && (this.handle.contains(to) || this.pill.contains(to))) return
    this.pointerY = null
    this.show(null, null)
  }

  private onPress = (ev: MouseEvent) => {
    const target = (ev.target as HTMLElement).closest('button')
    if (!target || !this.block) return
    ev.preventDefault()
    this.opts.onAction?.(target.dataset.action as BlockAction, this.block, target, this.view)
  }

  private refresh() {
    if (this.pointerY == null) return this.show(null, null)
    const y = this.pointerY
    // One measure: find the block under the pointer and where its first
    // line is, then place the controls in the same frame.
    this.view.requestMeasure({
      key: this,
      read: (view) => {
        const scroller = view.scrollDOM.getBoundingClientRect()
        const content = view.contentDOM.getBoundingClientRect()
        const pos = view.posAtCoords({ x: content.left + 1, y }, false)
        const block = blockAt(view.state, pos)
        if (!block) return null
        // The first line's own box: the block's top edge plus its padding, as tall as one
        // line of its text. (The block can be taller: padding, or later wrapped lines.)
        const line = view.domAtPos(block.from).node
        const el = (line instanceof Element ? line : line.parentElement)?.closest('.cm-line')
        const first = view.lineBlockAt(block.from)
        let top = first.top + view.documentTop
        let height = first.height
        if (el) {
          const cs = getComputedStyle(el)
          const size = parseFloat(cs.fontSize) || 16
          const lh = parseFloat(cs.lineHeight)
          height = Number.isFinite(lh) ? lh : size * 1.2
          top = el.getBoundingClientRect().top + (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.borderTopWidth) || 0)
        }
        return {
          block,
          top: top - scroller.top + view.scrollDOM.scrollTop,
          height,
          left: content.left - scroller.left + view.scrollDOM.scrollLeft,
          right: content.right - scroller.left + view.scrollDOM.scrollLeft,
        }
      },
      write: (m) => this.show(m?.block ?? null, m),
    })
  }

  private show(block: Block | null, m: { top: number; height: number; left: number; right: number } | null) {
    this.block = block
    this.view.dom.dataset.hoverBlock = block ? String(block.from) : ''
    if (!block || !m) {
      // Inline display wins over any app stylesheet, so hidden means hidden.
      this.handle.style.display = 'none'
      this.pill.style.display = 'none'
      return
    }
    // The handle sits just left of the text column, the pill just right.
    const h = this.handle.style
    h.top = `${m.top}px`
    h.left = `${m.left}px`
    h.transform = 'translateX(-100%)'
    h.setProperty('--line-h', `${m.height}px`)
    h.display = ''
    const p = this.pill.style
    p.top = `${m.top}px`
    p.setProperty('--line-h', `${m.height}px`)
    p.display = ''
    // In a window as narrow as a phone the column fills the width, so the pill would stick out past
    // the scroller's edge and make the page scroll sideways (#445). It sits over the line's end then.
    const room = this.view.scrollDOM.clientWidth + this.view.scrollDOM.scrollLeft - this.pill.offsetWidth - 4
    p.left = `${Math.min(m.right, room)}px`
  }
}

export function blockChrome(opts: ChromeOptions = {}): Extension {
  return [ViewPlugin.define((view) => new Chrome(view, opts)), swipeHold, pickedBlock]
}
