/**
 * How input is grouped for undo, decided once for every text surface. Typing and deleting join into one step while they keep coming and stay adjacent; a paste, a drop, a spelling or
 * autocorrect replacement and a yank are each a step of their own, and a drop that moves text (a delete by drag, then the insert) is ONE step. The block editor gets the same rule from
 * CodeMirror's own history (its joinable events are exactly typing and deleting); a text field applies it here.
 */

/** True for the input types that are a step by themselves. */
export const isolatedInput = (inputType: string): boolean => /^(insertFromPaste|insertFromPasteAsQuotation|insertFromDrop|insertReplacementText|insertFromYank|insertFromWebSearch)$/.test(inputType);
