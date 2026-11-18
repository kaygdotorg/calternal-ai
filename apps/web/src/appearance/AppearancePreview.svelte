<script lang="ts">
  // The live preview beside Appearance: a small real render of the app (a sidebar with its rows and the app capsule, the middle card with a message from the person, one from the agent with its provider mark and a
  // code block, a choice and a switch, the composer) built from the shell's own components and drawn with the live CSS variables, so every change (scheme, preset, accent, materials, wallpaper, fonts, UI size, window look)
  // shows at once with nothing extra to compute: it listens to nothing, it only reads the same variables. Read-only (`inert`, hidden from assistive technology) and fixture content only. Cost: one render of about forty elements,
  // and it never re-renders for a setting (the variables do the work).
  import { Card, Icon, Selector, Switch } from "@calternal/ui";
  import { canvasPrefs } from "../canvas/prefs.svelte";
  const variant = $derived(canvasPrefs.look === "card" ? "pane" : canvasPrefs.look === "clear" ? "clear" : "solid");
</script>

<div class="pv" aria-hidden="true" inert>
  <div class="side">
    <p class="mark">Calternal</p>
    <p class="row"><Icon name="plus" inline /> New thread</p>
    <p class="row on"><Icon name="threads" inline /> Fix the flaky sign-in</p>
    <p class="row"><Icon name="threads" inline /> Draft the release notes</p>
    <p class="row"><Icon name="threads" inline /> Review the schema change</p>
    <div class="cap"><Icon name="threads" /><Icon name="file" /><Icon name="settings" /></div>
  </div>
  <Card label="Preview" {variant} class="mid">
    <div class="msgs">
      <p class="user">Why does the passkey sign-in flake on the dev VM?</p>
      <div class="agent"><Icon name="claude" /><div><p>Two things are going on, and only one of them is a bug. The <code>catalog()</code> call rebuilt its map on every request.</p><pre>let tail = journal.read_tail(max)?;
verify_chain(&amp;tail)?;</pre></div></div>
      <div class="ctl"><Selector label="Preview choice" options={[{ value: "a", label: "Wide" }, { value: "b", label: "Centred" }]} value="a" onchange={() => {}} /><Switch label="Notify me" checked onchange={() => {}} /></div>
    </div>
    <p class="comp">Message Claude…<span class="send"><Icon name="up" inline /></span></p>
  </Card>
</div>

<style>
  .pv { display: grid; grid-template-columns: 7.5rem minmax(0, 1fr); gap: var(--s-3); padding: var(--s-3); border-radius: var(--r-pane); height: 28rem; overflow: hidden; pointer-events: none; user-select: none; background: rgb(var(--ink-rgb) / 0.05); box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); }
  .side { display: grid; align-content: start; gap: var(--s-2); min-width: 0; position: relative; }
  .mark { font-family: var(--font-brand); font-size: var(--text-heading); padding: 0 var(--s-2); }
  .row { display: flex; align-items: center; gap: var(--s-2); padding: var(--s-2); border-radius: var(--r-select); color: var(--ink-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row.on { background: var(--accent-soft); color: var(--accent); }
  .cap { position: absolute; left: 0; bottom: 0; display: flex; gap: var(--s-3); padding: var(--s-3) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-2); }
  .pv :global(.mid) { min-width: 0; display: grid; grid-template-rows: minmax(0, 1fr) auto; padding: var(--s-3); gap: var(--s-3); overflow: hidden; }
  .msgs { display: grid; align-content: start; gap: var(--s-3); min-height: 0; overflow: hidden; }
  .user { justify-self: end; max-width: 85%; padding: var(--s-3) var(--s-4); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); }
  .agent { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--s-3); color: var(--ink); font-family: var(--font-chat, var(--font-ui)); }
  .agent code { font-family: var(--font-mono); font-size: var(--text-code); background: rgb(var(--ink-rgb) / 0.07); padding: 0 var(--s-2); border-radius: var(--r-select); }
  .agent pre { margin: var(--s-2) 0 0; padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); font-family: var(--font-mono); font-size: var(--text-code); overflow: hidden; }
  .ctl { display: grid; gap: var(--s-3); }
  .comp { display: flex; align-items: center; justify-content: space-between; padding: var(--s-3) var(--s-3) var(--s-3) var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-3); }
  .send { display: grid; place-items: center; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; background: var(--accent-base); color: #fff; }
</style>
