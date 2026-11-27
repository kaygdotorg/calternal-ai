<script lang="ts">
  // The checkbox of a task. A circle that fills with the accent and draws its tick when ticked. ORDER: the box fills (`--dur-fast`, scale 0.88 to 1 on `--ease-out`), the tick draws as the fill lands (`--dur-base`, `--ease-out`), and the title's strike (in TaskRow) draws a beat later.
  // Interruptible: all of it is CSS transitions on the state, so a second press reverses from where it is. Reduced motion: the tokens are 1 ms, so it is instant. The hit area is the row's height (32 on a desktop, 44 on touch), the drawn circle is the glyph size.
  import { arr } from "../../arrange.svelte";
  let { id, label, status }: { id: string; label: string; status: string | undefined } = $props();
  const done = $derived(status === "Done");
  const toggle = (e: Event) => { e.stopPropagation(); arr.setStatus(id, done ? "To do" : "Done"); };
</script>

<button type="button" class="ck" role="checkbox" aria-checked={done} aria-label={`Done: ${label}`} onclick={toggle}>
  <span class="box" class:done><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 12.5l4 4 8-9" pathLength="1" /></svg></span>
</button>

<style>
  .ck { flex: none; display: grid; place-items: center; width: var(--lead); min-height: var(--row-h); margin-inline: 0; padding: 0; border-radius: var(--r-select); color: #fff; }
  .box { display: grid; place-items: center; width: var(--glyph); height: var(--glyph); box-sizing: border-box; border-radius: 50%; border: 2px solid var(--ink-4); transition: background-color var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .ck:hover .box:not(.done) { border-color: var(--ink-3); background: var(--hover); } }
  .ck:active .box { scale: 0.88; }
  .box.done { background: var(--badge-bg); border-color: var(--badge-bg); }
  svg { width: 72%; height: 72%; stroke-dasharray: 1; stroke-dashoffset: 1; transition: stroke-dashoffset var(--dur-base) var(--ease-out); }
  .done svg { stroke-dashoffset: 0; }
</style>
