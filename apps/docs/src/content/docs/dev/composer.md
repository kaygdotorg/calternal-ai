---
title: "Composer design"
---

<!-- Generated from: docs/composer.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Decided with kayg, 2026-11-17. The existing app's design carries over unchanged unless
this says otherwise; its map is in `reviews/composer-map-2026-11-17.md`. Terms:
[glossary](/docs/dev/glossary/).

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

## Chips and adaptive rows (decided 2026-11-18, the owner's grill)

1. **The chips are Attach, Model, Permission and Runs on.** The workspace stays in the context row above the field.
2. **The reasoning level lives inside the model chip.** The picker offers the levels each model accepts (the backend lane's operation, #46, will list them; until then a model offers none and the picker says so).
3. **The model chip reads: provider icon, model name, reasoning level.** No middle dots anywhere: an icon, then one label; a second fact is a second icon (the reasoning level is a small level mark) or goes in the tooltip.
4. **Runs on is the machine's name and a shield icon when sandboxed** (no word).
5. **Adaptive by the composer's own width**, not the window's (a measuring row, not a media query). As it narrows, chips drop from icon and text to icon only, in this order: Runs on, then Permission, then Model. A collapsed chip keeps its full text in its tooltip and as its accessible name.
6. **Narrowest (a phone): every chip icon only, except a chip holding a non-default choice, which keeps its text.** Full access also keeps a warning tint, so it is never hidden.
7. **Closed (owner, 2026-11-18): the chips are always shown, never hidden at their default value.** Built that way from the start (all four chips are in the row whatever they hold; `e2e/chips.mjs`); only a chip's words give way to width, never the chip.
7a. **Closed (owner, 2026-11-18): the palette and the composer are two surfaces of one engine** (question #30), as the top of this document already says: one parser, one set of recognised words and mentions, two places to type.
8. **The rule is the owner's for the whole product** ("pretty much everywhere we need to be adaptive, going from icons + text or text to icons only when there's less width available"), so it is built once: `PillRow` in `packages/ui` (a row of pills that measures itself, with the pure decision in `lib/fit.ts`). The composer, the right sidebar's tab capsule and the app switcher use that one primitive.
9. **Reasoning level, from omp** (oh-my-pi, by Stencil; `https://omp.sh/docs/using`, "Choose the model and thinking level", read 2026-11-18; nothing copied): the model and its thinking level are always visible in its status line; one key cycles the thinking effort the active model supports (Shift+Tab, with an Auto step), and a model that has no configurable thinking says so instead of changing; the transcript's thinking blocks are shown or hidden by a separate key, so visibility and effort are two controls. Taken here: the level is always visible on the model chip; the picker lists only the levels the model accepts and says plainly when it accepts none; showing the reasoning text (the folded "Thought for" block) is separate from the level. A one-key cycle of the level on the chip is worth adding once the levels are real.

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

## The keyboard on a phone in a Safari tab

iOS Safari keeps the layout viewport at full height under the keyboard and pans the visual viewport instead, so a fixed box stays where it was while the page scrolls under it: the header ends up above the screen and text runs under the status bar with no fade. While a text field is focused under a keyboard, `shell/chrome.ts` writes the visual viewport's top and height to `--vvt` and `--vvh` and marks the root `data-kb`; the shell and its three panes then occupy exactly the visible area (header in view, composer on the keyboard, the side panel's field on the keyboard too). Verified on an iPhone 18 Pro Simulator (the screen's own state was logged on the device: `scrollY` 330, visual viewport offset 330, header at y = -326 before the change).
