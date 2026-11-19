/**
 * The caret says the mode (#538, VM4): a block over the letter in normal mode, the usual bar in
 * insert mode, and a block outline in visual mode. In the accent, never a gradient. It blinks as
 * the editor's caret does, and not at all with reduced motion. Search matches wear the accent too.
 *
 * Vim draws the block itself (its `cm-fat-cursor`, the letter under it in the text's own font and
 * colour); insert mode is the editor's own caret (`.cm-cursor`, the app's 2 px bar).
 */

import { EditorView } from '@codemirror/view'

const accent = 'var(--accent, #b45309)'

export const vimCaret = EditorView.theme({
  '& .cm-vimCursorLayer .cm-fat-cursor': {
    background: `color-mix(in oklab, ${accent} 42%, transparent)`,
    borderRadius: '2px',
    outline: 'none',
  },
  '&[data-vim="visual"] .cm-vimCursorLayer .cm-fat-cursor': {
    background: 'none',
    boxShadow: `inset 0 0 0 1.5px ${accent}`,
  },
  '&:not(.cm-focused) .cm-vimCursorLayer .cm-fat-cursor': {
    background: 'none',
    boxShadow: `inset 0 0 0 1px color-mix(in oklab, ${accent} 60%, transparent)`,
    outline: 'none',
    color: 'transparent !important',
  },
  '&.cm-editor .cm-content .cm-searchMatch': {
    background: `color-mix(in oklab, ${accent} 26%, transparent)`,
    borderRadius: '2px',
  },
  '@media (prefers-reduced-motion: reduce)': {
    '& .cm-vimCursorLayer': { animation: 'none !important' },
  },
})
