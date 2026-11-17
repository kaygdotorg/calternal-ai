# The existing app's composer, palette and shortcuts

Read-only map of `/home/kayg/Developer/calternal` at `origin/dev` (and `origin/review/switcher-a-pills` where marked), 2026-11-17. Input to the composer design round.

I've written up the composer below. One thing to know first: the date parser that ADR 0053 describes is not the one in the code. The code uses a set of regex rules, and the 823 shared test cases that ADR 0053 requires are not in the repo. Everything was read from origin/dev unless marked **[review]** (origin/review/switcher-a-pills). "mem/" means ops/orchestrator/memory/.

## 1. What it is
- **One field for search and create.** It searches, creates (task, note, contact), goes to places, runs commands and filters (ComposerBody.svelte:2-11, rows.ts:4). Shortcut owner decisions: C9 "one field" and C3 "one shared surface for search + composer" (mem/calternal-decisions-tasks.md:61,65).
- **Desktop:** a glass palette centred under the top over a dimmed page. **Phone:** the field sits on the keyboard with results above it, and the panel springs out of the search pill (Palette.svelte:4-6, 73-80; ADR 0054:43).
- **Opened by** ⌘K, the sidebar search button, the phone search pill, pull-down-to-search (pull.svelte.ts), or a list Filter's "Search everywhere" row, which carries the typed words over (Palette.svelte:34; mem/g-series.md:204-210).
- ⌘K toggles it; when words are selected in the editor, ⌘K makes a link instead (keys.svelte.ts:57-58).
- ADR 0035 is not about the composer: it is the morphing note bar plus the `/` insert grid.

## 2. Plain-language understanding (all on the device, no model, no network)
- `recognise()` reads the whole phrase. Each registered phrase plugin claims character spans and fills slots. The first plugin to vote decides the kind, and what is left over is the title (recognise.ts:97-165). Text in quotes or backticks is never read (:95).
- **Kinds:** task, note, open, contact (:10). **Span types:** date, time, repeat, deadline, priority, tag, person, email, phone, company, title (:11).
- **Tasks plugin** (phrases-tasks.ts):

| Input | Rule |
|---|---|
| Day words | today, tonight, tomorrow, tmrw, next week; can be extended (date-words.ts:21-25) |
| Weekdays | weekday, `this`/`next` weekday (N1) |
| Other dates | `in N days/weeks`, ISO dates, `12 oct` / `oct 12` |
| Deadline | `by <date>` (:90) |
| Repeat | `every N day/week/month`, every weekday, every monday, daily/weekly/monthly |
| Times | `1–2 pm` ranges, `5pm`, `17:00`, `at 5`, `fri 5`; a bare hour is guessed from the time of day and never asked (:60-67) |
| Priority | `!high`, `!`, `!!`, `urgent` |
| Tags, people | `#tag/sub`, `@person` |
| Obsidian emoji | 📅 ⏳ 🔺 ⏫ 🔼 🔽 🔁 |

- **Vote** (:160-169): a question word goes to open. Any metadata, or a leading verb (call, buy, email…, :69), makes a task. Plain words typed in the Tasks app also make a task; context only breaks ties (C19).
- **Hinglish** adds day words only: aaj, kal (taken as tomorrow), parso, agle hafte (phrases-hinglish.ts).
- **Contacts plugin:** an email, or "Name, phone…", makes a contact card. "call/email/message X [on WhatsApp]" makes an action row (phrases-contacts.ts:20,68). Both are switched off until Contacts ships (contacts-seam.ts).
- **Search filter words** (packages/ui/src/search/words.ts:4-14):
  - kinds such as notes, journal, photos, files;
  - before/after/since/between, months, `last week`;
  - `#tag`, pinned, trash, newest/oldest.
  - Only kinds notes/journal, dates, tags and sort become tokens, because those are all the server answers (filters.ts:12-15).
- **Showing what was understood:** recognised words get a soft highlight behind the text. One intent line says what Enter will do ("Task · Fri 5 pm", "Open · X", "New note") (ComposerBody.svelte:70-95).
- **Ambiguity:** tap the kind to change it (C6). Tap a highlighted word for "change it / keep as text" (C17, recognise.ts:86). ADR 0053:17 says "do not silently commit one".
- **Same parser in notes:** checkboxes in notes reuse it. Any metadata turns a checkbox into a task (recognise.ts:171).

## 3. Results, ranking and previews
- **Row kinds:** note, command, setting, action, tag (rows.ts:4).
- **Order:** actions, then commands, then tags, then notes (results.ts:209-219).
- **Two answers meet:**
  - an instant title scan of the newest 1,500 notes in memory (results.ts:20,68);
  - then the server's `search.query`: FTS5 prefix matches → typo fixes and trigrams → titles first, then frecency, then BM25, with snippets (crates/search/src/read.rs:1-10; ADR 0031:7,13).
- **Opening a hit:** jumps to the block and flashes it (0031:7). Choosing a row feeds frecency (results.ts:222).
- **Previews:** desktop has a side preview that follows the arrow keys, with hits marked and ⌘G/⇧⌘G stepping between them (hits.ts). Phone uses a long-press popover (PreviewPopover.svelte).
- **Empty field:** shows Recent after 250 ms (search-state.svelte.ts:16), plus example searches that stop after 3 uses (examples.ts:6-9).
- **Filter tokens:** a filter becomes a token when you type the space after it. Tap a token to turn it back into words; Backspace at the start of the field un-tokens the last one (ComposerBody.svelte:226-252).

## 4. Commands and phrases
- **Phrases:** `registerPhrases({id, read, vote})` (recognise.ts:75), statically imported in phrases.ts:3-4.
- **Commands:** `commandSource(fn)` (commands.ts:16). Matching is "every word in the name", and names starting with the first word rank first (:58-70).
- **Built-in commands** (commands.ts:31-52):
  - Today, New note, Previous day, Next day, Settings, plus Settings sections (Appearance, Editor, Keyboard, Account, Devices);
  - when available: Log the time (day notes), Show/hide places, Focus;
  - with a note open: Insert, Style, Note info, Show/Hide source, Show hidden blocks.
- **Search providers:** on the server, the `SearchProvider` trait (crates/core/src/search.rs:45).

## 5. Keyboard
- **App shortcuts** (packages/ui/src/lib/shortcuts.ts:9-38):

| Keys | Action |
|---|---|
| ⌘K | palette |
| ⌃⌥N | new note |
| ⌘⇧; | log the time |
| ⌘. | focus |
| ⌘\ | sidebar |
| ⌘, | settings |
| ⌘⌥↑/↓ | move block |
| ⌘D | duplicate block |
| ⇧⌘K | delete block |
| ⌥⇧⌘L | copy link to block |
| ⌘/ | insert |
| ⌘Z | undo |
| T / P / N | today / previous / next day, only outside the text and not held down (keys.svelte.ts:69) |

  Formatting in the same table: ⌘B, ⌘I, ⌘K (link), ⇧⌘X, ⌘E, ⇧⌘Y, ⌘⌥1-3, Tab / ⇧Tab.
- **Inside the composer** (ComposerBody.svelte:180-221):
  - Enter: create, or open the top result;
  - ⌘Enter: create and open; on a result while a note is open, it inserts a link;
  - ⌥Enter: run the action row;
  - ⇧Enter: add a description;
  - ↑/↓: move through rows;
  - ⌘G: next match in the preview;
  - ⌘Z: undo while the field is empty;
  - Esc: close.
- **Platform handling:** ⌘ on Mac, Ctrl elsewhere, never both. Keys match the physical key (shortcuts.ts:63-80).
- **Discovery:** a fixed list in Settings → Keyboard (shortcut-help.ts, Settings.svelte:273), tooltip key chips, `aria-keyshortcuts`, and every action also in ⌘K. On dev there is no ⌘/ or ? sheet and no remapping.
- **[review]** Keyboard shortcuts sheet, KS1-KS5 (chrome/KeysSheet.svelte:4-9; lib/shortcuts.ts:42-44):
  - opens with ? or ⌘/;
  - tabs: [this app] [Global] [other apps];
  - filter by name or by keys;
  - record keys in place; a conflict shows inline with "Swap"; browser- and OS-owned keys are refused (keymap.ts:180); ↺ resets one, and the tab's ⋯ holds "Reset this app's shortcuts";
  - only shown on phones and iPads that have a hardware keyboard.
  - Storage: `cal.keymap` in localStorage. The product would keep it with the account, with a per-device override planned (keymap.svelte.ts:5). ADR 0055:19 instead says shortcuts are device-local, so these two records conflict.

## 6. Performance
- **Budgets** (perf/budgets.json):
  - recogniser and composer keystroke ≤8 ms at 10k notes with 6× CPU slowdown (:179-180);
  - zero blank frames (:181);
  - `search.query` p95 100 ms (:12); first query 1 s, not gated (:13);
  - ADR 0031:7 says first results under 50 ms; ADR 0053:73 says parse ≤1 ms p95.
- **Techniques:**
  - no debounce; the engine is asked after the key's own work (setTimeout 0);
  - one request in flight, keeping only the latest question (results.ts:148-173);
  - the last rows are held 160 ms so no frame is empty (hold.ts:6);
  - a warm-up parse while the palette opens (ComposerBody.svelte:96-104);
  - regexes built once, titles lower-cased once and cached (results.ts:30);
  - one index per person, rebuilt from data (0031:10);
  - TypeScript rather than WASM in the browser (0053:15).
  - No web worker is used for search.

## 7. Owner decisions
- "Search: ONE palette for everything, so it must be extremely fast" (mem/calternal-design-notes-ui.md:18).
- C1: "real intent recognition from typed words (far better NLP than prefixes/kind-cycling)" (tasks.md:61).
- C4, C5, C6, C7: as few UI elements as possible; "our own standard NLP engine… on device"; tap the kind to change it, no learning in v1; Enter / ⌘Enter / Esc (tasks.md:64).
- C11 "everything is a plugin"; C12 "8 ms per keystroke on the oldest phone" (:65).
- C13 v1 kinds: task, note, contact, go to; "App actions are result rows, not kinds" (:125). C14, C15, C18, C19, C20, C22 (:117-134).
- "search should tolerate typos and should be extremely fast!" (mem/feedback-search-gate.md:7).
- K1: shortcuts must be "STANDARD (most common convention wins; browser-owned ones dropped; everything also in ⌘K)" (r15.md:81). ⌘E is never used (g-series.md:181).
- S2/S4/S7: chips from natural language; typos tolerated like Spotify; power syntax documented only (r15.md:44).
- The composer finds settings; Settings has no search box (0055:15).
- Outward actions such as calling or sending need confirmation (0066:375-380).
- "Edit view…" opens ⌘K with the collection's query as chips (g-series.md:237).
- **Rejected:**
  - prefixes and kind-cycling;
  - chrono-node or another library as the grammar; WASM in the browser; a general grammar or guessing from any date-like token (0053:59-64);
  - a separate search engine; separate searches per area (0031:19-22);
  - a side panel; floating bubbles over the text (0035:13-16);
  - a search box inside Settings.

## 8. Built versus not built
- **Built on dev:**
  - the composer: tasks, notes, filters, previews, paste-as-list, undo;
  - the FTS5 search engine;
  - the fixed shortcut list.
- **Not built:**
  - **The ADR 0053 parser.** The tokenizer engine and the 823-vector corpus are absent. ADR 0066:363 says "not built yet"; today's code is regex rules. Also missing: recurrence as an RRULE, all-number dates by locale, and time zones.
  - Contacts phrases (behind a switched-off seam).
  - Search across individual setting names (only sections are listed today).
  - ⌘⇧K scoped search (⇧⌘K is "delete block").
  - Search kinds other than notes/journal.
  - Shortcut remapping and the ? sheet (mockup only).
  - **[review]** mockup only: ⌘K "Ask about this", the link picker reusing the palette core (search/pick.ts), and the AI chat composer (ai/Composer.svelte).

## 9. Open points for a multi-user, multi-space, plugin-based app with an AI-agent composer
1. **Plugin registration:** phrases and commands are client-side and statically imported. There is no manifest or sandbox for plugins, and no rule for ordering or arbitrating between plugins that claim the same words.
2. **Spaces:** no space or tenant scope in tokens, commands or the per-person index. It is undecided whether the palette spans spaces.
3. **Chat versus intent:** how the field tells "message to an agent" apart from create or search. AI notes have chat-input decisions (L8 says it needs its own grill, ai.md:45; FA3 says it was settled by the 11-02 composer-toolbar review, :145) but no decision on merging it with the palette. ⌘J is the AI side panel.
4. **Model use:** whether a model may parse at all. Today the rule is no model, and the intent line must render in 8 ms.
5. **Outward actions:** the confirm rule needs extending to agent sends, @mentions of people versus agents, and per-agent scopes (ai.md:24).
6. **Keymap storage:** account-wide or per-device is unresolved (ADR 0055:19 versus the review-branch keymap). Plugin-contributed shortcuts and their conflict namespaces are also undecided.
7. **Search permissions:** server-side checks per space (0031:7) need defining for shared and agent sessions.
8. **Languages:** only English plus Hinglish day words exist, and the language-pack interface is not formalised.
