/**
 * The inline picker, the part every page carries: what the app's glass list is told (the types), the effect that asks for a list, and
 * the watcher that fetches the picker's code. A trigger (`/`, `[[`, `@`, `#`) typed in the note, or the browser being idle after the editor
 * shows, brings in picker-core.ts: the plugin that decides when the list is open, the rows it holds and its keys. Until then nothing of
 * it is here, so the first screen pays for a few lines, not for a picker nobody has asked for.
 *
 * On the typing path this does one check per typed change: did the words just typed hold a trigger character? Pastes, undo and other
 * devices' text never count. A hand-typed `[[Title]]` becomes a link with or without a list (links.ts), so that handler stays here.
 */

import { Compartment, type Extension, StateEffect } from '@codemirror/state'
import { type EditorView, ViewPlugin } from '@codemirror/view'
import type { PickKind } from '../insert/registry'
import { type LinkOptions, closeHandler } from './links'
import type { PickRow } from './rows'
import type { TagOptions } from './tags'

export type { PickRow, RowKind } from './rows'

/** What the app draws: one list under the caret. */
export interface PickerShown {
  /** `slash` is the block menu; the rest are what `[[`, `@`, `#` offer. */
  kind: 'slash' | 'note' | 'day' | 'tag'
  query: string
  rows: PickRow[]
  active: number
  /** Where the trigger starts in the note: the list is placed again only when this (or the page) moves. */
  anchor: number
  /** The trigger's place in the window (its bottom, and its top: the list may open above it). Null when it is not on screen. */
  at: () => { left: number; top: number; bottom: number } | null
  /** The editor the list belongs to (it closes when that goes away). */
  dom: HTMLElement
  /** Take row `i` as Enter does, or as Tab does. */
  accept: (i: number, tab?: boolean) => void
  setActive: (i: number) => void
  close: () => void
}

/** The app's list: shows what the editor says, or nothing. */
export interface PickerHost {
  show: (p: PickerShown) => void
  hide: () => void
}

/**
 * The one key model of every picker list (the one under the caret, and the search fields of Move to,
 * Link to and "+" tag): ↑ ↓ move round the ends; Enter takes the row; Tab takes it too (a tag with tags
 * inside it opens it instead); Esc closes. Returns what the key means, or null when it is not the list's.
 */
export function pickKey(e: { key: string; shiftKey?: boolean; metaKey?: boolean; ctrlKey?: boolean; altKey?: boolean }, active: number, n: number): { active?: number; take?: 'enter' | 'tab'; close?: true } | null {
  if (n < 1 || e.metaKey || e.ctrlKey || e.altKey) return e.key === 'Escape' ? { close: true } : null
  if (e.key === 'ArrowDown') return { active: (active + 1) % n }
  if (e.key === 'ArrowUp') return { active: (active + n - 1) % n }
  if (e.key === 'Enter') return { take: 'enter' }
  if (e.key === 'Tab' && !e.shiftKey) return { take: 'tab' }
  if (e.key === 'Escape') return { close: true }
  return null
}

/** Opens the picker for a kind with nothing typed after its trigger (the Insert sheet and the `/` menu ask this). */
export const askPicker = StateEffect.define<PickKind>()

type Core = typeof import('./picker-core')
let core: Core | undefined
/** Fetches the picker's code (once). A failed fetch is asked for again the next time a trigger is typed. */
const fetchCore = () => import('./picker-core').then((m) => (core = m))
// One entry per installed picker: keyed by its compartment, not by the editor, because a note's text replacing the preview rebuilds every plugin
// of the editor (a new compartment, a new watcher) while an earlier watcher's load can still be on its way and must not count for the new one.
const installed = new WeakSet<Compartment>()
/** The characters that start a list: `/`, `[[`, `@`, `#`. */
const TRIGGER = /[/@#[]/

/** What puts the picker's code into one editor (made with the editor's plugin): tests wait for it, nothing else needs to. */
const loaders = new WeakMap<EditorView, (open: boolean, asked?: PickKind) => Promise<void>>()
/** Resolves when this editor has the picker's code in and has looked at the words before the caret. */
export const pickerReady = (view: EditorView): Promise<void> => loaders.get(view)?.(false) ?? Promise.resolve()

/**
 * Puts the picker's code into this editor (once). With `open`, the code then looks at the words before the caret: a trigger was typed
 * (or the Insert sheet asked) before the code was here. Loading on idle opens nothing: a caret that happens to sit after `#tag` is not a request.
 */
function loadPicker(view: EditorView, install: (m: Core) => Extension, slot: Compartment, open: boolean, asked?: PickKind): Promise<void> {
  return fetchCore().then((m) => {
    if (!view.dom.isConnected) return
    if (!installed.has(slot)) {
      installed.add(slot)
      view.dispatch({ effects: slot.reconfigure(install(m)) })
    }
    if (open) m.refresh(view, asked)
  })
}

/** Closes the list if it shows (Vim's Esc asks this). True when it was open. */
export function closePicker(view: EditorView): boolean {
  return core?.closeOpen(view) ?? false
}

export function inlinePicker(links: LinkOptions | undefined, tags: TagOptions | undefined, host: PickerHost | undefined): Extension {
  // A hand-typed `[[Title]]` becomes a link with or without a list to draw.
  if (!host || (!links && !tags)) return links ? closeHandler(links) : []
  const slot = new Compartment()
  const install = (m: Core) => m.pickerExtension(links, tags, host)
  const watch = ViewPlugin.define((view) => {
    const go = (open: boolean, asked?: PickKind) => loadPicker(view, install, slot, open, asked)
    loaders.set(view, go)
    // The list's code is fetched once the browser is idle after the editor shows, so the first `/` finds it here; a trigger typed sooner fetches it itself.
    const idle: (f: () => void) => void = (f) => ('requestIdleCallback' in window ? window.requestIdleCallback(f) : setTimeout(f, 500))
    idle(() => void go(false).catch(() => {}))
    return {
      update(u) {
        if (installed.has(slot)) return
        let asked: PickKind | undefined
        for (const tr of u.transactions) for (const e of tr.effects) if (e.is(askPicker)) asked = e.value
        let typed = false
        if (u.docChanged && u.transactions.some((tr) => tr.isUserEvent('input.type') || tr.isUserEvent('input.complete')))
          u.changes.iterChanges((_a, _b, _c, _d, t) => void (typed ||= TRIGGER.test(t.toString())))
        if (asked || typed) void go(true, asked).catch(() => {})
      },
    }
  })
  return [slot.of([]), watch, links ? closeHandler(links) : []]
}
