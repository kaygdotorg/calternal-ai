/**
 * Hidden blocks, as one lazy piece: the app fetches this module only for a note that has hidden blocks
 * or when a block is hidden (apps/web/src/lib/lazy-hidden.ts). Nothing in the first-paint bundle imports it.
 */

export { blocksIn } from './blocks/blocks'
export { hiddenBlockList, hiddenBlocks, hiddenExtension, hiddenInstalled, hiddenPeek, setPeek, type HiddenOptions, type Peek } from './blocks/hidden-blocks'
export { hiddenEdits, hiddenRefs } from './blocks/hidden-refs'
