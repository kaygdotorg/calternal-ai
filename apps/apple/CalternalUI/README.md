# CalternalUI

Calternal's native design system for macOS 26+ and iOS 27+: SwiftUI, no third-party dependencies. Issue #24, lane `apple-design-system`.
The web system is `packages/ui` (`docs/ui-design-system.md`); decisions are in `docs/ui-shell.md` and `docs/settings.md`; measured native values in
`docs/native-ui-study.md` and `.json`.

## Layout

| Path | What |
| --- | --- |
| `Sources/CalternalUI/Generated` | `Tokens.generated.swift`, written from `packages/ui/tokens/tokens.json` and the Dynamic Type table in `docs/native-ui-study.json`. Never edited. |
| `Sources/TokenGenerator`, `Sources/generate-tokens` | The generator (Foundation only, not linked into the library). An unknown token fails it, so a web-side addition cannot be missed. |
| `Sources/CalternalUI/Foundation` | `Metrics` (tokens times the UI size), `Theme` (colours, accent), `TextRole` fonts, `MotionPolicy`, `@Design` (all of it from the environment). |
| `Sources/CalternalUI/Materials` | `MaterialSettings` (Blur, Dim, Tint), `VeilSolver` (port of `veil.ts`), `Wallpaper` (blurred once, grids read once, accent inferred). |
| `Sources/CalternalUI/Interaction` | `InputModality`, the one press style (hover, press, bounce, keyboard-only ring), the optional interaction probe. |
| `Sources/CalternalUI/Surfaces` | `Pill`, `IconButton`, `ActionButton`, `SegmentedCapsule`, `BottomTabCapsule`, `ToolbarPill`, `Card`, `calternalSheet`, `Row`, `Badge`, `AttentionCard`, `RequestBar`, `Icon`. |
| `Sources/CalternalUI/Shell` | `ShellView` (Mac and phone), sidebars, thread view, `WallpaperScene`, `FrameMeter`, fixtures. |
| `Sources/CalternalUI/GenUI` | `GenUIView` renderer, the model for the evaluator's output, fixtures. |
| `Preview` | The preview app (macOS and iOS) and its UI tests (`CalternalPreview.xcodeproj`). |

## Commands (on `kayg@std`, in `~/Developer/lanes/apple-design-system/apps/apple/CalternalUI`; Xcode 27.2 directly, no `nix develop`)

```
scripts/tokens.sh            # regenerate the Swift tokens   (scripts/tokens.sh --check fails when stale)
scripts/check-docs.sh        # every public item documented
swift test                   # 28 unit tests: tokens vs the native study, metrics, veil, wallpaper, GenUI, interaction rules
cd Preview
xcodebuild test -project CalternalPreview.xcodeproj -scheme CalternalPreview -destination 'platform=macOS'
xcodebuild test -project CalternalPreview.xcodeproj -scheme CalternalPreview -destination 'platform=iOS Simulator,name=<device>'
```

Preview app launch arguments: `-scene shell|long|list|gallery|genui|lab`, `-appearance light|dark`, `-materials blur,dim,tint|none`, `-uiSize 1.2`,
`-wallpaper dune|aurora|ember`, `-reduceMotion 1`, `-open sidebar|right[,changes]`, `-rows N`.

## How the pieces map

- **Tokens.** One source. Lengths, durations, curves, weights, light and dark colours, shadows and the 3 by 3 mesh come from `tokens.json`; the phone's type
  scale from the measured iOS Dynamic Type table, as on the web. `Metrics` multiplies by the UI size exactly as `build-tokens.mjs` does (the same `SCALED` set;
  blur radii, durations and the pill radius are not scaled; glyphs scale up to 1.5). On touch, `.textRole` sizes follow Dynamic Type (`ScaledMetric`).
- **Materials.** Small floating surfaces (`Pill`, `SegmentedCapsule`, `ToolbarPill`, composer, request bar, attention card) use the system's Liquid Glass.
  Blur off draws the web's flat 6.5 % ink fill; Tint adds the scheme's tint at 0.3 over the glass (the same 0.3 as the web's glass edge); Dim adds the scheme's
  scrim token under it; Reduce Transparency is opaque paper. Large surfaces use no live blur: the sidebars and the card show the wallpaper's pre-blurred copy
  under the **solved veil** (Tint then Dim, per surface), exactly the web design (`docs/ui-craft-and-settings-study.md`).
- **Interaction.** `InteractionRules` decides the ring (focused, and on a Mac the last input was a key) and animation (never from the keyboard, never under
  Reduce Motion or the app's motion switch). Press goes to 0.96 over `dur-fast`; release rebounds to 1.035 and settles over `dur-slow` on the ease-out curve;
  rows press in without a rebound. Every interactive initialiser takes the accessibility label as a required argument (`label:`, `title:` or `text:`).
- **Generated UI.** `GenSnapshot(json:)` decodes the evaluator's `Evaluation`; `GenUIView` draws the 21 catalog entries the web renderer draws, with native
  controls (`Toggle`, `Menu`, `TextField`), and a fallback line for the others. A test keeps the list equal to `apps/web/src/genui/GenNode.svelte`.

## Measurements (development machine: Mac Studio `kayg@std`, 120 Hz display, Xcode 27.2; not benchmark numbers)

MEASUREMENTS

## Matches the web design system exactly

Every token value (spacing, radii, durations, curves, spring, colours in both schemes, shadows, backdrop mesh, row, bar, pill, glyph, text sizes per form,
sheet motion per form, bottom capsule 48 high and 28 in), the UI-size scaling rule, the veil solver (same constants, checked against `veil.ts` by a test), the
accent mix and inference, the interaction rules, the Blur, Dim and Tint semantics, the catalog the generated-UI renderer draws, the shell's regions and the
phone's slide-over structure (56 pt peek, 0.96 scale under the right sidebar, bottom capsule hidden while the sidebar is open).

## Differs because the platform differs

- **Font.** The system font (SF) with weights mapped by `fontWeight`; the web uses Google Sans Flex everywhere. `Tokens.Family` carries the names for a font setting. Rounded design is used for the wordmark.
- **Icons.** SF Symbols by name (`IconName`), not the web's stroke set; weights are tuned to a 1.4 pt line and not measured (the study lists stroke weights as not measured).
- **Glass.** The system's `glassEffect` draws the blur, edge and shadow of small surfaces; the web's `glass-edge` token has no native use and is listed as web-only in the generator.
- **Sheets, menus, switches, selects.** The system's `.sheet`, `Menu`, `Toggle`, with the design's material and accent applied; the web builds them.
- **Focus ring.** 2 pt inset accent, as the web; the system's own ring is 3 pt `#63ACFC` (study) and is disabled on our controls.
- **Dynamic Type** drives the phone's text, the web's phone scale is fixed at "Large".
- **Wordmark and traffic lights.** On a Mac the wordmark clears the lights (64 pt) in a full-size-content window.

## What the native apps need next

1. The live subscription (`/api/v1/live`, bookmark and resume) and thread operations through `CalternalCore`; `ShellFixture` is the seam to replace.
2. The `crates/genui` evaluator exposed through the bindings, so `GenUIView` receives real `Evaluation`s and `GenUIHost.set` re-evaluates instead of `LocalGenHost`.
3. Wallpaper choice (own picture, built-ins, Unsplash) feeding `Wallpaper`, fonts per role, the wordmark setting, UI size and Blur/Dim/Tint stored as settings.
4. The palette, shortcuts sheet, context menus, drag between sections and rename, text selection actions (Comment, Ask in Side Chat), edge fade per row.
5. Screen-reader and keyboard pass on a real device; Dynamic Type at the accessibility sizes; the animation fluidity check on a real device (below).

## Open

- Font: ship Google Sans Flex natively, or keep the system font on Apple platforms (the doc says "no system font" for the web, the native study measured SF).
- Icons: SF Symbols, or port the web's stroke set (needs an SVG path parser with arcs; about 150 lines).
- Veil floors live in `veil.ts` only; the Swift copy is guarded by a test. Moving them into `tokens.json` would make one source (touches the web build).
- Wordmark placement beside the traffic lights at 240 pt wide is tight; options: widen the sidebar, move the wordmark below the lights, or hide the lights' row.
- UI-test keyboard Tab focus needs full keyboard access, a machine setting the tests must not change; that path is covered by `InteractionRules` unit tests, and the pointer paths by UI tests.

## Simplification opportunities

- `tokens.json` carries web-only keys (`font-*` stacks, `safe-*`, `glass-edge`, `text-opsz`, `text-grad`, `msg-w`) that the generator lists with reasons; splitting them into a `web` group would let both generators fail on any unknown key.
- `veil.ts` and `VeilSolver` are the same 150 lines twice; a Rust implementation in `crates/client` behind UniFFI would give web (no Wasm, so not there) and native one copy; until then the parity test is the guard.
- Three places spell the component catalog (`catalog-data.json`, `GenNode.svelte`, `GenUIView.supportedTypes`); the test reads the other two, but a catalog flag `drawnBy: [web, native]` would be one place.
