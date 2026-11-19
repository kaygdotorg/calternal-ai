/**
 * What a table grid shares with the note around it: its raw rows (the revision), the hooks it asks of the note, and
 * the small helpers that compare or watch them. Split out of `grid.ts` so the grid file holds only the grid itself;
 * `grid.ts` re-exports everything here, so callers import from either.
 */

import { Annotation } from '@codemirror/state'
import type { Widths } from './widths'

/** What the grid asks of the note around it: which table it is, and where its dragged widths are kept. */
export interface Hooks {
  widths: Widths | null
  /** This table's place among the note's tables, and how many there are (for `table-widths`). */
  where(pos: number): { index: number; count: number }
  save(index: number, count: number, w: Widths | null): void
  /** Where this table's text starts in the note now, from the note's own table record; -1 if unknown. */
  start?(revision: GridRevision): number
}

/** The cell widget's identity and raw rows, shared with the table decoration until outside text replaces it. */
export interface GridRevision {
  /** The last Grid transaction version requested for the note. */
  version: number
  /** The last version the note accepted. */
  appliedVersion: number
  /** Raw table lines, including the header and delimiter; changed lines are prepared here before dispatch. */
  lines: string[]
  /** The raw lines currently present in the note, used to reject a stale cell update. */
  appliedLines: string[]
  /** The table's current Markdown length, updated with each row patch. */
  textLength: number
}

/** Marks a small Grid cell edit so the table field can map its decoration without a note scan. */
export const tableCellEdit = Annotation.define<{ revision: GridRevision; version: number; from: number }>()

/** Makes the raw-row identity a table widget and its Grid share. */
export function createGridRevision(text: string): GridRevision {
  const lines = text.split('\n')
  return { version: 0, appliedVersion: 0, lines, appliedLines: lines.slice(), textLength: text.length }
}

/** Watches only the scroller's width; a height-only or table-content change needs no horizontal fit. */
export function watchScrollerWidth(scroller: Element, onWidthChange: () => void): ResizeObserver | null {
  if (typeof ResizeObserver !== 'function') return null
  let width: number | null = null
  const observer = new ResizeObserver((entries) => {
    const entry = entries.find((item) => item.target === scroller)
    if (!entry || entry.contentRect.width === width) return
    width = entry.contentRect.width
    onWidthChange()
  })
  observer.observe(scroller)
  return observer
}

/** Compares raw row snapshots without joining or allocating another table string. */
export function sameLines(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((line, i) => line === b[i])
}
