<script lang="ts">
  // The one frame of every right panel (docs/ui-shell.md): flat on the background, a header with the window's title (22 px, `--text-title`) and a close button, the body (the app's details, scrolling), and at the bottom the same two tabs everywhere, Details and Assistant, as a full-width
  // equal-segment bar. The Assistant tab is the shell's AI chat stand-in (every app has it); an app only supplies what its Details body says. On a phone the panel is a sheet, which has its own dismissal, so the close button is not drawn there.
  import { getContext, type Snippet } from "svelte";
  import { IconButton, Selector } from "@calternal/ui";
  import { tweaks } from "../tweaks.svelte";
  let { title, sub, tab = $bindable("details"), suggestions = ["Summarise this", "What changed this week?", "Draft a reply"], children }: { title: string; /** A quiet line under the title (what it is, how big). */ sub?: string; tab?: "details" | "assistant"; /** Starting points the Assistant offers, fitted to the app. */ suggestions?: string[]; children: Snippet } = $props();
  const ctx = getContext<{ onclose?: () => void; sheet?: boolean; kind?: string } | undefined>("panel");
  const shown = $derived(tweaks.t.nameOnce && ctx?.kind ? ctx.kind : title);
  let say = $state("");
</script>

<div class="rp" data-right-panel>
  <header><span class="hd"><h2 title={shown}>{shown}</h2>{#if sub}<p class="sub">{sub}</p>{/if}</span>{#if ctx?.onclose && !ctx.sheet}<IconButton icon="close" label="Close the side panel" onclick={ctx.onclose} />{/if}</header>
  <div class="bd">
    {#if tab === "details"}{@render children()}
    {:else}
      <div class="ai"><p class="b">Ask about what is open. I can summarise it, find related items or draft a reply.</p><div class="sg">{#each suggestions as s (s)}<button type="button">{s}</button>{/each}</div></div>
      <form class="in" onsubmit={(e) => { e.preventDefault(); say = ""; }}><input bind:value={say} placeholder="Ask the assistant" aria-label="Ask the assistant" /></form>
    {/if}
  </div>
  <footer><Selector options={[{ value: "details", label: "Details" }, { value: "assistant", label: "Assistant" }]} value={tab} label="Side panel" class="equal" onchange={(v: string) => (tab = v as typeof tab)} /></footer>
</div>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); }
  header { flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--s-3); min-height: var(--pill-h); padding: var(--s-5) var(--s-3) 0; box-sizing: content-box; }
  .hd { display: grid; gap: var(--s-1); min-width: 0; } .sub { margin: 0; color: var(--ink-3); font-size: var(--text-label); line-height: 1.25; }
  h2 { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--s-7); padding: var(--s-4) var(--s-3); }
  .ai { display: grid; gap: var(--s-4); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; }
  .sg { display: flex; flex-wrap: wrap; gap: var(--s-3); } .sg button { min-height: 28px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--hover); font-size: var(--text-label); }
  .in input { width: 100%; box-sizing: border-box; min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); }
  footer { flex: none; padding-bottom: var(--s-3); }
</style>
