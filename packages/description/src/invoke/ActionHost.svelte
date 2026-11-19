<script lang="ts">
  // Draws the dialog an activation needs: a prompt (text, date, datetime), the picker over the host's option labels, and the PROTECTED confirmation, which shows exactly the material the host wrote (name, label, value) and
  // nothing the description supplied. Typing is never consent: only the Confirm button confirms. One dialog at a time; cancel and Escape answer null/false.
  import { Button, Sheet } from "@calternal/ui";
  import type { ActionUi } from "./actions.svelte";
  let { ui }: { ui: ActionUi } = $props();
  const d = $derived(ui.dialog);
  let text = $state("");
  $effect(() => { if (d?.kind === "prompt") text = d.value; });
  const show = (v: unknown): string => (typeof v === "string" ? v : JSON.stringify(v));
</script>

{#if d}
  <Sheet open label={d.title} onclose={() => { if (d.kind === "confirm") d.done(false); else d.done(null); }} side="center">
    <div class="dlg" data-dialog={d.kind}>
      <h2 class="t">{d.title}</h2>
      {#if d.kind === "prompt"}
        <form onsubmit={(e) => { e.preventDefault(); d.done(text); }}>
          <input class="in" type={d.input === "date" ? "date" : d.input === "datetime" ? "datetime-local" : "text"} placeholder={d.placeholder} bind:value={text} aria-label={d.title} />
          <div class="row"><Button text="Cancel" variant="quiet" onclick={() => d.done(null)} /><Button text="OK" variant="primary" type="submit" /></div>
        </form>
      {:else if d.kind === "choose"}
        <ul class="opts" role="listbox" aria-label={d.title}>{#each d.options as o (o.ref)}<li><button type="button" role="option" aria-selected="false" onclick={() => d.done(o.ref)}>{o.label}</button></li>{/each}</ul>
        <div class="row"><Button text="Cancel" variant="quiet" onclick={() => d.done(null)} /></div>
      {:else}
        <dl class="mat" data-material>{#each d.material as m (m.name)}<dt>{m.label}</dt><dd>{show(m.value)}</dd>{/each}</dl>
        <div class="row"><Button text="Cancel" variant="quiet" onclick={() => d.done(false)} /><Button text="Confirm" variant="primary" onclick={() => d.done(true)} /></div>
      {/if}
    </div>
  </Sheet>
{/if}

<style>
  .dlg { display: grid; gap: var(--s-4); padding: var(--s-6); min-width: min(26rem, 90vw); }
  .t { margin: 0; font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .in { width: 100%; min-height: 44px; padding: 0 var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 8%, transparent); color: var(--ink); border: 0; font-size: 16px; }
  .row { display: flex; justify-content: flex-end; gap: var(--s-3); margin-top: var(--s-3); }
  .opts { list-style: none; margin: 0; padding: 0; display: grid; max-height: 50vh; overflow: auto; }
  .opts button { width: 100%; min-height: 44px; text-align: start; padding: 0 var(--s-4); border-radius: var(--r-select); color: var(--ink); }
  .opts button:hover, .opts button:focus-visible { background: var(--hover); }
  .mat { display: grid; grid-template-columns: auto 1fr; gap: var(--s-2) var(--s-4); margin: 0; }
  .mat dt { color: var(--ink-3); } .mat dd { margin: 0; overflow-wrap: anywhere; }
</style>
