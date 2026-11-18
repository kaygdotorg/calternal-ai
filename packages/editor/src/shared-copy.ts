/**
 * What the end of a share needs from the editor: your unconfirmed edits to a shared note (open or closed),
 * and removing every local copy of it.
 *
 * Why its own file: these run only when a share ends or a peer tab forgets a note, so they load with the
 * sharing code after the first screen (#876, first paint). They read the open editors through `sessions`.
 */
import * as Y from 'yjs'
import { lingering, sessions, TEXT_KEY } from './note'
import { idbMetaStore } from './offline/meta'
import { forgetCopy, readClosedCopy } from './offline/closed-copy'

export type PendingSharedEdits = { base: string; local: string; delta: string }

/**
 * The lines you added (and quoted lines you removed) between `base` and `local`. Runs only when a share
 * ends, so the diff library is fetched then, never with the first screen.
 */
async function changedText(base: string, local: string): Promise<string> {
  if (base === local) return ''
  const { default: DiffMatchPatch } = await import('diff-match-patch')
  const dmp = new DiffMatchPatch()
  const lines = dmp.diff_linesToChars_(base, local)
  const diffs = dmp.diff_main(lines.chars1, lines.chars2, false)
  dmp.diff_charsToLines_(diffs, lines.lineArray)
  const added = diffs.filter(([op]) => op === 1).map(([, text]) => text).join('')
  const removed = diffs.filter(([op]) => op === -1).map(([, text]) => text).join('')
  const parts: string[] = []
  if (added) parts.push(added)
  if (removed) {
    const quote = removed.split('\n').map((line) => `> ${line}`).join('\n')
    parts.push(`Removed from the shared note:\n${quote}`)
  }
  return parts.join('\n\n')
}

/** Reads this Installation's unconfirmed edits, including a closed copy and its WAL. */
export async function pendingSharedEdits(id: string): Promise<PendingSharedEdits | null> {
  const open = [...sessions].find((session) => session.id === id)
  let base: string | null
  let local: string
  if (open) {
    const snapshot = open.pendingLocalSnapshot()
    if (!snapshot) return null
    base = snapshot.base
    local = snapshot.local
  } else {
    if (typeof indexedDB === 'undefined') return null
    const meta = await idbMetaStore().get(id)
    const doc = new Y.Doc()
    try {
      const dirty = await readClosedCopy(`calternal-note-${id}`, doc)
      if (dirty == null) return null
      base = meta.base
      local = doc.getText(TEXT_KEY).toString()
    } finally {
      doc.destroy()
    }
  }
  const delta = await changedText(base ?? '', local)
  return delta ? { base: base ?? '', local, delta } : null
}

/** Closes every open editor for one note and removes all of its local copies. */
export async function forgetNote(id: string, announce = true): Promise<void> {
  const name = `calternal-note-${id}`
  if (announce) {
    try { localStorage.setItem(`${name}:forget`, String(Date.now())) } catch { /* IndexedDB still enforces the deletion. */ }
  }
  for (const session of [...sessions]) {
    if (session.id === id) await session.forgetLocalCopy()
  }
  lingering.get(id)?.()
  await forgetCopy(name)
  await idbMetaStore().remove(id)
  try { localStorage.removeItem(`${name}:refused`) } catch { /* The metadata and editor database are already gone. */ }
}

/** Retires pending shared writes so a later read starts from the server's current copy. */
export function retireSharedCopy(id: string): Promise<void> {
  return forgetNote(id)
}
