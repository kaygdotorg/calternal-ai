/**
 * The languages a code block can name (the approved mockup's list). The word after the opening fence
 * (```rust) is the only thing that picks one: nothing is guessed from the code. Plain text is a bare
 * fence. The names and the words live here, away from the grammars, so the language button and the
 * menu never pull a grammar in.
 */

export type Lang = 'python' | 'typescript' | 'javascript' | 'rust' | 'shell' | 'json' | 'css' | 'sql' | 'plain'

export const LANG_NAME: Record<Lang, string> = {
  python: 'Python',
  typescript: 'TypeScript',
  javascript: 'JavaScript',
  rust: 'Rust',
  shell: 'Shell',
  json: 'JSON',
  css: 'CSS',
  sql: 'SQL',
  plain: 'Plain text',
}

/** What the menu offers first, in the order people reach for them, then every other one, A to Z. */
export const COMMON_LANGS: Lang[] = ['plain', 'javascript', 'typescript', 'python', 'rust', 'shell']
export const MORE_LANGS: Lang[] = (Object.keys(LANG_NAME) as Lang[])
  .filter((l) => !COMMON_LANGS.includes(l))
  .sort((a, b) => LANG_NAME[a].localeCompare(LANG_NAME[b]))

/** The word written after the opening fence. Plain text is no word at all. */
export const FENCE_WORD: Record<Lang, string | undefined> = {
  plain: undefined,
  python: 'python',
  typescript: 'typescript',
  javascript: 'javascript',
  rust: 'rust',
  shell: 'shell',
  json: 'json',
  css: 'css',
  sql: 'sql',
}

const BY_WORD: Record<string, Lang> = {
  py: 'python',
  python: 'python',
  ts: 'typescript',
  tsx: 'typescript',
  typescript: 'typescript',
  js: 'javascript',
  jsx: 'javascript',
  mjs: 'javascript',
  javascript: 'javascript',
  rs: 'rust',
  rust: 'rust',
  sh: 'shell',
  bash: 'shell',
  zsh: 'shell',
  shell: 'shell',
  console: 'shell',
  json: 'json',
  css: 'css',
  sql: 'sql',
  text: 'plain',
  txt: 'plain',
  plain: 'plain',
}

/** The language a fence word names, or plain text. */
export const langOf = (word?: string): Lang => BY_WORD[(word ?? '').trim().toLowerCase()] ?? 'plain'

/** What the corner button says: a known language's name, a word we can't colour as itself, or Plain text. */
export function langLabel(word?: string): string {
  const w = (word ?? '').trim()
  const lang = langOf(w)
  return lang === 'plain' && w && !(w.toLowerCase() in BY_WORD) ? w : LANG_NAME[lang]
}

/** The info word on an opening fence line (```rust → rust), or ''. */
export function fenceWord(line: string): string {
  return /^\s*(?:```+|~~~+)\s*(\S*)/.exec(line)?.[1] ?? ''
}
