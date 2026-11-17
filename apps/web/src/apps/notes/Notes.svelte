<script lang="ts">
  // Notes placeholder: proves the middle card's second layout. A narrow centred column with no inner card, and a thin rail at the card's
  // left edge that expands into a small table-of-contents card (hover, focus or press; Escape folds it). Fixture text only.
  import { Glass, Icon, adaptive, ready } from "@calternal/ui";
  import { shell } from "../../shell/shell.svelte";
  import { IconButton } from "@calternal/ui";
  let { id }: { id?: string } = $props();
  const sections = ["Why the journal is read twice", "What changes on restart", "The bookmark", "Open questions"];
  let toc = $state(false);
  const uid = $props.id();
</script>

<div class="note">
  <div class="rail" role="presentation" onpointerenter={() => (toc = true)} onpointerleave={() => (toc = false)}>
    <button type="button" class="ticks" aria-expanded={toc} aria-controls="{uid}-toc" aria-label="Table of contents" onclick={() => (toc = !toc)} onfocus={() => (toc = true)} onkeydown={(e) => e.key === "Escape" && (toc = false)}>
      {#each sections as _}<i></i>{/each}
    </button>
    {#if toc}
      <nav id="{uid}-toc" class="tocard" aria-label="Contents" use:adaptive={"card"} use:ready>
        <Glass dense />
        {#each sections as s, i}<a href="#s{i}" onclick={(e) => e.preventDefault()}>{s}</a>{/each}
      </nav>
    {/if}
  </div>

  <div class="scroll">
  <header>
    {#if shell.narrow || !shell.sidebarOpen}<IconButton icon="sidebar" label="Show notes" onclick={() => shell.toggleSidebar()} />{/if}
    <span class="grow"></span>
    {#if !shell.narrow}<IconButton icon="sidebar-right" label={shell.rightOpen ? "Hide the side card" : "Show the side card"} pressed={shell.rightOpen} onclick={() => shell.toggleRight()} />{/if}
  </header>

  <article>
    <h1>Journal replay notes</h1>
    <p class="lede">{id ? `Note ${id}` : "A placeholder note"} on fixture text. The column is narrow on purpose and has no card around it.</p>
    {#each sections as s, i}
      <h2 id="s{i}">{s}</h2>
      <p>The journal is read once to find the tail and once more to verify it. Both passes touch every entry, which is why a restart with a long journal feels slow. The plan is to read the tail a single time and keep the hash chain in memory while it is verified, so the second pass disappears.</p>
      <p>Nothing about the file format changes. A reader that only wants the last screen should never have to parse the rest.</p>
    {/each}
  </article>
  </div>
</div>

<style>
  .note { position: absolute; inset: 0; }
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  header { position: sticky; top: 0; z-index: 2; display: flex; align-items: center; gap: var(--s-4); min-height: var(--bar-h); padding: 0 var(--s-5); }
  .grow { flex: 1; }
  article { max-width: 40rem; margin: 0 auto; padding: var(--s-5) var(--pad-region) calc(var(--s-10) + var(--bottom-inset, 0px)); display: grid; gap: var(--s-5); }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; letter-spacing: -0.01em; }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); margin-top: var(--s-6); }
  p { font-size: var(--text-ui); line-height: 1.65; font-weight: var(--body-weight); }
  .lede { color: var(--ink-3); }
  .rail { position: absolute; z-index: 3; top: 50%; translate: 0 -50%; left: 0; }
  .ticks { display: grid; gap: var(--s-4); padding: var(--s-6) var(--s-5); min-width: var(--pill-h); justify-items: start; }
  .ticks i { display: block; width: var(--s-6); height: 2px; border-radius: var(--r-pill); background: var(--ink-4); }
  .ticks i:first-child { width: var(--s-8); background: var(--ink-2); }
  .tocard { position: absolute; left: var(--s-5); top: 50%; translate: 0 -50%; isolation: isolate; display: grid; min-width: 15rem; padding: var(--s-3); border-radius: var(--r-card); --origin: left center; }
  .tocard a { padding: var(--s-3) var(--s-5); border-radius: calc(var(--r-card) - var(--s-3)); font-size: var(--text-ui); color: var(--ink); }
  @media (hover: hover) { .tocard a:hover { background: var(--hover); } }
</style>
