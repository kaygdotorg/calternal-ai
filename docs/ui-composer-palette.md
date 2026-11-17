# The composer and the palette in the web shell (plan and status)

Plan for bringing the existing app's composer and palette across, following `docs/composer.md` (decisions) and `docs/reviews/composer-map-2026-11-17.md` (how the existing app does it). Built on fixtures in `apps/web/src/palette/`.

## What is ported, what is new

| Piece | Source | Here |
| --- | --- | --- |
| Recogniser (the whole phrase, spans, slots, kind vote) | existing `composer/recognise.ts` | `palette/recognise.ts`, unchanged except imports |
| Dates, day words, Hinglish day words | `dates.ts`, `date-words.ts`, `phrases-hinglish.ts` | same names |
| Task phrases (dates, times, repeat, deadline, priority, #tag, @person) | `phrases-tasks.ts` | same name; contacts phrases are not ported (switched off there too) |
| The 25 test vectors | `recognise.test.ts` | `palette/recognise.test.ts` (the 823-vector corpus of ADR 0053 does not exist yet) |
| Two surfaces, one engine | new (`composer.md`) | `palette/engine.ts` is the engine; `Palette.svelte` is the palette surface; the thread composer uses the same `parse` and `search` next |
| Plugin phrases and commands as data | new | `palette/manifest.ts`: a plugin lists phrases and commands; the engine matches them, no plugin code runs per keystroke |
| Typo-tolerant search over every space | new on fixtures | `palette/search.ts` (prefix, word-prefix, subsequence and one-edit typos over lower-cased titles built once); results carry their space |

## The engine, in order

1. `parse(text)`: the ported recogniser. No model, no network. Quoted or backticked text is never read; a word the person taps is kept as text.
2. `plan(parsed, hits)`: one line saying what Enter will do ("Open: Split store.rs", "New thread: ...", "Ask Sonnet 5.5: ...", "Task: Fri 5 pm", "Go to Settings"). The kind is changeable (arrow to another row); outward actions (send to an agent) are the last row and always shown.
3. `search(query)`: threads and notes in every space, commands and settings sections from the manifests, typo tolerant. Ordering: what the line says, commands, threads (title matches before body), notes. The open app breaks ties.

## Budgets (checked by tests on the dev VM, not claims)

- Keystroke work (parse + plan + search over 2,000 fixture items): under 8 ms median on the dev VM; the bench test prints the number. The 6x CPU slowdown run for the phone is a Playwright step in `e2e/palette.mjs`.
- Never an empty frame: the previous rows stay until the new ones are ready (they are computed in the same tick, so there is no gap).
- Constant work is built once: regexes (in the phrases), the lower-cased title index (on data change), the command list.

## Surface

- Opens with Cmd/Ctrl-K, the sidebar's search button, and on a phone as a sheet from the bottom. A top-centred glass panel on a desktop.
- One `InlineEditor` field; recognised words are drawn highlighted behind the text (a mirror layer) with the kind's name in a small pill; Up/Down move, Enter does, Esc closes, Tab keeps the line as text.
- Rows show the space they are in when it is not the current one.

## Not built yet

- The thread composer using the same engine (highlighting, `@` picker for people, agents, threads and items, `/` for skills). The engine exports what it needs; wiring is the next step.
- Filter tokens, previews, frecency, recents. (The existing app has them; they come with the real search service.)
- Plugin manifests from the server: the data shape is fixed in `manifest.ts`; the fixture registers the built-in apps.
- The decision service (optional, off): only after Enter, never while typing.
