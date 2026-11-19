import { EditorSelection, EditorState } from '@codemirror/state'
import { showBlocks, shownBlocks } from '../src/blocks/selected'
import { Vim, getCM } from '@replit/codemirror-vim'
import { afterEach, describe, expect, it } from 'vitest'
import { blockBeside, blocksIn } from '../src/blocks/blocks'
import { type NoteEditor, createEditor } from '../src/editor'
import { NoteSession, freezeNotes, resumeNotes } from '../src/note'
import { memoryMetaStore } from '../src/offline/meta'
import { type VimHost, vimMode } from '../src/vim'
import { tagAround } from '../src/vim/tags'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'
import { markdownLanguage } from '@codemirror/lang-markdown'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 2000) {
  const end = Date.now() + ms
  while (!f()) {
    if (Date.now() > end) throw new Error('timed out')
    await tick(5)
  }
}

let editors: NoteEditor[] = []
afterEach(() => {
  editors.forEach((e) => e.destroy())
  editors = []
})

const said: string[] = []
const host: VimHost = {
  selectBlocks: (view, spans) =>
    view.dispatch({ selection: EditorSelection.range(spans[0]!.from, spans.at(-1)!.to), effects: showBlocks(spans) }),
  deleteBlocks: (view) => {
    const { from, to } = view.state.selection.main
    view.dispatch({ changes: { from, to: Math.min(to + 1, view.state.doc.length) }, selection: { anchor: from } })
  },
  nest: () => {},
  commandLine: () => {},
  say: (t) => void said.push(t),
  quit: () => {},
}
const vimExt = vimMode(host)

async function open(text: string) {
  const room = new Room(text)
  const s = await NoteSession.open({
    id: `n${Math.random()}`,
    offline: false,
    meta: memoryMetaStore(),
    checkDelayMs: 5,
    transport: (lineage) => memoryTransport(() => room, lineage),
  })
  s.connect()
  await until(() => s.client.isSynced)
  const parent = document.createElement('div')
  document.body.append(parent)
  const ed = createEditor(parent, s, { extensions: vimExt })
  editors.push(ed)
  const view = ed.view
  const cm = getCM(view)!
  // A note opens in insert mode; tests start from normal mode.
  await tick()
  await tick()
  const keys = (...ks: string[]) => {
    for (const k of ks) Vim.handleKey(cm, k, 'user')
  }
  keys('<Esc>')
  return { room, s, view, cm, keys, text: () => view.state.doc.toString() }
}

describe('vim mode', () => {
  it('opens in insert mode and Esc gives normal mode', async () => {
    const { view, cm } = await open('# T\n\nwords\n')
    expect(view.dom.dataset.vim).toBe('normal')
    Vim.handleKey(cm, 'i', 'user')
    expect(view.dom.dataset.vim).toBe('insert')
  })

  it('u and Ctrl-r undo and redo through the shared history', async () => {
    const { keys, text, view, s } = await open('# T\n\none two three\n')
    view.dispatch({ selection: { anchor: text().indexOf('one') } })
    keys('d', 'w')
    expect(text()).toBe('# T\n\ntwo three\n')
    expect(s.text.toString()).toBe('# T\n\ntwo three\n')
    keys('u')
    expect(text()).toBe('# T\n\none two three\n')
    keys('<C-r>')
    expect(text()).toBe('# T\n\ntwo three\n')
  })

  it('u and Ctrl-r cannot edit a frozen note and keep their history for cancellation', async () => {
    const { keys, text, view, s } = await open('# T\n\nwords\n')
    try {
      const original = text()
      view.dispatch({ changes: { from: original.indexOf('words'), insert: 'own ' }, userEvent: 'input.type' })
      const changed = text()
      freezeNotes()
      keys('u')
      expect(text()).toBe(changed)
      expect(s.text.toString()).toBe(changed)
      resumeNotes()
      keys('u')
      expect(text()).toBe(original)
      freezeNotes()
      keys('<C-r>')
      expect(text()).toBe(original)
      resumeNotes()
      keys('<C-r>')
      expect(text()).toBe(changed)
    } finally { resumeNotes(); s.destroy() }
  })

  it('each normal-mode change is its own undo step, however fast', async () => {
    const { keys, text, view } = await open('# T\n\na b c d\n')
    view.dispatch({ selection: { anchor: text().indexOf('a') } })
    keys('x', 'x', 'x')
    expect(text()).toBe('# T\n\n c d\n')
    keys('u')
    expect(text()).toBe('# T\n\nb c d\n')
  })

  it('never lands in the hidden properties', async () => {
    const { keys, text, view } = await open('---\ncalternal-id: x\n---\n# T\n\nbody\n')
    view.dispatch({ selection: { anchor: text().indexOf('body') } })
    // The title's line; the caret stays after its hidden `# `, as everywhere in the default mode.
    const title = () => view.state.doc.lineAt(view.state.selection.main.head).text
    keys('g', 'g')
    expect(title()).toBe('# T')
    keys('k', 'k')
    expect(title()).toBe('# T')
    expect(view.state.selection.main.head).toBeGreaterThanOrEqual(text().indexOf('# T'))
  })

  it('a remote change in insert mode is never part of what `.` repeats', async () => {
    const { keys, text, view, room } = await open('# T\n\nmine\n\ntheirs\n')
    view.dispatch({ selection: { anchor: text().indexOf('mine') } })
    keys('i')
    view.dispatch({ changes: { from: view.state.selection.main.head, insert: 'X' }, selection: { anchor: view.state.selection.main.head + 1 }, userEvent: 'input.type' })
    room.doc.getText('md').insert(room.doc.getText('md').toString().indexOf('theirs'), 'REMOTE ')
    await until(() => text().includes('REMOTE'))
    keys('<Esc>', 'j', 'j', '0', '.')
    // `.` typed X again, and only X: never the other device's words.
    expect(text().split('X').length - 1).toBe(2)
    expect(text().split('REMOTE').length - 1).toBe(1)
  })

  it(':w reports Saved only after the authority has a covering receipt', async () => {
    const { cm, s } = await open('# T\n\nx\n')
    await until(() => s.client.isConfirmed)
    said.length = 0
    Vim.handleEx(cm as never, 'w')
    expect(said).toEqual(['Saved'])
    s.client.disconnect()
    s.text.insert(s.text.length, ' offline')
    said.length = 0
    Vim.handleEx(cm as never, 'w')
    expect(said[0]).toMatch(/^Not saved yet/)
    s.client.refuse('too_large')
    said.length = 0
    Vim.handleEx(cm as never, 'w')
    expect(said).toEqual(['Too large to save'])
  })
})

describe('V', () => {
  const press = (view: import('@codemirror/view').EditorView, key: string) =>
    view.contentDOM.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }))

  it('selects blocks, walks with j and k, deletes with d, and p puts them back', async () => {
    const { keys, text, view } = await open('# T\n\none\n\ntwo\n\nthree\n')
    view.dispatch({ selection: { anchor: text().indexOf('one') } })
    keys('V')
    expect(shownBlocks(view.state)?.length).toBe(1)
    press(view, 'j')
    expect(shownBlocks(view.state)?.length).toBe(2)
    press(view, 'k')
    expect(shownBlocks(view.state)?.length).toBe(1)
    press(view, 'd')
    expect(text()).toBe('# T\n\n\ntwo\n\nthree\n')
    // Gone with its block: keys are Vim's again.
    expect(shownBlocks(view.state)).toBe(null)
    expect(view.dom.dataset.vim).toBe('normal')
    press(view, 'p')
    expect(text()).toContain('one')
  })
})

describe('blocks for V', () => {
  const state = EditorState.create({ doc: '# T\n\none\n\n- a\n- b\n  - c\n\nlast\n', extensions: markdownLanguage })
  const at = (s: string) => state.doc.toString().indexOf(s)

  it('lists the blocks in a range, in order', () => {
    expect(blocksIn(state, at('one'), at('last')).map((b) => state.sliceDoc(b.from, b.to))).toEqual(['one', '- a', '- b', '- c', 'last'])
  })

  it('finds the block beside one, either way', () => {
    expect(state.sliceDoc(blockBeside(state, at('one'), 1)!.from, blockBeside(state, at('one'), 1)!.to)).toBe('- a')
    expect(blockBeside(state, at('one'), -1)).toBe(null) // the title is not a block
    const up = blockBeside(state, at('last'), -1)!
    expect(state.sliceDoc(up.from, up.to)).toBe('- c')
  })
})

describe('it and at in markdown', () => {
  it('finds the innermost tag pair around the caret', () => {
    const doc = 'some <b>bold <i>it</i> text</b> end'
    const view = { state: EditorState.create({ doc }) }
    const cm = { cm6: view } as never
    const t = tagAround(cm, { line: 0, ch: doc.indexOf('bold') })!
    expect(doc.slice(t.open.to.ch, t.close.from.ch)).toBe('bold <i>it</i> text')
    const i = tagAround(cm, { line: 0, ch: doc.indexOf('it<') })!
    expect(doc.slice(i.open.to.ch, i.close.from.ch)).toBe('it')
    expect(tagAround(cm, { line: 0, ch: 1 })).toBeUndefined()
  })
})
