/** Horizontal table fitting should run only when the table scroller gains or loses width. */
import { expect, test, vi } from 'vitest'
import * as grid from '../src/tables/grid'

type Resize = { observed: Element[]; callback: ResizeObserverCallback }
const observers: Resize[] = []

/** Captures the observer callback so the test can send width and height changes independently. */
class ResizeObserverStub {
  readonly observed: Element[] = []

  constructor(readonly callback: ResizeObserverCallback) {
    observers.push({ observed: this.observed, callback })
  }

  observe(target: Element): void {
    this.observed.push(target)
  }

  disconnect(): void {}
}

/** Builds a resize entry for the selected target without requiring a browser layout engine. */
function send(observer: Resize, target: Element, width: number, height: number): void {
  observer.callback([{ target, contentRect: { width, height } } as ResizeObserverEntry], {} as ResizeObserver)
}

test('does not refit when a table box changes height or a non-scroller width changes', () => {
  const watch = (grid as unknown as { watchScrollerWidth?: (scroller: Element, onWidthChange: () => void) => ResizeObserver | null }).watchScrollerWidth
  expect(watch, 'the grid must expose its scroller-width observer for this focused check').toBeDefined()
  if (!watch) return

  observers.length = 0
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
  const scroller = document.createElement('div')
  const table = document.createElement('table')
  const fit = vi.fn()
  const resize = watch(scroller, fit)

  try {
    expect(resize).not.toBeNull()
    const observer = observers[0]!
    expect(observer.observed).toEqual([scroller])
    send(observer, scroller, 640, 240)
    expect(fit).toHaveBeenCalledTimes(1)

    fit.mockClear()
    send(observer, scroller, 640, 260)
    send(observer, table, 641, 260)
    expect(fit).not.toHaveBeenCalled()

    send(observer, scroller, 641, 260)
    expect(fit).toHaveBeenCalledTimes(1)
  } finally {
    resize?.disconnect()
    vi.unstubAllGlobals()
  }
})
