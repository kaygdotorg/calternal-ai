/**
 * The small table action doorway stays with the editor. It can request table code for a note or insert action without
 * installing a table field in notes that do not contain a table.
 */

import type { EditorView } from '@codemirror/view'

let prepare: (() => Promise<unknown>) | null = null

/** Keeps the app's table bar and styles ready to load with the editor, without loading them during first paint. */
export function tablesNeed(load: () => Promise<unknown>): void {
  prepare = load
}

/** Loads app-owned table parts only when the editor's lazy table chunk asks for them. */
export function loadTableAssets(): Promise<unknown> | undefined {
  return prepare?.()
}

/** Loads the table action before inserting its Markdown, leaving a plain note usable if the chunk is unavailable. */
export async function insertTable(view: EditorView): Promise<void> {
  try {
    await (await import('./lazy')).insertTable(view)
  } catch {
    // A later Insert request can retry if the lazy chunk itself failed to load.
  }
}
