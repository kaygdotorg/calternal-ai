/**
 * The blocks the editor ships with, registered once when the editor loads.
 * A note's own title is its one `#` line, so the three heading sizes written
 * inside a note start at `##`. Image and File write nothing themselves: they ask the app
 * to open its file chooser (see `attach` below), so they are plain metadata here and the
 * heavy attachment code stays in the app's lazy chunk. Embeds are not here yet.
 *
 * Another area adds its block the same way: call `registerBlock` (exported
 * from the package) from its own module, once, when it loads.
 */

import type { EditorView } from '@codemirror/view'
import { insertTable } from '../tables/slot'
import { type BlockAction, registerBlock } from './registry'

/** What a Media block does: tells the app (an event on the editor's element, caught in apps/web lib/attachments/intake.ts) to open its chooser inside this gesture. */
const attach = (view: EditorView, what: 'photos' | 'file') => view.dom.dispatchEvent(new CustomEvent('calternal:attach', { bubbles: true, detail: what }))

const clock = () => {
  const d = new Date()
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

const DEFAULTS: BlockAction[] = [
  // ---- Basic
  { id: 'heading-1', label: 'Heading 1', group: 'Basic', icon: 'heading', badge: '1', keywords: ['h1', 'heading', 'title', 'big', 'section'], markdown: '## ', shortcut: '##' },
  { id: 'heading-2', label: 'Heading 2', group: 'Basic', icon: 'heading', badge: '2', keywords: ['h2', 'heading', 'subheading', 'medium'], markdown: '### ', shortcut: '###' },
  { id: 'heading-3', label: 'Heading 3', group: 'Basic', icon: 'heading', badge: '3', keywords: ['h3', 'heading', 'small'], markdown: '#### ', shortcut: '####' },
  { id: 'quote', label: 'Quote', group: 'Basic', icon: 'quote', keywords: ['blockquote', 'cite', 'quotation'], markdown: '> ', shortcut: '>' },
  { id: 'code', label: 'Code block', group: 'Basic', icon: 'codeblock', keywords: ['code', 'snippet', 'fence', 'program', 'mono'], markdown: '```\n\n```', caret: 4, shortcut: '```' },
  { id: 'divider', label: 'Divider', group: 'Basic', icon: 'divider', keywords: ['line', 'rule', 'separator', 'hr', 'break', 'pagebreak', 'page-break', 'page break'], markdown: '---\n', shortcut: '---' },
  { id: 'table', label: 'Table', group: 'Basic', icon: 'table', keywords: ['grid', 'rows', 'columns', 'spreadsheet'], markdown: '', run: (view) => void insertTable(view) },

  // ---- Lists
  { id: 'bullets', label: 'Bullet list', group: 'Lists', icon: 'list', keywords: ['bullet', 'list', 'unordered', 'ul', 'dots'], markdown: '- ', shortcut: '-' },
  { id: 'numbers', label: 'Numbered list', group: 'Lists', icon: 'numbered', keywords: ['numbered', 'ordered', 'ol', 'steps', '1.'], markdown: '1. ', shortcut: '1.' },
  { id: 'task', label: 'Task', group: 'Lists', icon: 'todo', keywords: ['todo', 'to-do', 'checkbox', 'check', 'checklist', 'done'], markdown: '- [ ] ', shortcut: '[]' },
  // The time it was made, as the daily note writes it: `- 09:12 words`.
  { id: 'time', label: 'Time-log line', group: 'Lists', icon: 'log', keywords: ['log', 'time', 'clock', 'now', 'journal', 'entry'], markdown: () => `- ${clock()} ` },

  // ---- Media: no markdown of their own; the app's chooser puts the file in the Home and the line in the note.
  { id: 'image', label: 'Image', group: 'Media', icon: 'image', keywords: ['photo', 'picture'], markdown: '', run: (view) => attach(view, 'photos') },
  { id: 'file', label: 'File', group: 'Media', icon: 'file', keywords: ['attachment', 'pdf'], markdown: '', run: (view) => attach(view, 'file') },

  // ---- Links: each starts the picker you would get by typing its trigger.
  { id: 'link-note', label: 'Link to a note', group: 'Links', icon: 'notelink', keywords: ['link', 'note', 'reference', 'mention', 'wiki'], markdown: '[Title](<Note.md>)', shortcut: '[[', pick: { kind: 'note', trigger: '[[' } },
  { id: 'link-day', label: 'Day link', group: 'Links', icon: 'today', keywords: ['day', 'date', 'today', 'yesterday', 'tomorrow', 'calendar', 'journal'], markdown: '[Today](<2026-10-16.md>)', shortcut: '@', pick: { kind: 'day', trigger: '@' } },
  { id: 'tag', label: 'Tag', group: 'Links', icon: 'tag', keywords: ['tag', 'label', 'hashtag', 'topic'], markdown: '#tag', shortcut: '#', pick: { kind: 'tag', trigger: '#' } },
]

/** Registers the blocks above (once; registering an id again replaces it). */
export function registerDefaultBlocks(): void {
  if (done) return
  done = true
  for (const a of DEFAULTS) registerBlock(a)
}

let done = false
