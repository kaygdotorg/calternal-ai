# The host component catalogue (UI side of the plugin interface plan)

2026-11-18 · planning and measurement, no renderer written · ui-shell lane · issue #15 · answers the UI half of [plugin-interface-proposal.md](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/plugin-dx/docs/plugin-interface-proposal.md) (lane #50)

The decision (owner, 2026-11-18): a plugin's interface runs in a worker and sends the host a tree of the host's own components with small patches, and gets typed events back; sealed frames only for what the catalogue cannot express; the shell itself becomes a plugin that supplies a layout tree; the core (sign-in, approvals, secrets, recovery) cannot be replaced or styled; official and community plugins are treated the same. This document says what the web renderer must offer for that to be true, proves it by writing the two layouts we have as trees, measures what a tree update costs in a browser, and orders the migration so that no component ever has two implementations.

Words: **host component** = a Svelte component the web client owns (`packages/ui`, the shell, Threads, Notes). **Catalogue** = the list of host components a tree may name, with typed props and events; it extends GenUI's (`packages/genui/src/catalog-data.json`, 67 components, 4 actions, the source of truth the Rust and Swift sides share). **Tree** = nodes `{id, type, props, children}`. **Patch** = a bounded change to a tree by node id.

## 0. Summary

- The catalogue today has 67 GenUI components, nearly all *content* (text, tables, charts, forms). What the shell and the two apps need that it does not have is **structure and chrome**: regions, a strip of windows, pills and capsules, menus, a virtual list, an editor, a composer. About half of those already exist as Svelte components in `packages/ui` and only need a catalogue entry; the rest are app components (transcript, composer, editor) that stay entirely in the host behind one catalogue name each.
- Measured on dev-02: **the boundary is not the cost.** Applying a patch takes 0.1 ms at p50 (1.9 ms at p95 at 4x), the same in a worker as on the main thread. The cost is the DOM the patch causes, which is the same DOM a hand-written component causes. A 10,000-row list through the virtual list, a 60 Hz streaming message and opening a window all held 60 fps at 1x; at 4x CPU throttling only layout-heavy updates (resizing a window with a 30-row list every frame) drop frames.
- A sealed frame costs **about 40 ms to mount at 1x (100 ms at 4x) and about 0.5 MB of heap each**, needs CORS headers on every font it loads (otherwise it silently falls back to a system font), swallows keystrokes (the host sees none), and, contrary to the proposal's wording, **can blur the host's pixels behind it** in both Chromium and iOS Safari (so "no independent blur" is a policy the host must enforce by where it places frames, not a browser limit).
- Migration (§5): the renderer is GenUI's existing one made *live*; the first thing to become tree-driven is something that has no legacy (the Skills and Automations pages), then in-place leaf swaps, then the shell's layout, then the canvas as a second layout tree, then Threads' transcript and composer last. The worker boundary is added last because it is only a transport.

Timings are from the shared dev VM (calternal-dev-02-std), headless Chromium, with CPU throttling 1x and 4x. They are indicative, not benchmark passes (AGENTS.md: measure performance on the perf VMs). Run to run the 4x numbers moved by up to 2x with other lanes' load; the tables show the range.

## 1. The host catalogue

Legend. **G** = exists in the GenUI catalogue today. **U** = exists as a component in `packages/ui` (or the app) but has no catalogue entry. **M** = missing (nothing to reuse). **host** = stays inside the host: the plugin sends data and gets semantic events, never the mechanism. Events are asynchronous and bounded; none carries a raw keystroke, pointer move or scroll offset.

### 1.1 Layout and chrome

| Component | Props | Events | Stays in the host | Status |
| --- | --- | --- | --- | --- |
| `Layout` (root of a shell tree) | `regions` | none | the whole window: safe areas, backdrop, the one veil, one material, focus order, recovery surfaces | M (today `Shell.svelte`) |
| `Region` | `edge: left\|right\|center`, `size`, `min`, `max`, `resizable`, `collapsible`, `pull: bool`, `phone: "underlay"\|"stack"` | `resized {size}`, `toggled {open}` | the pull gesture and its rules (`shell/pull.ts`), resize handles, settle animation on the sheet tokens, history entries on a phone, the material solved from what is behind | M (`Shell`, `ResizeHandle`) |
| `Card` | `title?`, `variant`, `children` | none | radius from the scale, glass, the card's veil | **G** (GenUI `Card`) and U (`packages/ui` `Card`: the two are different: GenUI's is a content box, `ui`'s is the middle card; the catalogue needs both names) |
| `Strip` | `axis`, `gap`, `snap`, `children` (windows) | `scrolled {index}` | scrolling, snap, momentum, the minimap | M (`canvas/Canvas.svelte`) |
| `Window` | `title`, `w`, `h`, `x?`, `edge`, `children`, `actions` | `moved`, `sized`, `closed`, `focused` | drag and resize gestures (host-owned, clamped), z-order, the frame's chrome, stable address | M (`canvas/WindowFrame.svelte`) |
| `Stack`, `Row`, `Grid`, `Divider`, `Tabs`, `Details` | as GenUI | `Tabs`: `selected {i}` | layout maths | **G** |
| `Slot` | `name`, `fallback?` | none | resolving which plugins contribute to `name` in this space, ordering, scoping | M |
| `Backdrop` | none | none | wallpaper, blur, veil solver | U (`shell/Backdrop`), host-only: not offered to plugins (core look) |
| `Sheet` | `side`, `label`, `children`, `open` | `closed` | focus trap, scrim, drag-to-dismiss, inset corners (`sheet-radius`) | U |
| `Menu` / `Popover` | `items[{id,label,sub,icon,checked,danger,disabled}]`, `label`, `search`, `anchor` | `picked {id}`, `closed` | placement, morph from the trigger, roving focus, the dropdown-or-sheet rule | U |
| `Toolbar`, `Capsule`, `PillRow`, `Pill`, `IconButton`, `Button` | `items[{id,label,icon}]`, `value`, `drop` order, `keep`, `floor` | `selected {id}`, `pressed` | adaptive icon+text to icon-only (fit.ts), tooltips, roving focus, press scale | U (GenUI `Button` exists as **G**; `Pill`/`Capsule`/`PillRow`/`Toolbar` are U) |
| `Selector` (segmented, "selection-state switching") | `options[{id,label}]`, `value` | `selected {id}` | the sliding indicator, edge-case measure batching | U (GenUI `Choice` is rendered with it) |
| `Switch`, `StepSlider`, `Badge`, `Spinner`, `Tick`, `Icon` | as the component | `Switch`: `changed {on}`; `StepSlider`: `changed {i}` | spring, tick draw | **G** `Badge`, `Icon`, `Toggle`; U `Switch` (`Toggle` is G), `StepSlider`, `Spinner`, `Tick` |
| `Toast` | `text`, `tone`, `undo?` | `undone`, `dismissed` | queue, timing, announcements, limits | U (`packages/ui/src/toast`) |
| `AppSwitcher` | `apps[{id,label}]`, `value`, `placement: card\|bar` | `selected {id}`, `reordered` | scroll, snap, order preference, keys | U (`shell/AppSwitcher`) |
| `CommandPalette` | `sources`, `phrases` | `ran {action}` | the field, the recogniser, results, two-column preview, shortcut | U (`shell/Palette`): contributions are *sources* (rows) and *actions*, not the palette itself |

### 1.2 Lists, rows and data

| Component | Props | Events | Stays in the host | Status |
| --- | --- | --- | --- | --- |
| `List` (rows) | `items[{title,detail,icon,value,tone,action}]`, `label` | `picked {i}` | roving focus, selection state | **G** |
| `Row` (sidebar/settings row) | `title`, `sub`, `meta`, `icon`, `href`, `selected`, `depth`, `trailing` | `pressed`, `menu` | link vs button semantics, swipe actions (`SwipeRow`), long-press menu | U (`packages/ui` `Row`) |
| `VirtualList` | `count`, `estimate`, `label`, `startAtBottom`, `inside`, `rowType`, `rows` (a *page*: `{start, items}`) | `range {first, end}`, `scrolledToEnd` | windowing, the Fenwick height tree, measuring, scroll position and anchoring, "stays at the bottom while streaming" | U (`VirtualList`); the plugin never holds 10,000 rows (§3) |
| `Paged` list source | `cursor`, `pageSize`, `drop`, `has` | `needPage {cursor}` | page cache, refresh keeping scrolled pages | U (`notes/paged.svelte.ts`) |
| `Table`, `Chart`, `Sparkline`, `Progress`, `Steps`, `Timeline`, `Tree`, `Code`, `Diff`, `Diagram`, `Calendar`, `Gauge`, `CalendarHeatmap`, `SegmentedBar`, `KeyValue`, `Metric`, `Image`, `Gallery`, `Map`, `Text`, `Callout`, `Entity`, `Sources` | as GenUI | per component | rendering, fonts, tokens | **G** |
| `Markdown` (rich text, read-only) | `text`, `streaming` | `linkOpened`, `copied` | parsing once, code highlighting, tables, caret, the shared text styles | U (`lib/Md.svelte`); GenUI `Text` is plain/limited |

### 1.3 Inputs and editors

| Component | Props | Events | Stays in the host | Status |
| --- | --- | --- | --- | --- |
| `Input`, `TextArea`, `Select`, `Radio`, `Checklist`, `Slider`, `Stepper`, `DateInput`, `Rating`, `Form`, `ListEditor`, `EditableTable`, `OptionCards`, `Swatches`, `FollowUps` | as GenUI (bindings `holds`) | `changed`, `submitted` (semantic, debounced) | typing, IME, selection, caret, validation, autofill | **G** |
| `TextField` with marks (`InlineEditor`) | `value`, `placeholder`, `marks[{start,end,type}]`, `multiline` | `changed`, `submitted` | the field, the highlight layer, scroll sync, IME | U (`InlineEditor`) |
| `Editor` (block Markdown editor) | `docId`, `features`, `readonly`, `width` | `changed {revision}`, `linkOpened`, `selection {text}` (when asked), `saved` | CodeMirror, frontmatter/ids/tables/code features loaded lazily, undo, IME, selection, paste, drag, links, scroll, the revision and merge protocol | U (`packages/editor`, `NoteEditor`) |
| `Composer` | `value?` (initial only), `chips[{id,label,icon,priority,menu}]`, `attach`, `placeholder`, `running`, `queued` | `sent {text, attachments}`, `chip {id, picked}`, `steered`, `stopped`, `chipsChanged` | the editor, send and offline queue, paste/drop/progress, fold/expand motion, keyboard sizing; a plugin proposes chips and gets semantic events, never keystrokes | U (`threads/Composer.svelte`) |

### 1.4 App components that stay one catalogue name each

These are large, stateful and gesture-heavy. The decision (§5) is that they enter the catalogue as **one component each**, not as trees of their parts: the plugin chooses where they go and what data they get, the host draws them.

| Component | Props the plugin sends | Events | Host-owned |
| --- | --- | --- | --- |
| `Transcript` (thread view body) | `threadId`, `page`, `appendTo {messageId, text}`, `state` | `range`, `action {messageId, id}`, `selected {text}` | the virtual list, message rows, steps/thought folding, streaming append at a fixed rate, selection actions, the request bar |
| `ThreadHeader` | `title`, `state`, `actions[]` | `renamed`, `action {id}` | the title pill, overflow menu, safe-area and edge-button room |
| `ThreadRail` / `PanelTabs` | `panels[{id,label,icon}]`, `value` | `selected` | the tab capsule (PillRow), panel mounting, resize bounds |
| `RequestForm` | `kind`, `fields` | `answered`, `expired` | trusted for approvals/secrets: **never plugin-drawn** (core) |
| `Sidebar` section | `rows`, `groups`, `topEntries`, `inbox` | `picked`, `menu`, `moved` | swipe rows, drag reorder, sections, paging |
| `SettingsPage` | `groups[{title,rows}]` | `changed {id,value}` | row types (switch, popup, order, slider, custom), search, the danger area |

### 1.5 What can never be in the catalogue

Sign-in and passkey forms, the approval and request forms of the host turn path, secret entry, recovery, the account and plugin-permission screens, the shortcut layer (a plugin may name an action; the host binds keys), and any component that would let a tree read pixels or other plugins' trees. They are foundation (plugin-interface-proposal "protected forms").

### 1.6 Counts

GenUI today: 67 components. Needed in addition, from §1.1 to §1.4: **U** (catalogue entry only, the component exists): `Card`(ui), `Sheet`, `Menu`, `Popover`, `Toolbar`, `Capsule`, `PillRow`, `Pill`, `IconButton`, `Selector`, `Switch`, `StepSlider`, `Spinner`, `Tick`, `Toast`, `AppSwitcher`, `CommandPalette`, `Row`, `VirtualList`, `Paged`, `Markdown`, `TextField`, `Editor`, `Composer`, `Transcript`, `ThreadHeader`, `PanelTabs`, `Sidebar` section, `SettingsPage` (29). **M** (new work): `Layout`, `Region`, `Strip`, `Window`, `Slot` (5), plus the patch and event protocol itself. `RequestForm` is deliberately not offered.

## 2. Two layouts written as trees

The shell plugin returns a `Layout` tree; apps and other plugins fill the `Slot`s. Component names are §1's. A prop in `{{ }}` is a reactive value the host supplies (current space, current app, what is open), so the plugin does not re-send the tree when it changes.

### 2.1 Today's shell

```
Layout {
  Region id=left   edge=left  size=240 min=200 max=420 resizable collapsible pull phone=underlay {
    Slot name="sidebar:{{app}}"                       // the current app's sidebar (Threads list, Notes list…)
    Row  [ SpaceDots | Pill(search) | Avatar→Menu ]   // footer, host-supplied pieces
  }
  Region id=center edge=center {
    Card variant=pane {
      Slot name="card:{{app}}:{{address}}"            // thread, note, settings page
      AppSwitcher apps={{apps}} value={{app}} placement=card   // wide: under the card; phone: bar
    }
  }
  Region id=right  edge=right size=340 min=280 max=560 resizable collapsible pull phone=underlay {
    PanelTabs panels={{panels}}                       // Chat, Changes, Run, Files…
    Slot name="panel:{{app}}:{{panel}}"
  }
}
```

### 2.2 The canvas

No inner card: the area between the sidebars is the canvas and each window is a middle card.

```
Layout {
  Region id=left   …same…
  Region id=center edge=center {
    Strip axis=x gap=8 snap="start" {
      for w in {{windows}}:
        Window id=w.id title=w.title w=w.w edge=w.edge actions=[expand, link, menu] {
          Slot name="card:{{w.app}}:{{w.address}}"
        }
    }
    AppSwitcher apps={{apps}} value={{app}} placement=card   // floats over the canvas
  }
  Region id=right  …same…
}
```

Opening a window is one patch: `insert {parent:"strip", index, node: Window{…}}`. The host animates it into its slot and owns its gestures.

### 2.3 What "anybody can make the app look however they want" means

A shell plugin **can**: change the layout (which regions exist, their edges, sizes, whether windows are a strip, a grid or tabs), the chrome (which pills, capsules, menus and where), which named slots exist and which app content goes in them, the order and grouping of apps, tokens and themes (colours, type scale, radii *within the scale's rules*, density, wallpaper presets, motion presets inside the reduced-motion rule), icons and labels.

It **cannot**: draw outside the catalogue (that is a sealed frame, §4), replace or restyle sign-in, approvals, secrets, recovery, the permission screens or the shortcut layer; touch the backdrop's solver or the one-material rule (it picks a theme, not a shader); change how a component behaves (a `VirtualList` always windows, a `Composer` always owns its editor and queue); receive raw input, scroll or pointer streams; read another plugin's tree; make a region cover the recovery surface; shrink touch targets below the host minimum (`--pill-h`) or break the safe areas. Contrast, focus rings and reduced motion are the host's: a theme that fails the contrast check is refused at install, not drawn.

Official and community shell plugins are the same: the seed catalogue chooses the default, nothing in host code names the official one.

## 3. What a tree update costs in the browser (measured)

Bench: `apps/web/bench/ui-tree/` (throwaway, not part of the app build; README there). A worker generates patches and posts them; a 90-line recursive renderer maps nodes onto the **real** `packages/ui` components (`VirtualList`, `Row`, `Panel` with glass, `Button`). It applies patches to a store the components read. Scenarios, each 3.5 to 5 s, at CPU throttling 1x and 4x (CDP), Chromium (Nix) on dev-02, load 2 to 8:

- **list**: a 10,000-row `VirtualList` (44 px estimate); the plugin sends only the visible page when the host reports its range (`range` event, `rows` patch). Scroll at 6 px per ms; and 30 scrollbar-drag jumps to a far row.
- **stream**: a message grows by 45 characters every 1/60 s (about 2.7 k characters per second) through `append` patches.
- **resize**: one window's width changes every frame (a drag) with 30 rows inside.
- **window**: a window with a 30-row list and a button is inserted into a strip of three, then removed, 4 times a second.
- Each also run **direct** (the same patches generated on the main thread, no worker) to separate the boundary from the drawing.

Frame time is the interval between `requestAnimationFrame` callbacks (60 Hz here: 16.7 ms is perfect). Main-thread busy is Chromium's `TaskDuration` per second (an idle page costs 6 to 12 ms/s at 1x). "Apply" is the time to put a patch into the store.

| Scenario (1x) | frame ms p50 / p99 / max | frames over 25 ms | main-thread busy ms/s (script · layout · style) | patch to painted p50 / p95 ms | apply p50 / p95 ms |
| --- | --- | --- | --- | --- | --- |
| idle | 16.7 / 16.8 / 16.8 | 0 | 6 to 12 | | |
| list scroll (10,000 rows) | 16.7 / 16.8 / 16.8 (one run: 66.7, 7 frames over, 2 long tasks) | 0 to 7 | 131 to 366 (41 to 137 · 14 to 40 · 13 to 37) | 17.7 / 19.1 | 0.0 / 0.1 |
| stream 2.7 k chars/s | 16.7 / 16.8 / 16.8 | 0 | 44 to 56 (7 to 9 · 17 to 21 · 0) | 9.0 / 17.0 | 0.0 / 0.1 |
| resize every frame | 16.7 / 16.8 / 16.8 | 0 | 79 to 107 (7 to 10 · 19 to 27 · 8 to 11) | 9.5 / 17.6 | 0.0 / 0.1 |
| open/close a window, 4 per s | 16.7 / 16.8 / 16.8 | 0 | 16 to 23 | 6 to 14 / 11 to 19 | 0.1 / 0.3 |

| Scenario (4x CPU) | frame ms p50 / p99 / max | frames over 25 ms | main-thread busy ms/s | patch to painted p50 / p95 ms | apply p50 / p95 ms |
| --- | --- | --- | --- | --- | --- |
| idle | 16.7 / 16.8 / 16.8 | 0 | 11 to 31 | | |
| list scroll | 16.7 / 16.8 to 33 / 16.8 to 50 | 0 to 19 | 536 to 786 | 21 to 24 / 24 to 52 | 0.1 / 0.9 to 1.1 |
| stream | 16.7 / 16.8 / 16.8 | 0 | 117 to 245 | 11 to 12 / 19 to 21 | 0.0 / 0.1 to 0.9 |
| resize every frame | 16.7 / 16.8 to 66 / up to 67 | 0 to 26 | 256 to 676 (layout 76 to 204) | 13 to 22 / 22 to 54 | 0.0 / 0.4 to 0.9 |
| open/close a window | 16.7 / 16.8 / 16.8 | 0 | 36 to 90 | 12 to 17 / 24 to 32 | 0.1 / 1.1 to 3.1 |

Scrollbar-drag jumps to a far row (30): 1x p50 4.6 ms, p95 7.3 ms, max 15.1; 4x p50 3.5 ms, p95 50 ms, max 56 (one jump missed a frame). Placeholder rows ("…") seen while scrolling at 1x: 0 % of frames; at 4x 1 % of frames, at worst 11 % of that frame's rows. The page is the plugin's: the worker answered the `range` event within a frame.

Reading the numbers:

1. **The worker boundary is invisible.** Apply is 0.0 to 0.1 ms at p50 and at most 1.9 ms at p95 (4x), the same worker or direct. This confirms and extends lane #50's 0.08 ms for 60 rows: the drawing is the cost, and it is the cost the components would have had anyway (the renderer is a thin map onto the same components).
2. **A tree is as cheap as the DOM it makes.** Streaming a message and opening a window never dropped a frame at 4x. The things that did were layout-bound: scrolling 10,000 rows through the virtual list at 4x with measuring (up to 19 frames over 25 ms in 3.5 s, one in three runs), and resizing a window with 30 rows every frame at 4x (layout 76 to 204 ms/s). These are properties of the host components (`VirtualList` measuring, a window re-laying out its list), unchanged by the tree. The consequences are rules for the protocol: coalesce streaming to a fixed rate (60 Hz was fine; per-token patches are unnecessary), never send 10,000 rows (a full snapshot is 7.5 ms of structured clone alone on perf-02, lane #50's figure), keep gestures (drag, resize, scroll) in the host and send only their end state (`sized {w}`), and send *ranges*.
3. **Patch to painted is 9 to 24 ms at 1x**, which is mostly waiting for the next frame; it is one frame plus apply. At 4x up to 54 ms at p95 for resize. An interaction that must feel instant (typing, selection, a button's press) therefore never goes through the plugin: the host component reacts, then tells the plugin.
4. What was **not** measured: the cost of the *first* mount of a big tree (the bench mounts the 4-window shell once), validation of untrusted patches against the catalogue (GenUI's existing validation is pure TypeScript and was not in the loop), memory of a worker (lane #50), Safari/WebKit frame times on a device (the iPhone Simulator ran the sealed-frame page, §4, not this one). These need a perf-VM run and a device before budgets are set.

To reproduce: `cd apps/web && npx vite build --config bench/ui-tree/vite.config.ts && node bench/ui-tree/run.mjs`.

## 4. What needs a sealed frame, and what it costs

A sealed frame is for what the catalogue cannot express: a custom drawing surface (canvas, WebGL, a map, a game), a third-party editor, an HTML layout with its own scrolling and typography that no composition of host components reproduces, or a legacy web page. Anything that is a list, a form, a chart already in the catalogue, text, a toolbar, a menu or a layout is **not** a sealed-frame case; if it feels like one, the catalogue is missing a primitive (add it, do not seal it). The honest list of what the current app has that a plugin author could not rebuild from §1: the block editor (it is a catalogue component), the canvas strip and window gestures (catalogue components), the glass material (host), and nothing else. Plugin-specific drawing (a diagram editor, a flame graph, a whiteboard) is the real sealed-frame use.

Measured with `apps/web/bench/ui-tree/public/frame.html` and `run-frame.mjs`: the same UI (a text field, 100 rows, a streaming message, a glass probe) mounted in the page and in `<iframe sandbox="allow-scripts" srcdoc>` (opaque origin, no `allow-same-origin`), dev-02, Chromium; and the same page in the iPhone 18 Pro Simulator (iOS 27, Safari) on std.

| Cost | In the page | Sealed frame |
| --- | --- | --- |
| Mount, time until painted (1x) | 5 ms (1 UI), 8 (5), 14 (20) | **40 ms** (1), 58 (5), 97 (20), including the frame's own document and script |
| Mount at 4x | 21, 30, 54 ms | **97, 125, 388 ms** |
| Heap in the main process, per UI | about 120 KB first, then close to 0 | **400 to 580 KB each** (the frame's own JS realm and DOM; the frame is same-process here, a site-isolated browser would add a process) |
| DOM nodes | 311 | 325 (inside the frame) |
| Streaming into 5 UIs at 60 Hz, 3 s, frame time | p99 16.8, 0 frames over 25 ms (1x and 4x) | the same; main-thread busy 120 vs 145 ms/s at 1x, 499 vs 504 at 4x. **Once running, a frame costs no more than the page.** |
| Fonts | inherited | **not inherited.** A frame with an opaque origin that loads a font from the host is blocked by CORS (`Access-Control-Allow-Origin` absent: Chromium refuses, `document.fonts.load` rejects, the text falls back to the system font). The host must serve fonts with `Access-Control-Allow-Origin: *` (they are public files) or hand the bytes over by message. With CORS: font usable 10 to 12 ms after mount for one frame, 70 to 85 ms for twenty (1x), 70 ms for one at 4x, 380 for twenty. |
| Focus | native order | Tab from a host button lands **on the `<iframe>` element**, then inside it; the frame owns its internal order and must report boundary movement; the host cannot see the focused element, so focus restoration after a sheet closes needs the frame's cooperation. |
| Shortcuts | host bound | **keystrokes typed inside the frame are invisible to the page** (0 of 2 seen). A global shortcut needs the frame to forward the key by message, which the host then validates. Frames can never be on the path of Cmd-K, Esc or app navigation without that cooperation. |
| Material | the adaptive material | **A transparent frame can still blur the host's pixels behind it**: a `backdrop-filter: blur(20px)` element inside the frame blurred the host's red and blue stripes in Chromium (0 sharp stripe edges left across the probe, the same as the in-page probe), and in iOS 27 Safari the Simulator screenshot shows the same smear across the stripes (attached to #15: `sim-frame-inpage.png`, `sim-frame-sealed.png`). The proposal's "no backdrop sampling" therefore cannot be a browser guarantee; it must be a host rule: mount frames only inside a host-drawn `Card` that supplies the surface, never over chrome, and treat frame-drawn blur as the plugin's own business inside its rect. The frame also cannot read the host's tokens: they must be sent (a versioned snapshot) and re-sent on a scheme or type-scale change; fonts as above. |

Rules this fixes: on-demand mounting and disposal of hidden frames (the 20-frame mount costs 100 to 390 ms: never mount frames in a list or a transcript, which stays a generated view, as the proposal says); a mount cap per window; fonts served with CORS; keys and focus via the host's explicit routing; a frame inside a `Window` or `Card` only. Not measured: a frame under site isolation, memory on a device, first-interaction latency after mount, a frame with its own heavy canvas.

## 5. Migration order: never two implementations

The rule: **a component is replaced in the same merge that deletes its predecessor**, and the app works at the end of every merge (the e2e gate is green, 42 suites today). There is no flag that keeps the old and new path alive. Each step lists the component, what it is replaced by, and why the existing app keeps working.

0. **A live renderer from GenUI's.** `genui/GenNode.svelte` already renders a normalised tree. Add (a) a patch applier (the bench's store, 40 lines), (b) a catalogue manifest for the *U* components of §1 generated from their Svelte props, so a catalogue entry is the component's own signature rather than a second description (one place per fact). Nothing in the app calls it yet. Gate: unit tests for patch validation.
1. **Skills and Automations pages (new, no predecessor).** Express the two pages and the Settings plugin pages as trees (`SettingsPage`, `List`, `Form`, `Button`) produced in-process by official TypeScript first. They have no legacy, so nothing is kept in step; they prove the renderer on a real surface and the guided forms (`Wizard`). The current Svelte panels (`SkillsPanel`, `AutomationsPanel`) are deleted in the same merge.
2. **Leaf swaps in place.** Menus, pills, capsules, rows and the toast become catalogue-addressable by *registering what already exists*: no behaviour changes, so there is no second implementation. The hand-written calls (`<Menu items=…>`) and the tree path call the same component.
3. **The shell's layout.** `Shell.svelte` becomes a `Layout` renderer: its template turns into the §2.1 tree (written in TypeScript in the official shell plugin, in-process), and its behaviours (pull, resize, settle, history entries, veil and material) become the host props of `Region`. The old template is deleted in that merge. The gate's shell suites (seam, flat-sidebars, sidebar-pull, phone-nav, safe-areas, shell-audit, app-switcher) are the proof that nothing moved.
4. **The canvas as the second layout tree.** `Strip` and `Window` replace `canvas/Canvas.svelte` and `WindowFrame.svelte` as catalogue components; the canvas layout (§2.2) is a second tree the same renderer draws. Selecting between the two is the plugin's choice (a setting), not two code paths. The Threads lane's experiment keeps working because the components are theirs, re-registered. Do not start this until the owner has decided the canvas shape (in progress, 2026-11-18).
5. **App sidebars and panels.** Threads' and Notes' sidebars become `Sidebar` sections (rows, groups, inbox, top entries) fed by trees; the swipe rows, reorder and paging are host. One app at a time, Notes first (smaller), the list component being the same `VirtualList` in both.
6. **Threads' transcript and composer last.** `Transcript` and `Composer` enter as one catalogue name each (§1.4). They are the heaviest host code (virtual list, streaming, editor, queue); the plugin sends pages, appended text and chips and receives `sent`, `action`, `chip`. They are not decomposed into trees of their parts: that would be a second implementation of a hot path.
7. **The worker boundary.** Move the official plugins' tree producers from the main thread into workers (the same code, `postMessage` instead of a call), then enable community plugins behind the isolation checks of lane #50. By §3 it is a transport change.
8. **Sealed frames** after step 7, behind the host rules of §4.

Open questions for the owner before step 3: whether the shell plugin may declare *new* slots (the proposal says yes, plugin-owned slots are a sequence item); whether a theme may change the radius scale's values or only choose among presets (§2.3 assumes presets); the canvas shape.

## 6. Simplification opportunities

- Two components named `Card` (content box in GenUI, the middle card in `packages/ui`): rename one before either is in the catalogue (`Surface`/`Window` for the layout one).
- `Menu`, `Popover` and `Sheet` already encode one rule (dropdown unless search or long); the catalogue should expose `Menu` only and keep the rule out of plugin hands.
- The onboarding stepper and the skills `Wizard` are two steppers; one `Steps` flow component would serve both and the catalogue.
- `GenUI`'s `Choice` is rendered by `Selector`; `Selector`, `Capsule` and `PillRow` overlap and could share one adaptive-row primitive.
