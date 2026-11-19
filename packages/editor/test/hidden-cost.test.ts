import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport, ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { buildHidden } from '../src/syntax/hidden'

type Usage = { user: number; system: number }
/** Node's `process`, without Node's types in a browser package. */
const proc = (globalThis as unknown as { process: { cpuUsage(since?: Usage): Usage } }).process

// Rebuilding decorations runs on every keystroke for the drawn range, so it
// has to stay far below a frame. This is a coarse guard, not the perf gate.
describe('hidden syntax cost', () => {
  it('rebuilds a drawn range of a long note well under a millisecond or two', () => {
    const body = Array.from({ length: 200 }, (_, i) => `Paragraph ${i} with **bold** and [a link](<N${i}.md>).\n\n- [ ] task ${i}`).join('\n\n')
    const state = EditorState.create({ doc: `# Big\n\n${body}\n`, extensions: new LanguageSupport(markdownLanguage) })
    const tree = ensureSyntaxTree(state, state.doc.length, 5000)
    const range = [{ from: 0, to: 6000 }]
    for (let i = 0; i < 20; i++) buildHidden(state, range, tree)
    // CPU time, not the wall clock: on a busy machine (CI beside the agents' builds) the wall clock
    // also counts the time this process waited for a core, which says nothing about the code.
    const c0 = proc.cpuUsage()
    const n = 200
    for (let i = 0; i < n; i++) buildHidden(state, range, tree)
    const used = proc.cpuUsage(c0)
    const per = (used.user + used.system) / 1000 / n
    console.log(`buildHidden over ${range[0]!.to} chars: ${per.toFixed(3)} ms`)
    expect(per).toBeLessThan(4)
  })
})
