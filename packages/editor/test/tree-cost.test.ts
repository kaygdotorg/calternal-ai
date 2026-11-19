/**
 * Large-list correctness for the tree region and its rendered shapes.
 *
 * Every item and its trailing continuation must survive shaping. CPU cost is
 * measured by the nested-list typing harness on the perf VMs; elapsed time in
 * a shared Vitest worker also counts scheduling and GC, so it cannot gate speed
 * here (#1079).
 */
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { treeShape } from '../src/lists/shape'
import { treeRegion } from '../src/lists/tree-lines'

describe('large tree lists', () => {
  it('keeps every item and the trailing continuation in a 1,000-item list', () => {
    const markers = ['-', '- [ ]', '1.']
    const lines = Array.from({ length: 1000 }, (_, i) => `${'  '.repeat(i % 5)}${markers[i % 3]} item ${i} with some words`)
    const state = EditorState.create({ doc: `# Long list\n\n${lines.join('\n')}\n` })
    // As on a keystroke near the middle: the list around a screenful of lines.
    const region = treeRegion(state, 480, 520)
    const shapes = treeShape(region.lines.map((c) => c.line))
    // Every item is shaped. The blank line after the list is shaped too, as a continuation
    // under the last item (no rails), so count the items themselves (#266).
    expect(shapes.filter((s) => s?.item).length).toBe(1000)
    expect(shapes.at(-1)).toMatchObject({ item: false, depth: 4, branch: null })
  })
})
