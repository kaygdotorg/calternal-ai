<script lang="ts">
  // Where a thread runs, in two choices: a machine, then Sandboxed or Direct. A bottom sheet on a phone, a popover on a desktop (the composer
  // decides); this is the content of both. A choice applies at once, to the next message; it never moves work that is already running.
  import { Capsule, Icon } from "@calternal/ui";
  import type { RunsOn } from "./model";
  let { machines, value, onchange }: { machines: string[]; value: RunsOn; onchange: (r: RunsOn) => void } = $props();
</script>

<div class="picker">
  <h2 id="ro-m">Machine</h2>
  <div role="radiogroup" aria-labelledby="ro-m" class="machines">
    {#each machines as m (m)}
      <button type="button" role="radio" aria-checked={value.machine === m} class="mrow" onclick={() => onchange({ ...value, machine: m })}>
        <Icon name="terminal" /><span>{m}</span>{#if value.machine === m}<Icon name="check" mini />{/if}
      </button>
    {/each}
  </div>
  <h2 id="ro-w">How</h2>
  <Capsule label="Sandboxed or direct" kind="radio" glass={false} items={[{ id: "sandboxed", label: "Sandboxed", icon: "shield" }, { id: "direct", label: "Direct", icon: "folder" }]} value={value.mode} onchange={(v) => onchange({ ...value, mode: v as RunsOn["mode"] })} />
  <p class="note">{value.mode === "sandboxed" ? "The thread gets its own sandbox on this machine." : "The agent works straight on a folder of this machine, with no sandbox."}</p>
</div>

<style>
  .picker { display: grid; gap: var(--s-4); padding: var(--s-5); }
  h2 { font-size: var(--text-section); font-weight: var(--strong-weight); letter-spacing: 0.05em; text-transform: uppercase; color: var(--ink-3); }
  .machines { display: grid; gap: var(--s-1); }
  .mrow { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h-settings); padding: 0 var(--s-5); border-radius: var(--r-select); text-align: start; color: var(--ink); }
  .mrow span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .mrow[aria-checked="true"] { background: var(--accent-soft); color: var(--accent); }
  @media (hover: hover) and (pointer: fine) { .mrow:not([aria-checked="true"]):hover { background: var(--hover); } }
  .note { font-size: var(--text-label); color: var(--ink-3); }
</style>
