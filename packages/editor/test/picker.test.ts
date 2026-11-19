import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { afterEach, describe, expect, it } from 'vitest'
import { findActions } from '../src/insert/find'
import { insertBlockAction } from '../src/insert/insert'
import { registerDefaultBlocks } from '../src/insert/plugins'
import { type BlockAction, blockActions, blockById, onBlockActions, registerBlock } from '../src/insert/registry'
import { type PickerShown, closePicker, inlinePicker, pickKey, pickerReady } from '../src/transforms/picker'

registerDefaultBlocks()

/** Lets the (async) note search answer. */
const settle = () => new Promise((r) => setTimeout(r, 0))

const labels = (q: string) => findActions(blockActions(), q).flatMap((g) => g.items.map((a) => a.label))

describe('the block registry (#471)', () => {
  it('lists the blocks by group, in the registry order, with nothing typed', () => {
    const groups = findActions(blockActions(), '').map((g) => g.group)
    expect(groups).toEqual(['Basic', 'Lists', 'Media', 'Links'])
    expect(labels('')[0]).toBe('Heading 1')
  })

  it('finds by label, by other words and by letters in order', () => {
    expect(labels('head')).toEqual(['Heading 1', 'Heading 2', 'Heading 3'])
    expect(labels('todo')[0]).toBe('Task')
    expect(labels('bllt')).toEqual(['Bullet list'])
    expect(labels('zzzz')).toEqual([])
  })

  it('shows another area’s block in the search as soon as it registers, and takes it away again', () => {
    let calls = 0
    const stop = onBlockActions(() => calls++)
    const table: BlockAction = { id: 'table', label: 'Table', group: 'Basic', icon: 'table', keywords: ['grid'], markdown: '| A |\n| --- |\n|  |', caret: 2 }
    const undo = registerBlock(table)
    expect(labels('tbl')).toEqual(['Table'])
    expect(blockById('table')).toBe(table)
    undo()
    expect(blockById('table')).toBeUndefined()
    expect(calls).toBe(2)
    stop()
  })

  it('replaces an action registered again under the same id', () => {
    const before = blockActions().length
    registerBlock({ ...blockById('quote')!, label: 'Quotation' })
    expect(blockActions().length).toBe(before)
    expect(blockById('quote')!.label).toBe('Quotation')
    registerBlock({ ...blockById('quote')!, label: 'Quote' })
  })
})

const language = new LanguageSupport(markdownLanguage)
const views: EditorView[] = []
afterEach(() => views.splice(0).forEach((v) => v.destroy()))

async function make(doc: string, extra: { links?: Parameters<typeof inlinePicker>[0]; tags?: Parameters<typeof inlinePicker>[1] } = {}) {
  const shown: (PickerShown | null)[] = []
  const host = { show: (p: PickerShown) => shown.push(p), hide: () => shown.push(null) }
  const view = new EditorView({
    parent: document.body,
    state: EditorState.create({ doc, selection: { anchor: doc.length }, extensions: [language, inlinePicker(extra.links, extra.tags ?? { counts: () => new Map() }, host)] }),
  })
  views.push(view)
  const type = (text: string, userEvent = 'input.type') => {
    const at = view.state.selection.main.head
    view.dispatch({ changes: { from: at, insert: text }, selection: { anchor: at + text.length }, userEvent })
  }
  const now = () => [...shown].reverse().find((s) => s !== undefined) ?? null
  // The picker's code comes in on idle (or at the first trigger): the tests start once it is in.
  await pickerReady(view)
  return { view, type, shown, now, doc: () => view.state.doc.toString() }
}

describe('insert (#471)', () => {
  it('offers Image and File from the start, and they ask the app for a chooser instead of writing markdown', async () => {
    expect(labels('')).toEqual(expect.arrayContaining(['Image', 'File']))
    const { view, doc } = await make('# T\n\n')
    const asked: string[] = []
    view.dom.addEventListener('calternal:attach', (e) => asked.push((e as CustomEvent<string>).detail))
    insertBlockAction(view, blockById('image')!)
    insertBlockAction(view, blockById('file')!)
    expect(asked).toEqual(['photos', 'file'])
    expect(doc()).toBe('# T\n\n')
  })

  it('writes into the empty line the caret is on, caret after the marker', async () => {
    const { view, doc } = await make('# T\n\n')
    insertBlockAction(view, blockById('heading-1')!)
    expect(doc()).toBe('# T\n\n## ')
    expect(view.state.selection.main.head).toBe(doc().length)
  })

  it('writes on a new line after a line that has words, and puts the caret where the action says', async () => {
    const { view, doc } = await make('# T\n\nwords')
    insertBlockAction(view, blockById('code')!)
    expect(doc()).toBe('# T\n\nwords\n```\n\n```')
    expect(view.state.selection.main.head).toBe('# T\n\nwords\n```\n'.length)
  })

  it('gives a divider a line to write on after it', async () => {
    const { view, doc } = await make('# T\n\n')
    insertBlockAction(view, blockById('divider')!)
    expect(doc()).toBe('# T\n\n---\n')
  })

  it('writes the time of a log line as the daily note does', async () => {
    const { view, doc } = await make('# T\n\n')
    insertBlockAction(view, blockById('time')!)
    expect(doc()).toMatch(/^# T\n\n- \d\d:\d\d $/)
  })
})

describe('the inline picker (#165, #449, #471)', () => {
  it('opens the block menu for a slash at the start of a line, and narrows as you type', async () => {
    const t = await make('# T\n\n')
    t.type('/')
    expect(t.now()!.kind).toBe('slash')
    expect(t.now()!.rows.length).toBeGreaterThan(8)
    t.type('hea')
    expect(t.now()!.rows.map((r) => r.label)).toEqual(['Heading 1', 'Heading 2', 'Heading 3'])
  })

  it('opens after a space, but never in the middle of a word or a path', async () => {
    const a = await make('# T\n\nso /')
    a.type('b')
    expect(a.now()?.kind).toBe('slash')
    for (const text of ['and/or', 'see /usr/bin', 'http://x']) {
      const t = await make('# T\n\n')
      t.type(text)
      expect(t.view.state.doc.toString().endsWith(text)).toBe(true)
      expect(t.shown.length, text).toBe(0)
    }
  })

  it('takes a row with Enter: the typed words go and the block goes where the caret’s line is', async () => {
    const t = await make('# T\n\n')
    t.type('/quo')
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\n> ')
    expect(t.shown.at(-1)).toBeNull()
  })

  it('leaves the words before the slash alone and puts the block on the next line', async () => {
    const t = await make('# T\n\nbuy milk ')
    t.type('/task')
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\nbuy milk \n- [ ] ')
  })

  it('opens the tag list after # and a letter, never for "#12" or a lone #', async () => {
    const counts = new Map([['alpha', 3], ['area', 2], ['area/work', 2]])
    const t = await make('# T\n\nissue ', { tags: { counts: () => counts } })
    t.type('#')
    expect(t.now()).toBeNull()
    t.type('12')
    expect(t.now()).toBeNull()
    t.type(' #a')
    expect(t.now()!.kind).toBe('tag')
    expect(t.now()!.rows.map((r) => r.label).sort()).toEqual(['alpha', 'area', 'area/work'])
    expect(t.now()!.rows[0]).toMatchObject({ label: 'alpha', count: 3 })
  })

  it('does not look at the title, code, links or the properties', async () => {
    for (const doc of ['# Title ', '# T\n\n```\n']) {
      const t = await make(doc)
      t.type('/')
      expect(t.shown.length, JSON.stringify(doc)).toBe(0)
    }
  })

  it('still opens when the editor was rebuilt (the note’s text replaced the preview) while the picker’s code was on its way', async () => {
    const shown: (PickerShown | null)[] = []
    const host = { show: (p: PickerShown) => shown.push(p), hide: () => shown.push(null) }
    const ext = () => [language, inlinePicker(undefined, { counts: () => new Map() }, host)]
    const view = new EditorView({ parent: document.body, state: EditorState.create({ doc: '', extensions: ext() }) })
    views.push(view)
    const early = pickerReady(view)
    view.setState(EditorState.create({ doc: '# T\n\n', selection: { anchor: 5 }, extensions: ext() }))
    await early
    view.dispatch({ changes: { from: 5, insert: '/' }, selection: { anchor: 6 }, userEvent: 'input.type' })
    await pickerReady(view)
    expect([...shown].reverse().find((s) => s !== undefined)?.kind).toBe('slash')
  })

  it('opens only for the person’s own typing: a paste, an undo and another device never open it', async () => {
    const t = await make('# T\n\n')
    t.type('/he', 'input.paste')
    expect(t.shown.length).toBe(0)
    t.type('x', 'undo')
    expect(t.shown.length).toBe(0)
  })

  it('closes on Escape and when the caret leaves what it completes', async () => {
    const t = await make('# T\n\n')
    t.type('/he')
    expect(t.now()).not.toBeNull()
    t.view.dispatch({ selection: { anchor: 1 } })
    expect(t.shown.at(-1)).toBeNull()
    t.view.dispatch({ selection: { anchor: t.view.state.doc.length } })
    t.type(' /he')
    expect(t.shown.at(-1)).not.toBeNull()
    expect(closePicker(t.view)).toBe(true)
    expect(t.shown.at(-1)).toBeNull()
    expect(closePicker(t.view)).toBe(false)
  })

  it('a Link action types its trigger and the same picker opens', async () => {
    const found = [{ title: 'Trip', path: 'Notes/Trip.md', id: 'n1' }]
    const t = await make('# T\n\n', { links: { notePath: () => 'Notes/T.md', search: () => found } })
    t.type('/link')
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\n[[')
    await settle()
    expect(t.now()!.kind).toBe('note')
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\n[Trip](<Trip.md>)')
  })

  it('a Tag action opens the tags with nothing typed after the #', async () => {
    const t = await make('# T\n\n', { tags: { counts: () => new Map([['alpha', 3]]) } })
    t.type('/tag')
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\n#')
    expect(t.now()!.kind).toBe('tag')
  })

  it('a link written by the picker is a standard markdown link, never [[ ]]', async () => {
    const t = await make('# T\n\nsee ', { links: { notePath: () => 'Notes/T.md', search: () => [{ title: 'Trip ideas', path: 'Notes/Trip ideas.md', id: 'n1' }] } })
    t.type('[[tr')
    await settle()
    t.now()!.accept(0)
    expect(t.doc()).toBe('# T\n\nsee [Trip ideas](<Trip ideas.md>)')
  })
})

describe('the keys every list shares', () => {
  it('moves round the ends, takes with Enter and Tab, closes with Esc', () => {
    expect(pickKey({ key: 'ArrowDown' }, 2, 3)).toEqual({ active: 0 })
    expect(pickKey({ key: 'ArrowUp' }, 0, 3)).toEqual({ active: 2 })
    expect(pickKey({ key: 'Enter' }, 1, 3)).toEqual({ take: 'enter' })
    expect(pickKey({ key: 'Tab' }, 1, 3)).toEqual({ take: 'tab' })
    expect(pickKey({ key: 'Tab', shiftKey: true }, 1, 3)).toBeNull()
    expect(pickKey({ key: 'Escape' }, 1, 3)).toEqual({ close: true })
    expect(pickKey({ key: 'Escape' }, 0, 0)).toEqual({ close: true })
    expect(pickKey({ key: 'a' }, 0, 3)).toBeNull()
    expect(pickKey({ key: 'ArrowDown', metaKey: true }, 0, 3)).toBeNull()
  })
})
