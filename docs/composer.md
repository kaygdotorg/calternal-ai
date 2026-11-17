# The composer and the palette

Decided with kayg, 2026-11-17. The existing app's design carries over unchanged unless
this says otherwise; its map is in `reviews/composer-map-2026-11-17.md`. Terms:
[glossary](glossary.md).

## Carried over

- One field for search, create, go to, commands and filters; opened with Cmd-K, the search
  button, or pull-down on a phone.
- Plain language is read on the device with no model: recognised words are highlighted,
  one line says what Enter will do, tap the kind to change it, tap a word to keep it as text.
- Budgets: 8 ms per keystroke, search within 100 ms, never an empty frame.
- Typo-tolerant search with previews. Standard shortcuts, each also in the palette.
- Outward actions (sending, calling) confirm first.

## New decisions

- **Two surfaces, one engine.** The palette (Cmd-K, anywhere) does what its line says and
  has an "Ask" row that sends the text to an agent. A thread's composer sends to the agent
  on Enter, with the same highlighting, mentions and chips. Built as one modular engine.
- **No model while typing.** Parsing stays instant and local.
- **Plugins add phrases and commands as data** in their manifest; the app does the
  matching, so no plugin code runs per keystroke. The open app breaks ties.
- **The palette searches every space the person can access**, and must never be slow.
  Results say which space they are in.
- **Shortcuts are a file per account** (file over app), with per-device overrides (for
  example Mac and Windows), and agents can change them through operations. Discoverable
  with Cmd-/ or ?, remapped in place, conflicts offer a swap.
- **Settings are a plugin**, so everything in them is an operation an agent can call.
- **The parser is the existing app's working rules, ported**: TypeScript for the browser
  and Rust for the Apple apps, kept identical by shared test cases. The ADR 0053 engine
  can replace it later.
- `@` opens one picker for people, agents, threads and items, grouped by kind. `/` in a
  thread lists skills.
- **Every inline field is the Notes block editor**: the composer, rename fields, the
  palette field. The block editor therefore has to be extremely fast.

## Decision service

A fast typed-decision model (OpenAI's Decisions API, TypeSafe's Jev: yes/no with a
probability, a choice, a score; tens to hundreds of milliseconds) is an optional,
replaceable service slot, off until a key is connected, with rules as the fallback. It
is never used while typing. Uses: resolving an ambiguous line after Enter; the automatic
reviewer in Approve for me for actions that cannot be undone; routing a request to an
agent or model; deciding what deserves the attention card. The text judged leaves the
server, so it is the person's or the space's choice.

## The block editor

Every inline field is the Notes block editor, which is taken from the existing app:
rewritten with reference to it and simplified wherever that helps speed, modularity and
reuse. It gets its own performance budget before anything is built on it.
