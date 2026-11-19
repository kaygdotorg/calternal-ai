/**
 * The raw lezer parsers for code blocks, one lazy chunk each (the build groups them, see vite.config.ts).
 * Only the parser is used, never CodeMirror's language wrappers: those would pull their helpers into the
 * first-paint bundle. A parser turns a block's text into a tree; code/colours.ts reads the colours off it.
 */
import type { Parser } from '@lezer/common'
import type { Lang } from '../langs'

type Lezer = Exclude<Lang, 'plain' | 'shell' | 'sql'>

const load: Record<Lezer, () => Promise<Parser>> = {
  javascript: () => import('./javascript').then((m) => m.parser),
  typescript: () => import('./typescript').then((m) => m.parser),
  python: () => import('./python').then((m) => m.parser),
  rust: () => import('./rust').then((m) => m.parser),
  json: () => import('./json').then((m) => m.parser),
  css: () => import('./css').then((m) => m.parser),
}

export const isLezer = (lang: Lang): lang is Lezer => lang in load
export const loadParser = (lang: Lezer): Promise<Parser> => load[lang]()
