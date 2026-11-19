/** The editor's first screen leaves table fields, NodeViews, and styles unloaded. */
import { expect, test, vi } from 'vitest'

/** Entry points keep the table NodeView module out of the first screen. */
test('the editor entry points do not load the table NodeView module', async () => {
  vi.resetModules()
  let loaded = false
  vi.doMock('../src/tables/lazy', () => {
    loaded = true
    return { insertTable: () => {}, tablesNeed: () => {} }
  })

  try {
    await import('../src/index')
    await import('../src/insert/plugins')
    expect(loaded).toBe(false)
  } finally {
    vi.doUnmock('../src/tables/lazy')
    vi.resetModules()
  }
})

/** A plain note's default editor mode does not install the table-only compartment. */
test('a plain default-mode note has no table compartment', async () => {
  vi.resetModules()
  try {
    const { markdownLanguage } = await import('@codemirror/lang-markdown')
    const { LanguageSupport } = await import('@codemirror/language')
    const { EditorState } = await import('@codemirror/state')
    const { modeExtension } = await import('../src/syntax/mode')
    const { tableSlot } = await import('../src/tables/lazy')
    const state = EditorState.create({
      doc: '# Notes\n\nA plain paragraph.\n',
      extensions: [new LanguageSupport(markdownLanguage), modeExtension('default')],
    })

    expect(tableSlot.get(state)).toBeUndefined()
  } finally {
    vi.resetModules()
  }
})

/** A loaded table appends its compartment after the note is already open. */
test('activating a table appends its extension to the live note', async () => {
  vi.resetModules()
  let view: import('@codemirror/view').EditorView | undefined
  try {
    const { EditorState, StateField } = await import('@codemirror/state')
    const { EditorView } = await import('@codemirror/view')
    const marker = StateField.define({ create: () => true, update: (value) => value })
    vi.doMock('../src/tables/tables', () => ({ tables: [marker], insertTable: () => {} }))

    const { markdownLanguage } = await import('@codemirror/lang-markdown')
    const { LanguageSupport } = await import('@codemirror/language')
    const { modeExtension } = await import('../src/syntax/mode')
    const { activate } = await import('../src/tables/lazy')
    view = new EditorView({
      state: EditorState.create({
        doc: '# Notes\n\nA plain paragraph.\n',
        extensions: [new LanguageSupport(markdownLanguage), modeExtension('default')],
      }),
      parent: document.body,
    })

    expect(view.state.field(marker, false)).toBeUndefined()
    await activate(view)
    expect(view.state.field(marker)).toBe(true)
  } finally {
    view?.destroy()
    vi.doUnmock('../src/tables/tables')
    vi.resetModules()
  }
})
