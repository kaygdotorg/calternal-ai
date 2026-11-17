<script lang="ts">
  // A folded group of tool calls ("Worked for 1 min · 6 tools"): quiet row that opens in place into the steps; each step opens into its
  // command and output. Folded by default so a long thread reads as a conversation, not a log.
  import { untrack } from "svelte";
  import { Icon, type IconName } from "@calternal/ui";
  import LogView from "./LogView.svelte";
  import type { Step, StepKind } from "./model";
  let { steps, ms, running = false }: { steps: Step[]; ms: number; running?: boolean } = $props();
  const ICON: Record<StepKind, IconName> = { read: "file", edit: "code", run: "terminal", search: "search", spawn: "branch" };
  let open = $state(untrack(() => running));
  let openStep = $state<string | null>(null);
  const kinds = $derived([...new Set(steps.map((s) => s.kind))].slice(0, 3));
  const secs = $derived(Math.round(ms / 1000));
  const worked = $derived(secs >= 60 ? `${Math.round(secs / 60)} min` : `${secs} s`);
  const uid = $props.id();
</script>

<div class="steps" class:open>
  <button type="button" class="head" aria-expanded={open} aria-controls="{uid}-list" onclick={() => (open = !open)}>
    <span class="chips" aria-hidden="true">{#each kinds as k, i}<span class="chip" style:--i={i}><Icon name={ICON[k]} inline /></span>{/each}</span>
    <span class="sum" class:shimmer={running}>{running ? "Working" : `Worked for ${worked}`} · {steps.length} {steps.length === 1 ? "tool" : "tools"}</span>
    <span class="chev"><Icon name="chevron" inline /></span>
  </button>
  <div class="list" id="{uid}-list" inert={!open}>
    <ol>
      {#each steps as s (s.id)}
        <li>
          <button type="button" class="step" aria-expanded={openStep === s.id} onclick={() => (openStep = openStep === s.id ? null : s.id)}>
            <span class="ic"><Icon name={ICON[s.kind]} inline /></span>
            <span class="t">{s.title}</span><code>{s.detail}</code>
            <span class="ms">{s.status === "failed" ? "failed" : `${(s.ms / 1000).toFixed(1)} s`}</span>
          </button>
          {#if openStep === s.id && s.output}<div class="out"><LogView text={s.output} label="Output of {s.title} {s.detail ?? ''}" /></div>{/if}
        </li>
      {/each}
    </ol>
  </div>
</div>

<style>
  .steps { display: grid; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); overflow: clip; }
  .head { border-radius: var(--r-select); display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: calc(var(--row-h) + var(--s-3)); padding: 0 var(--s-5) 0 var(--s-4); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); text-align: start; }
  .chips { display: flex; }
  .chip { width: var(--s-7); height: var(--s-7); margin-left: calc(var(--s-2) * -1); display: grid; place-items: center; border-radius: 50%; background: var(--paper); box-shadow: 0 0 0 var(--s-1) rgb(var(--tint) / 0.9), inset 0 0 0 1px var(--line); color: var(--ink-3); z-index: calc(3 - var(--i)); position: relative; }
  .chip:first-child { margin-left: 0; }
  .sum { flex: 1; font-variant-numeric: tabular-nums; }
  .shimmer { background: linear-gradient(100deg, var(--ink-3) 40%, var(--ink) 50%, var(--ink-3) 60%) 0 0 / 250% 100% no-repeat; -webkit-background-clip: text; background-clip: text; -webkit-text-fill-color: transparent; animation: sweep 1.9s linear infinite; }
  @keyframes sweep { to { background-position: 100% 0; } }
  .chev { display: grid; transition: rotate var(--dur-base) var(--ease-out); }
  .open .chev { rotate: 90deg; }
  .list { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--dur-base) var(--ease-out); }
  .open .list { grid-template-rows: 1fr; padding-bottom: var(--s-2); }
  ol { min-height: 0; overflow: clip; display: grid; }
  .step { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5) 0 var(--s-4); text-align: start; font-size: var(--text-label); color: var(--ink-2); }
  .ic { display: grid; width: var(--s-7); place-items: center; color: var(--ink-3); }
  .t { font-weight: var(--strong-weight); flex: none; }
  code { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-3); }
  .ms { flex: none; color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .out { padding: var(--s-1) var(--s-4) var(--s-4); }
  @media (hover: hover) and (pointer: fine) { .step:hover, .head:hover { background: var(--hover); } }
  @media (prefers-reduced-motion: reduce) { .shimmer { animation: none; -webkit-text-fill-color: currentColor; } .list, .chev { transition: none; } }
</style>
