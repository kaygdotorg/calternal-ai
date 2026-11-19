/**
 * The note editor: CodeMirror bound to a note's shared text.
 *
 * - The shared text is the document (ADR 0018). The editor is created from
 *   it, and when the session's document is replaced the editor is rebuilt
 *   from the new one, never the other way round, so they can't drift.
 * - Undo only undoes this person's own changes (ADR 0022).
 * - Changes from elsewhere wait while an input method is composing.
 * - One editor shows note after note (#150). A note can first be shown as
 *   a preview: a remembered copy of its text, read-only and bound to no
 *   document, so it appears the moment it is picked. Its live document is
 *   attached once the person settles on it, and only that document is
 *   ever edited.
 */

import { markdownKeymap } from '@codemirror/lang-markdown'
import { noteMarkdown } from './tables/parser'
import { LanguageSupport } from '@codemirror/language'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import { drawSelection, EditorView, keymap } from '@codemirror/view'
import { defaultKeymap } from '@codemirror/commands'
import { yCollab } from 'y-codemirror.next'
import * as Y from 'yjs'
import { type ChromeOptions, blockChrome } from './blocks/chrome'
import { selectedBlocks } from './blocks/selected'
import type { NoteSession } from './note'
import { type EditorMode, modeExtension, setMode } from './syntax/mode'
import { tableEnterHandler } from './syntax/hidden'
import { parseAhead } from './syntax/parse-ahead'
import { iosKeys } from './input/ios-keys'
import { continueList } from './lists/enter'
import type { LinkOptions } from './transforms/links'
import { type PickerHost, inlinePicker } from './transforms/picker'
import type { TagOptions } from './transforms/tags'
import { taskToggle } from './transforms/tasks'
import { editPermission, historyInput, noteUndo, yUndoManagerKeymap } from './undo'
import { isRemote } from './origins'
import { editorSaveStatus } from './sync/status'
import { frontmatterEnd } from './syntax/frontmatter'
import { visibleText } from './syntax/visible-text'

export interface EditorOptions {
  mode?: EditorMode
  links?: LinkOptions
  /** Tags the `#` picker offers (#449). */
  tags?: TagOptions
  /** The picker list's stylesheet, loaded late: the picker's first lookup waits for it. */
  pickerStyles?: () => Promise<unknown>
  /** Where the inline picker (`/`, `[[`, `@`, `#`) is drawn; without it the editor offers none. */
  picker?: PickerHost
  chrome?: ChromeOptions
  /**
   * Extra extensions from the app (theme, keymaps). A function is asked
   * again for every note shown, for extensions that depend on the note.
   */
  extensions?: Extension | (() => Extension)
  /** Label read by screen readers for the text area. */
  label?: string
  /** Synchronous device permission, checked before local edits and history commands. */
  canEdit?: () => boolean
}

export interface NoteEditor {
  readonly view: EditorView
  /** The live note the editor is bound to; null while it shows a preview. */
  readonly session: NoteSession | null
  mode: EditorMode
  setMode(mode: EditorMode): void
  /**
   * Shows `text` read-only, from the top: a remembered copy of a note, not
   * bound to any document. Typing into it is refused, never kept aside.
   */
  preview(text: string): void
  /**
   * Binds the editor to a note's live document, rebuilt from it. When the
   * same text is already showing, the caret and scroll stay where they
   * are, so nothing moves.
   */
  attach(session: NoteSession): void
  /** Rebuilds the editor from the session's current document. */
  reattach(): void
  /** True when the editor shows exactly the shared text (it always should). */
  inSync(): boolean
  destroy(): void
}

/** A preview takes no change at all: not typed, pasted, dropped or dispatched. */
const readOnly: Extension = [
  EditorState.readOnly.of(true),
  EditorState.transactionFilter.of((tr) => (tr.docChanged ? [] : tr)),
]

/** The app's extensions, swapped for each note a preview shows. */
const perNote = new Compartment()
/** Read-only for a preview, or the live document's binding: swapped in place when a preview goes live. */
const binding = new Compartment()
const editing = new Compartment()
const editable = (session: NoteSession | null): Extension => [
  EditorState.readOnly.of(!session || session.readOnly),
  EditorView.editable.of(!!session && !session.readOnly),
]
const bindingFor = (session: NoteSession | null, opts: EditorOptions): Extension =>
  // Block actions need the live note; they sit in the margins, so adding them moves nothing.
  session ? [live(session, opts), blockChrome(opts.chrome), editorSaveStatus.of(() => session.status.get())] : readOnly
const appExtensions = (opts: EditorOptions): Extension =>
  typeof opts.extensions === 'function' ? opts.extensions() : (opts.extensions ?? [])

function extensions(session: NoteSession | null, opts: EditorOptions, mode: EditorMode): Extension {
  const extra = appExtensions(opts)
  return [
    // The bare markdown language, not `markdown()`: that one bundles the
    // HTML, CSS and JavaScript languages for HTML blocks (~50 KB).
    new LanguageSupport(noteMarkdown),
    binding.of(bindingFor(session, opts)),
    editing.of(editable(session)),
    drawSelection(),
    // Drawn lines are parsed before their frame, the rest of the note at idle (plan 1c).
    parseAhead,
    selectedBlocks,
    EditorView.lineWrapping,
    modeExtension(mode),
    inlinePicker(opts.links, opts.tags, opts.picker),
    taskToggle,
    EditorView.domEventHandlers({
      copy(event, view) {
        return copyVisibleText(event, view, false)
      },
      cut(event, view) {
        return copyVisibleText(event, view, true)
      },
    }),
    EditorView.contentAttributes.of({ 'aria-label': opts.label ?? 'Note', 'aria-multiline': 'true' }),
    perNote.of(extra),
  ]
}

function copyVisibleText(event: ClipboardEvent, view: EditorView, cut: boolean): boolean {
  const clipboard = event.clipboardData
  const selection = view.state.selection.main
  if (selection.empty) return false
  event.preventDefault()
  if (!clipboard) return true

  const selected = view.state.sliceDoc(selection.from, selection.to)
  const hidden: { from: number; to: number }[] = []
  const text = visibleText(selected, hidden)
  clipboard.clearData()
  clipboard.setData('text/plain', text)

  if (cut && !view.state.readOnly) {
    view.dispatch({
      changes: { from: selection.from, to: selection.to },
      userEvent: 'input.cut',
    })
  }
  return true
}

/** What makes the editor live: the shared text, this person's undo, the IME gate. */
function live(session: NoteSession, opts: EditorOptions): Extension {
  const ytext = session.text
  // Track only this editor's own changes: never another device's or tab's.
  const undoManager = new Y.UndoManager(ytext, { trackedOrigins: new Set() })
  const allowed = () => !session.readOnly && (opts.canEdit?.() ?? true)
  return [
    editPermission.of(allowed),
    historyInput,
    EditorState.transactionFilter.of((tr) =>
      !allowed() && tr.docChanged && !isRemote(tr) ? [] : tr),
    tableEnterHandler,
    // Return in a list never adds a blank line (#225); then CodeMirror's own.
    keymap.of([...yUndoManagerKeymap, { key: 'Enter', run: continueList }, ...markdownKeymap, ...defaultKeymap]),
    yCollab(ytext, null, { undoManager }),
    noteUndo.of(undoManager),
    composition(session),
    // One tap on Return is one line break, even when Safari's own edit comes late (#582).
    iosKeys,
  ]
}

/** Opens and closes the IME gate around compositions. */
function composition(session: NoteSession): Extension {
  return EditorView.domEventHandlers({
    compositionstart() {
      session.gate.start()
      return false
    },
    compositionend(_ev, view) {
      // Let CodeMirror apply the composed text first, then let remote
      // changes in on top of it.
      const release = () => (view.composing ? requestAnimationFrame(release) : session.gate.end())
      requestAnimationFrame(release)
      return false
    },
  })
}

export function createEditor(parent: HTMLElement, first: NoteSession | null, opts: EditorOptions = {}): NoteEditor {
  let mode: EditorMode = opts.mode ?? 'default'
  let session: NoteSession | null = null
  let offReplaced: (() => void) | null = null
  let offReadOnly: (() => void) | null = null
  const make = (text: string, keep?: EditorState) => {
    const state = EditorState.create({ doc: text, extensions: extensions(session, opts, mode) })
    // A new state starts with its caret at 0, inside the hidden properties: move it to the body.
    if (!keep) return frontmatterEnd(state) ? state.update({ selection: state.selection }).state : state
    // The caret stays where it was, clamped to the text.
    const sel = keep.selection.main
    const at = (n: number) => Math.min(n, text.length)
    return state.update({ selection: { anchor: at(sel.anchor), head: at(sel.head) } }).state
  }
  const bind = (s: NoteSession | null) => {
    offReplaced?.()
    offReadOnly?.()
    session = s
    offReplaced = s ? s.onReplaced(() => editor.reattach()) : null
    offReadOnly = s ? s.onReadOnly(() => view.dispatch({ effects: editing.reconfigure(editable(s)) })) : null
  }
  bind(first)
  const view = new EditorView({ state: make(first?.text.toString() ?? ''), parent })
  const mark = () => {
    view.dom.dataset.mode = mode
    view.dom.dataset.preview = session ? 'false' : 'true'
  }
  mark()
  view.dom.dataset.ready = 'true'

  const editor: NoteEditor = {
    view,
    get session() {
      return session
    },
    get mode() {
      return mode
    },
    setMode(next) {
      mode = next
      setMode(view, next)
    },
    // Nothing here touches layout after the new state: scrolling or dispatching right after
    // `setState` forced a full layout of the new note, on every note shown. The scroller keeps
    // its position across `setState` by itself.
    preview(text) {
      if (view.scrollDOM.scrollTop) view.scrollDOM.scrollTop = 0
      if (session) {
        bind(null)
        view.setState(make(text))
      } else {
        // Preview to preview (holding ↓): swap the text in place. A new state would re-measure
        // the text and rebuild every plugin, for every note passed. `filter: false` lets this
        // one change through the preview's own refusal of changes.
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: text },
          selection: { anchor: 0 },
          effects: perNote.reconfigure(appExtensions(opts)),
          filter: false,
        })
      }
      mark()
    },
    attach(s) {
      const text = s.text.toString()
      const same = text === view.state.doc.toString()
      bind(s)
      if (same) {
        // The preview already shows this text: only its binding changes. A new state would parse
        // and lay out the whole note again (#327: the longest task of a cold start, and of every
        // note going live after a walk); the caret and scroll stay where they are anyway.
        // The app's extensions are asked again too: what they depend on (the note's own entry in
        // the list, for a day's heading) may only have arrived since the preview.
        view.dispatch({ effects: [binding.reconfigure(bindingFor(s, opts)), editing.reconfigure(editable(s)), perNote.reconfigure(appExtensions(opts))] })
      } else {
        view.setState(make(text))
      }
      mark()
    },
    reattach() {
      view.setState(make(session ? session.text.toString() : view.state.doc.toString()))
      mark()
    },
    inSync() {
      return !!session && view.state.doc.toString() === session.text.toString()
    },
    destroy() {
      offReplaced?.()
      offReadOnly?.()
      view.destroy()
    },
  }
  return editor
}
