/**
 * What makes a block editor live: the room's shared text (`Y.Text md`) bound to the view, other people's cursors drawn from presence, this person's own undo, and the input-method gate. Loaded only when a note opens
 * (the host imports it lazily), so a note read without a room, and the first load, carry none of the Yjs code.
 *
 * Undo is author-only (`Y.UndoManager` tracking only this editor's own origin): another person's typing is never undone by this person's undo, from the keyboard or from the Edit menu and shake. Changes that arrive from the
 * room are marked `remote` (block/origin.ts), so the editor's automatic transforms (a link, a list continuation) never run on them. A change from the room waits behind the session's IME gate until a composition ends
 * (`NoteSession.gate`, which `SyncClient` honours), so a phone keyboard's composition is never disturbed.
 */
import { EditorState, Prec, type Extension } from '@codemirror/state'
import { EditorView, ViewPlugin, keymap, type KeyBinding } from '@codemirror/view'
import { yCollab, yUndoManagerKeymap as shared, ySyncAnnotation, ySyncFacet } from 'y-codemirror.next'
import type { Awareness } from 'y-protocols/awareness'
import * as Y from 'yjs'
import { origin } from '../block/origin'
import type { NoteSession } from '../note'
import { LocalIntent } from './intent'

/** How another person is drawn: a display name and a colour (from their account, resolved by the browser). */
export interface Person { name: string; color: string; colorLight: string }

/** Presence as the cursor plugin reads it: every state, with `user` resolved from the account the room stamped on it. */
class Named {
  constructor(private readonly inner: Awareness, private readonly resolve: (account: string) => Person) {}
  get doc(): Y.Doc { return this.inner.doc }
  on(name: 'change', f: (...a: never[]) => void): void { this.inner.on(name, f) }
  off(name: 'change', f: (...a: never[]) => void): void { this.inner.off(name, f) }
  getLocalState() { return this.inner.getLocalState() }
  setLocalStateField(field: string, value: unknown): void { this.inner.setLocalStateField(field, value) }
  getStates(): Map<number, Record<string, unknown>> {
    const out = new Map<number, Record<string, unknown>>()
    for (const [id, state] of this.inner.getStates()) {
      const account = typeof state.account_id === 'string' ? state.account_id : ''
      out.set(id, { ...state, user: account ? this.resolve(account) : { name: 'Someone', color: '#888', colorLight: '#8884' } })
    }
    return out
  }
}

/** Undo and redo for the keyboard, through Yjs, refused while the note cannot be edited. */
const undoKeys = (session: NoteSession): KeyBinding[] => shared.map((b) => ({ ...b, run: (view: EditorView) => (view.state.readOnly || session.readOnly ? true : (b.run?.(view) ?? false)) }))

/** The Edit menu's and the phone's undo and redo (`historyUndo`, `historyRedo`) go through the same guarded Yjs commands. */
const historyInput = (keys: KeyBinding[]): Extension => Prec.highest(EditorView.domEventHandlers({
  beforeinput(event, view) {
    const key = event.inputType === 'historyUndo' ? 'Mod-z' : event.inputType === 'historyRedo' ? 'Mod-y' : null
    if (!key) return false
    event.preventDefault()
    keys.find((b) => b.key === key)?.run?.(view)
    return true
  },
}))

/** Opens and closes the session's input-method gate around compositions: the composed text lands first, then the room's changes. */
const composition = (session: NoteSession): Extension => EditorView.domEventHandlers({
  compositionstart() { session.gate.start(); return false },
  compositionend(_e, view) {
    const release = () => (view.composing ? requestAnimationFrame(release) : session.gate.end())
    requestAnimationFrame(release)
    return false
  },
})

/** The event the header's presence avatars send to show every name until the next press elsewhere. */
export const SHOW_LABELS = 'calternal-live-labels'
/** How long a person's name stays beside their cursor after it moves (a touch screen has no hover). */
const LABEL_MS = 2000

/**
 * Names beside other people's cursors on a screen with no hover: a name shows for two seconds whenever somebody's cursor or selection moves, then fades (the theme's transition uses the motion tokens, so under reduced motion it
 * simply goes). Pressing the presence avatars shows every name until the next press anywhere else. A pointer still shows a name on hover (the cursor plugin's own rule). The state is one attribute on the editor, `data-labels`
 * (`move` or `all`), so it costs nothing per keystroke: it listens to presence changes only.
 */
const labels = (presence: Awareness | null): Extension => ViewPlugin.fromClass(class {
  private seen = new Map<number, string>()
  private timer = 0
  private all = false
  private readonly onChange = ({ added, updated }: { added: number[]; updated: number[] }) => {
    const a = presence!
    let moved = false
    for (const id of [...added, ...updated]) {
      if (id === a.doc.clientID) continue
      const now = JSON.stringify(a.getStates().get(id)?.cursor ?? null)
      if (this.seen.get(id) !== now) { this.seen.set(id, now); if (now !== 'null') moved = true }
    }
    if (moved) this.flash()
  }
  private readonly onShowAll = () => { this.all = true; this.view.dom.dataset.labels = 'all' }
  private readonly onPress = (e: Event) => {
    if (!this.all || (e.target as Element | null)?.closest?.('[data-presence]')) return
    this.all = false
    delete this.view.dom.dataset.labels
  }
  constructor(private readonly view: EditorView) {
    presence?.on('change', this.onChange)
    addEventListener(SHOW_LABELS, this.onShowAll)
    addEventListener('pointerdown', this.onPress, true)
  }
  private flash() {
    if (this.all) return
    this.view.dom.dataset.labels = 'move'
    clearTimeout(this.timer)
    this.timer = window.setTimeout(() => { if (!this.all) delete this.view.dom.dataset.labels }, LABEL_MS)
  }
  destroy() {
    presence?.off('change', this.onChange)
    removeEventListener(SHOW_LABELS, this.onShowAll)
    removeEventListener('pointerdown', this.onPress, true)
    clearTimeout(this.timer)
  }
})

/** The look of another person's cursor, in the app's own type and radii (the plugin's defaults are serif and black). */
const theme = EditorView.baseTheme({
  '.cm-ySelectionCaret': { borderLeftWidth: '2px', borderRightWidth: '0', marginRight: '0' },
  '&[data-labels] .cm-ySelectionInfo': { opacity: '1', transitionDelay: '0s' },
  '.cm-ySelectionInfo': { transition: 'opacity var(--dur-base) var(--ease-out)', fontFamily: 'inherit', fontSize: '11px', fontWeight: '600', lineHeight: '1.3', padding: '1px 6px', borderRadius: '6px 6px 6px 0', top: '-1.5em', left: '-2px', whiteSpace: 'nowrap' },
})

/**
 * The extension for a live note: pass it as the block editor's `extensions` and create the editor with `session.text.toString()`. `resolve` turns the account the room stamped on a cursor into a name and colour;
 * `awareness` is `session.awareness` (null shows no cursors). The returned `undo` is this editor's own undo manager.
 */
export function liveExtension(session: NoteSession, resolve: (account: string) => Person): { extension: Extension; undo: Y.UndoManager; intent: LocalIntent } {
  const ytext = session.text
  const undo = new Y.UndoManager(ytext, { trackedOrigins: new Set() })
  // What this editor itself wrote (typing and paste go through the view's sync configuration, undo and redo through the undo manager): the only text a rescue may take.
  const intent = new LocalIntent(ytext, () => false)
  const own = ViewPlugin.define((view) => { const conf = view.state.facet(ySyncFacet); intent.mine = (o) => o === conf || o === undo; return { destroy: () => intent.destroy() } })
  const keys = undoKeys(session)
  const presence = session.awareness ? new Named(session.awareness, resolve) : null
  return {
    undo,
    intent,
    extension: [
      own,
      historyInput(keys),
      Prec.highest(keymap.of(keys)),
      // The editor's own history is not used here: Yjs owns undo, for this author only.
      yCollab(ytext, presence as unknown as Awareness | null, { undoManager: undo }),
      EditorState.transactionExtender.of((tr) => (tr.annotation(ySyncAnnotation) != null ? { annotations: origin.of('remote') } : null)),
      // Once the room has ended for lack of access, nothing more is typed into the note (what was written is kept by the host).
      EditorState.transactionFilter.of((tr) => (session.readOnly && tr.docChanged && tr.annotation(ySyncAnnotation) == null ? [] : tr)),
      composition(session),
      labels(session.awareness),
      theme,
    ],
  }
}
