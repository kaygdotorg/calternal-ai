<script lang="ts">
  // "Pick a date and time" for a snooze: one date-time field and Snooze. Opened from the Inbox row's menu and the thread's menu (snooze.svelte.ts); the presets are in the menus.
  import { Button, Sheet } from "@calternal/ui";
  import { backLabel, snoozeChoices } from "./snooze";
  import { snoozeAsk, snoozeThread } from "./snooze.svelte";
  const local = (t: number) => { const d = new Date(t - new Date(t).getTimezoneOffset() * 60_000); return d.toISOString().slice(0, 16); };
  let value = $state("");
  $effect(() => { if (snoozeAsk.id) value = local(Date.now() + 24 * 3600_000 - (Date.now() % 900_000)); });
  const at = $derived(value ? new Date(value).getTime() : NaN);
  const valid = $derived(Number.isFinite(at) && at > Date.now());
  const choices = $derived(snoozeAsk.id ? snoozeChoices(Date.now()) : []);
  const close = () => (snoozeAsk.id = null);
  const preset = (c: { at: number }) => { if (snoozeAsk.id) snoozeThread(snoozeAsk.id, c.at, backLabel(c.at, Date.now())); close(); };
  function go() { if (!valid || !snoozeAsk.id) return; snoozeThread(snoozeAsk.id, at, backLabel(at, Date.now())); close(); }
</script>

<Sheet open={!!snoozeAsk.id} label="Snooze until" side="center" onclose={close}>
  <form class="pick" onsubmit={(e) => { e.preventDefault(); go(); }}>
    <h2>Snooze until</h2>
    <div class="presets">{#each choices as c (c.id)}<button type="button" class="pre" onclick={() => preset(c)}>{c.label}</button>{/each}</div>
    <input type="datetime-local" bind:value min={local(Date.now())} aria-label="Date and time" />
    <p class="note" role="status">{valid ? backLabel(at, Date.now()) : "Pick a time in the future."}</p>
    <div class="row"><Button variant="quiet" text="Cancel" onclick={close} /><Button variant="primary" text="Snooze" disabled={!valid} onclick={go} /></div>
  </form>
</Sheet>

<style>
  .pick { display: grid; gap: var(--s-4); padding: var(--s-6); min-width: min(20rem, 80vw); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  input { height: var(--pill-h); padding: 0 var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); }
  .presets { display: grid; gap: var(--s-1); }
  .pre { display: flex; align-items: center; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; }
  @media (hover: hover) and (pointer: fine) { .pre:hover { background: var(--hover); } }
  .note { color: var(--ink-3); font-size: var(--text-label); }
  .row { display: flex; justify-content: flex-end; gap: var(--s-2); }
</style>
