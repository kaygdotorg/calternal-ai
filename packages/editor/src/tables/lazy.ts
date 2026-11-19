/**
 * The small doorway into tables. The grid, model, bar and styles load only when a visible table needs them, or a
 * table is made by typing, paste or Insert. A failed load can be tried again; the Markdown remains the document.
 */

import { Compartment, StateEffect } from '@codemirror/state'
import type { EditorView } from '@codemirror/view'
import { loadTableAssets } from './slot'

type Widget = typeof import('./tables')

/** Holds table state only after a note first draws or creates a Markdown table. */
export const tableSlot = new Compartment()

let loading: Promise<Widget> | null = null

/** Loads the editor and app pieces together, then forgets a failed request so the next trigger can try again. */
const load = (): Promise<Widget> => {
  if (loading) return loading
  loading = Promise.all([import('./tables'), loadTableAssets()])
    .then(([m]) => m)
    .catch((error: unknown) => {
      loading = null
      throw error
    })
  return loading
}

/** Turns the full table extension on after this editor has a visible Markdown table. */
export async function activate(view: EditorView): Promise<void> {
  const m = await load()
  if (!view.dom.isConnected || view.dom.dataset.mode === 'source') return
  const effect =
    tableSlot.get(view.state) === undefined
      ? StateEffect.appendConfig.of(tableSlot.of(m.tables))
      : tableSlot.reconfigure(m.tables)
  view.dispatch({ effects: effect })
  view.dom.dataset.tablesActive = 'true'
}

/** Stops running table state and key handlers when the editor no longer draws a table. */
export function deactivate(view: EditorView): void {
  if (!view.dom.isConnected) return
  if (tableSlot.get(view.state) !== undefined) view.dispatch({ effects: tableSlot.reconfigure([]) })
  delete view.dom.dataset.tablesActive
}

/** Insert and /table: loads table code and styles before writing the new Markdown table. */
export async function insertTable(view: EditorView): Promise<void> {
  try {
    const m = await load()
    if (view.dom.isConnected) m.insertTable(view)
  } catch {
    // Keep the editor usable and let a later Insert request retry the lazy load.
  }
}

/** Converts a typed pipe header, or replays Enter if the line is not a table. */
export async function enterTable(view: EditorView, replay: () => void): Promise<void> {
  try {
    const m = await load()
    if (!view.dom.isConnected || m.typedLine(view)) return
  } catch {
    // The typed Markdown stays visible; the replay leaves Enter's usual behavior intact.
  }
  if (view.dom.isConnected) replay()
}

/** Converts a spreadsheet paste, falling back to its plain text when the table path is unavailable. */
export async function pasteTable(view: EditorView, html: string, text: string): Promise<void> {
  try {
    const m = await load()
    if (!view.dom.isConnected) return
    if (m.pasteRows(view, html, text)) return
  } catch {
    // A later paste can retry after a failed stylesheet or code import.
  }
  if (view.dom.isConnected) view.dispatch(view.state.replaceSelection(text))
}
