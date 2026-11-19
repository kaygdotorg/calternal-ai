/**
 * Code blocks, as one lazy piece: the app fetches this module only for a note with a fenced block
 * (apps/web/src/lib/lazy-code.ts). Nothing in the first-paint bundle imports it.
 */

import type { Extension } from '@codemirror/state'
import { codeBlockParts } from './blocks'
import { type CodeLangPress, codeLangMenu } from './lang-widget'

export { type CodeLangPress, fenceLine } from './lang-widget'
export { COMMON_LANGS, FENCE_WORD, LANG_NAME, MORE_LANGS, type Lang, fenceWord, langLabel, langOf } from './langs'

/** `open` is what a press on a language button does (the app's menu). */
export const codeBlocks = (open: (press: CodeLangPress) => void): Extension => [codeLangMenu.of(open), codeBlockParts]
