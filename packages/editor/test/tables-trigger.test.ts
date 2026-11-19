/** Table code is requested only for a visible table or a table-making action. */
import { markdownKeymap, markdownLanguage } from '@codemirror/lang-markdown'
import { defaultKeymap } from '@codemirror/commands'
import { LanguageSupport } from '@codemirror/language'
import { EditorState, Transaction, type Extension } from '@codemirror/state'
import { Decoration, EditorView, keymap } from '@codemirror/view'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { continueList } from '../src/lists/enter'
import * as lazy from '../src/tables/lazy'
import { hiddenSyntax, tableEnterHandler } from '../src/syntax/hidden'

const requested = vi.hoisted(() => ({ activate: vi.fn(), deactivate: vi.fn(), enterTable: vi.fn(), pasteTable: vi.fn() }))
vi.mock('../src/tables/lazy', () => requested)

const activate = (lazy as unknown as { activate: typeof requested.activate }).activate
const deactivate = (lazy as unknown as { deactivate: typeof requested.deactivate }).deactivate
const enterTable = (lazy as unknown as { enterTable: typeof requested.enterTable }).enterTable
const pasteTable = (lazy as unknown as { pasteTable: typeof requested.pasteTable }).pasteTable

/** Mounts the real hidden-syntax view plugin with the editor's Markdown grammar. */
function open(doc: string, beforeHidden: Extension[] = []): EditorView {
  return new EditorView({
    state: EditorState.create({ doc, extensions: [new LanguageSupport(markdownLanguage), ...beforeHidden, hiddenSyntax] }),
    parent: document.body,
  })
}

beforeEach(() => {
  requested.activate.mockReset()
  requested.deactivate.mockReset()
  requested.enterTable.mockReset()
  requested.pasteTable.mockReset()
})

describe('lazy table activation', () => {
  /** Plain notes should not request the NodeView or its table-specific styles. */
  test('does not request table code for a note without a table', async () => {
    const view = open('# Notes\n\nA paragraph with no table.\n')
    await Promise.resolve()

    expect(activate).not.toHaveBeenCalled()
    view.destroy()
  })

  /** An existing top-level Markdown table should request its grid as soon as it is drawn. */
  test('requests table code when a note opens with a table', async () => {
    const view = open('# Notes\n\n| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n')
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(1))

    view.destroy()
  })

  /** A plain note that receives a complete table starts the same lazy load as an existing table. */
  test('requests table code when a table is added to an open note', async () => {
    const view = open('# Notes\n\nPlain text.\n')
    const insert = '\n| Item | Qty |\n| --- | --- |\n| Oats | 2 |'
    view.dispatch({ changes: { from: view.state.doc.length, insert } })
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(1))

    view.destroy()
  })

  /** A Grid cell edit maps already-drawn syntax marks instead of rebuilding every pending row. */
  test('maps existing hidden decorations on a table cell edit', async () => {
    const view = open('# Notes\n\n| A | B |\n| --- | --- |\n| old | value |\n\n[After](<Notes/After.md>)\n')
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(1))
    const buildDecorations = vi.spyOn(Decoration, 'set')
    const at = view.state.doc.toString().indexOf('| old |')
    const row = view.state.doc.lineAt(at)

    try {
      view.dispatch({
        changes: { from: row.from, to: row.to, insert: '| a longer cell | value |' },
        annotations: Transaction.userEvent.of('input.table.cell'),
      })

      expect(buildDecorations.mock.calls.flatMap(([ranges]) => ranges)).toHaveLength(0)
      expect(view.dom.querySelector('[data-href="Notes/After.md"]')?.textContent).toBe('After')
      expect(view.dom.querySelectorAll('.cm-line.cm-table-pending')).toHaveLength(3)
    } finally {
      buildDecorations.mockRestore()
      view.destroy()
    }
  })

  /** The already-loaded table state should leave a plain note without table update work. */
  test('stops table code after the last visible table is removed', async () => {
    const view = open('# Notes\n\n| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n')
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(1))
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: '# Notes\n\nPlain text.\n' } })
    await vi.waitFor(() => expect(deactivate).toHaveBeenCalledTimes(1))

    view.destroy()
  })

  /** Enter must wait for the lazy NodeView code before converting a pipe header. */
  test('requests table code for a typed pipe header on Enter', async () => {
    const view = open('# Notes\n\n| Item | Qty |', [tableEnterHandler, keymap.of([])])
    view.dispatch({ selection: { anchor: view.state.doc.length } })
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    view.contentDOM.dispatchEvent(event)
    await vi.waitFor(() => expect(enterTable).toHaveBeenCalledTimes(1))

    expect(event.defaultPrevented).toBe(true)
    view.destroy()
  })

  /** The app's earlier Enter commands must not consume the typed table header before lazy loading sees it. */
  test('claims a typed pipe header before the app Enter keymap', async () => {
    const appEnterKeymap = keymap.of([{ key: 'Enter', run: continueList }, ...markdownKeymap, ...defaultKeymap])
    const view = open('# Notes\n\n| Item | Qty |', [tableEnterHandler, appEnterKeymap])
    view.dispatch({ selection: { anchor: view.state.doc.length } })
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    view.contentDOM.dispatchEvent(event)
    await vi.waitFor(() => expect(enterTable).toHaveBeenCalledTimes(1))

    expect(event.defaultPrevented).toBe(true)
    view.destroy()
  })

  /** A spreadsheet paste is held until its parser and styles have loaded. */
  test('requests table code for spreadsheet paste', async () => {
    const view = open('# Notes\n\n')
    const event = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent
    Object.defineProperty(event, 'clipboardData', {
      value: { getData: (kind: string) => (kind === 'text/html' ? '<table><tr><td>A</td></tr></table>' : 'A\tB\n1\t2') },
    })
    view.contentDOM.dispatchEvent(event)
    await vi.waitFor(() => expect(pasteTable).toHaveBeenCalledTimes(1))

    expect(event.defaultPrevented).toBe(true)
    view.destroy()
  })

  /** A later view update may retry a failed lazy load without losing the Markdown table. */
  test('retries table activation after a failed request', async () => {
    requested.activate.mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(undefined)
    const view = open('# Notes\n\n| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n')
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(1))

    view.dispatch({ selection: { anchor: 0 } })
    await vi.waitFor(() => expect(activate).toHaveBeenCalledTimes(2))

    view.destroy()
  })
})
