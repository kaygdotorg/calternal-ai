<script module lang="ts">
  // Read when the module loads, before the appearance store writes its first save.
  const fresh = ((): boolean => { try { return !localStorage.getItem("cal.firstrun") && !localStorage.getItem("cal.appearance"); } catch { return false; } })();
</script>

<script lang="ts">
  // The first-run sheet: a short gallery of presets, then straight in. Shown once per browser (the choice, or Skip, ends it); the same gallery stays in
  // Appearance settings.
  import { Button, Sheet } from "@calternal/ui";
  import { shell } from "../shell/shell.svelte";
  import PresetGallery from "./PresetGallery.svelte";
  const KEY = "cal.firstrun";
  let open = $state(fresh);
  function done() {
    open = false;
    try { localStorage.setItem(KEY, "1"); } catch { /* private mode: asked again next time */ }
  }
</script>

<Sheet {open} label="Choose a look" onclose={done} side={shell.narrow ? "bottom" : "right"}>
  <div class="in">
    <h1>Choose a look</h1>
    <p class="note">A preset sets the backdrop and the colours together. You can change any part later in Appearance.</p>
    <PresetGallery onpick={() => done()} />
    <footer><Button variant="quiet" text="Keep the default" onclick={done} /></footer>
  </div>
</Sheet>

<style>
  .in { display: grid; gap: var(--s-5); padding: var(--s-6); }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .note { font-size: var(--text-label); color: var(--ink-3); }
  footer { display: flex; justify-content: flex-end; }
</style>
