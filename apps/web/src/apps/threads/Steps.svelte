<script lang="ts">
  // A folded group of tool calls ("Worked for 1 min · 6 tools"; "Worked · 6 tools" when the time is not known): quiet row that opens in place into the steps; each step opens into its
  // command and output. Folded by default so a long thread reads as a conversation, not a log. While the turn runs (`running`) it is open and says only how many tools so far: the working row beside it says what is happening.
  import { untrack } from "svelte";
  import { Icon, type IconName } from "@calternal/ui";
  import LogView from "./LogView.svelte";
  import type { Step, StepKind } from "./model";
  let { steps, ms, running = false }: { steps: Step[]; ms: number; running?: boolean } = $props();
  const ICON: Record<StepKind, IconName> = { read: "file", edit: "code", run: "terminal", search: "search", spawn: "branch" };
  let open = $state(untrack(() => running));
  let openStep = $state<string | null>(null);
  const kinds = $derived([...new Set(steps.map((s) => s.kind))].slice(0, 3));
  /** The pictures the steps produced or read: when there are any, the deck in the folded row is their thumbnails (up to three) instead of the tools' icons. */
  const thumbs = $derived(steps.flatMap((s) => (s.files ?? []).filter((f) => f.kind === "image")).slice(0, 3));
  const secs = $derived(Math.round(ms / 1000));
  const worked = $derived(secs >= 60 ? `${Math.round(secs / 60)} min` : `${secs} s`);
  const uid = $props.id();
</script>

<div class="steps" class:open>
  <button type="button" class="head" aria-expanded={open} aria-controls="{uid}-list" onclick={() => (open = !open)}>
    <!-- The deck: the tools' icons (or the pictures' thumbnails) overlap in a small stack; pointing at the row fans them out (pointer only, on the motion tokens); a press opens the block. -->
    <span class="chips" class:thumbs={thumbs.length > 0} aria-hidden="true">{#if thumbs.length}{#each thumbs as f, i}<span class="chip pic" style:--i={i}><img src={f.url} alt="" loading="lazy" /></span>{/each}{:else}{#each kinds as k, i}<span class="chip" style:--i={i}><Icon name={ICON[k]} inline /></span>{/each}{/if}</span>
    <span class="sum">{running ? "" : ms > 0 ? `Worked for ${worked} · ` : "Worked · "}{steps.length} {steps.length === 1 ? "tool" : "tools"}</span>
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
          {#if s.files?.length}
            <div class="files">
              {#each s.files as f (f.url)}
                <a class="file" class:image={f.kind === "image"} href={f.url} target="_blank" rel="noopener noreferrer" aria-label="Open {f.name}">
                  {#if f.kind === "image"}<img src={f.url} alt="" loading="lazy" />{:else}<Icon name="file" inline />{/if}<span class="fn">{f.name}</span>
                </a>
              {/each}
            </div>
          {/if}
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
  .chip { transition: margin-left var(--dur-base) var(--ease-out); }
  .chip.pic { overflow: hidden; border-radius: var(--r-select); }
  .chip.pic img { width: 100%; height: 100%; object-fit: cover; }
  .chip { width: var(--s-7); height: var(--s-7); margin-left: calc(var(--s-2) * -1); display: grid; place-items: center; border-radius: 50%; background: var(--paper); box-shadow: 0 0 0 var(--s-1) rgb(var(--tint) / 0.9), inset 0 0 0 1px var(--line); color: var(--ink-3); z-index: calc(3 - var(--i)); position: relative; }
  .chip:first-child { margin-left: 0; }
  .sum { flex: 1; font-variant-numeric: tabular-nums; }
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
  /* The deck fans out while the pointer is on the folded row: each card after the first steps out of the overlap (margin only, a few pixels; the row's text does not move more than that). */
  @media (hover: hover) and (pointer: fine) { .head:hover .chip:not(:first-child) { margin-left: var(--s-2); } }
  .files { display: flex; flex-wrap: wrap; gap: var(--s-3); padding: 0 var(--s-5) var(--s-3) calc(var(--s-4) + var(--s-7) + var(--s-4)); }
  .file { display: inline-flex; align-items: center; gap: var(--s-3); max-width: 100%; border-radius: var(--r-select); color: var(--ink-2); font-size: var(--text-label); }
  .file.image { flex-direction: column; align-items: flex-start; gap: var(--s-2); }
  .file img { display: block; width: 7.5rem; max-height: 5rem; object-fit: cover; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); }
  .fn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 12rem; text-decoration: underline; text-underline-offset: 2px; }
  @media (hover: hover) and (pointer: fine) { .step:hover, .head:hover { background: var(--hover); } .file:hover .fn { color: var(--ink); } }
</style>
