import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { iosKeys } from '../src/input/ios-keys'

// The order of events on an iPhone, as captured in the simulator with its software keyboard (#582).
//
//   normal  keydown Enter, beforeinput insertLineBreak, input, [CodeMirror's] keydown Enter
//   late    keydown Enter, [CodeMirror's] keydown Enter, beforeinput insertLineBreak, input
//
// "[CodeMirror's]" is the key CodeMirror makes up (marked `synthetic`) to run the key's command. In the late order
// Safari's own edit comes after that command ran, and would be a second line break.

let view: EditorView | null = null
afterEach(() => view?.destroy())

/** An editor where a made-up key press is "handled" (as a bound command does), or not. */
function editor(handled: boolean) {
  view = new EditorView({
    state: EditorState.create({
      extensions: [
        iosKeys,
        EditorView.domEventHandlers({ keydown: (ev) => handled && (ev as KeyboardEvent & { synthetic?: boolean }).synthetic === true }),
      ],
    }),
    parent: document.body,
  })
  return view.contentDOM
}

const key = (dom: HTMLElement, k: string, synthetic = false) => {
  const ev = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true })
  if (synthetic) (ev as KeyboardEvent & { synthetic?: boolean }).synthetic = true
  dom.dispatchEvent(ev)
}

/** Safari's own edit; true when something cancelled it. */
const edit = (dom: HTMLElement, inputType: string) => {
  const ev = new InputEvent('beforeinput', { inputType, bubbles: true, cancelable: true })
  dom.dispatchEvent(ev)
  return ev.defaultPrevented
}

const tick = () => new Promise<void>((r) => queueMicrotask(r))

describe('Safari’s own edit after a key’s command ran (#582)', () => {
  afterEach(() => vi.restoreAllMocks())

  it('is cancelled when it comes after the command (late order)', async () => {
    const dom = editor(true)
    key(dom, 'Enter')
    key(dom, 'Enter', true)
    await tick()
    expect(edit(dom, 'insertLineBreak')).toBe(true)
  })

  it('is left alone when it comes first (normal order)', async () => {
    const dom = editor(true)
    key(dom, 'Enter')
    expect(edit(dom, 'insertLineBreak')).toBe(false)
    key(dom, 'Enter', true)
    await tick()
    // Nothing is left to cancel for this press.
    expect(edit(dom, 'insertLineBreak')).toBe(false)
  })

  it('is left alone when the key has no command (Shift+Return)', async () => {
    const dom = editor(false)
    key(dom, 'Enter')
    key(dom, 'Enter', true)
    await tick()
    expect(edit(dom, 'insertLineBreak')).toBe(false)
  })

  it('is cancelled once: the next press has its own', async () => {
    const dom = editor(true)
    key(dom, 'Enter')
    key(dom, 'Enter', true)
    await tick()
    expect(edit(dom, 'insertParagraph')).toBe(true)
    expect(edit(dom, 'insertParagraph')).toBe(false)
    // A new press, quick this time.
    key(dom, 'Enter')
    expect(edit(dom, 'insertLineBreak')).toBe(false)
  })

  it('covers Backspace and Delete the same way', async () => {
    const dom = editor(true)
    key(dom, 'Backspace')
    key(dom, 'Backspace', true)
    await tick()
    expect(edit(dom, 'deleteContentBackward')).toBe(true)
    key(dom, 'Delete')
    key(dom, 'Delete', true)
    await tick()
    expect(edit(dom, 'deleteContentForward')).toBe(true)
  })

  it('only cancels the edit that key makes', async () => {
    const dom = editor(true)
    key(dom, 'Enter')
    key(dom, 'Enter', true)
    await tick()
    expect(edit(dom, 'insertText')).toBe(false)
    expect(edit(dom, 'deleteContentBackward')).toBe(false)
    expect(edit(dom, 'insertLineBreak')).toBe(true)
  })

  it('does not cancel an edit that comes long after (it is not that press’s)', async () => {
    const dom = editor(true)
    const now = Date.now()
    key(dom, 'Enter')
    key(dom, 'Enter', true)
    await tick()
    vi.spyOn(Date, 'now').mockReturnValue(now + 5000)
    expect(edit(dom, 'insertLineBreak')).toBe(false)
  })

  it('ignores a made-up key with no key press before it', async () => {
    const dom = editor(true)
    key(dom, 'Enter', true)
    await tick()
    expect(edit(dom, 'insertLineBreak')).toBe(false)
  })
})
