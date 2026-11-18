<script module lang="ts">
  // Read when the module loads, before the appearance store writes its first save.
  const fresh = ((): boolean => { try { return !localStorage.getItem("cal.firstrun") && !localStorage.getItem("cal.appearance"); } catch { return false; } })();
</script>

<script lang="ts">
  // The first-run sheet: a short gallery of presets, then straight in. Shown once per browser (the choice, or Skip, ends it); the same gallery stays in
  // Appearance settings.
  import { Button, Sheet } from "@calternal/ui";
  import { route, setQuery } from "../lib/route.svelte";
  import { shell } from "../shell/shell.svelte";
  import PresetGallery from "./PresetGallery.svelte";
  const KEY = "cal.firstrun";
  // Shown on a fresh browser, or on demand with ?firstrun=1 (so it can be looked at and tested); ?firstrun=0 skips it for good (a device that cannot tap, a test).
  let open = $state(fresh);
  $effect(() => {
    const q = route.query.get("firstrun");
    if (q === "1") open = true;
    else if (q === "0") { open = false; try { localStorage.setItem(KEY, "1"); } catch { /* none */ } }
  });
  function done() {
    open = false;
    if (route.query.has("firstrun")) setQuery("firstrun", null);
    try { localStorage.setItem(KEY, "1"); } catch { /* private mode: asked again next time */ }
  }
</script>

<Sheet {open} label="Choose a look" onclose={done} side={shell.narrow ? "bottom" : "center"}>
  <div class="in">
    <h1>Choose a look</h1>
    <p class="note">A preset sets the backdrop and the colours together. You can change any part later in Appearance.</p>
    <div class="gal"><PresetGallery onpick={() => done()} /></div>
    <footer><Button variant="quiet" text="Keep the default" onclick={done} /></footer>
  </div>
</Sheet>

<style>
  /* The gallery scrolls inside the sheet; the title and "Keep the default" stay in view (on a phone the sheet is as tall as the screen allows, and a button scrolled to the very bottom sat under the rounded corner). */
  .in { display: grid; grid-template-rows: auto auto minmax(0, 1fr) auto; gap: var(--s-5); padding: var(--s-6) var(--s-6) var(--s-8); max-height: min(88dvh, 52rem); box-sizing: border-box; }
  .gal { min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  footer { display: flex; justify-content: flex-end; }
</style>
