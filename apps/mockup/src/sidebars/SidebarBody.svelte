<script lang="ts">
  // The body of the sidebar: a fixed action row where the app has one (New note), then the sidebar's sections, each a collection shown small, then the foot of the list: "New section" and, when there are hidden sections, "Hidden sections" (the way back).
  // Empty (a new sidebar) it says what to do. With nothing selected it shows "Recent", a collection of what was touched across apps (not editable). It is the drop target for a collection dragged from the card, a section dragged within it, or a row
  // dragged between sections (see dnd.svelte.ts): while dragging, a line shows where it would land.
  import AppIcon from "../shell/AppIcon.svelte";
  import Section from "./Section.svelte";
  import { ACTIONS, type Item } from "../data/collections";
  import { actionHandlers } from "../apps/registry";
  import { arr } from "../arrange.svelte";
  import AddSection from "../shell/AddSection.svelte";
  import { dnd } from "../dnd.svelte";
  import { roving } from "../lib/roving";
  let { sid, selected, onopen, oncollection, narrow, appName, onapp, onaction }: { sid: string | null; selected: string | null; onopen: (i: Item) => void; oncollection: (cid: string) => void; narrow: boolean; /** The sidebar's name, and the way to its app's own page (a phone shows the sidebar first, so the page needs a door). */ appName?: string; onapp?: () => void; /** Called after an action row's app handler ran (a phone pushes the canvas to show what it opened). */ onaction?: () => void } = $props();
  let el = $state<HTMLElement>(), showHidden = $state(false), adding = $state(false);
  const all = $derived(sid ? arr.sections(sid).filter((s) => arr.coll(s.cid)) : []);
  const secs = $derived(all.filter((s) => !s.hidden)), hidden = $derived(all.filter((s) => s.hidden));
  const action = $derived(sid ? ACTIONS[sid] : undefined);
  $effect(() => { if (!el) return; dnd.zone = sid ? { el, sid } : null; return () => { if (dnd.zone?.el === el) dnd.zone = null; }; });
  const line = $derived(dnd.drag?.over && sid ? dnd.drag.line : null);
</script>

<div class="body" class:narrow bind:this={el} use:roving={{ selector: ".tog, .ib, .row, .main, .tile, .fb, .all, .act, .tr button.ti, [data-row] > button, [data-row] a", extras: ".ab, .grip, .rm, .pin, .ck, .mo" }} data-sidebar-body data-sid={sid ?? ""}>
  {#if narrow && sid && onapp}<button type="button" class="act" data-open-app onclick={onapp}><span class="lead"><AppIcon name="expand" /></span><span>Open {appName}</span></button>{/if}
  {#if action}<button type="button" class="act" onclick={() => { if (sid) { actionHandlers[sid]?.(); onaction?.(); } }}><span class="lead"><AppIcon name="plus" /></span><span>{action}</span></button>{/if}
  {#if sid}
    {#each secs as s (s.cid)}<Section {sid} sec={s} {selected} {onopen} {oncollection} />{/each}
    {#if !secs.length && !hidden.length}<p class="none" data-empty>Nothing here yet. Use + below to add a collection.</p>{/if}
    <div class="foot">
      <button type="button" class="fb" onclick={() => (adding = true)} data-new-section aria-label="Add a section"><AppIcon name="plus" inline />Add a section</button>
      {#if hidden.length}<button type="button" class="fb" aria-expanded={showHidden} onclick={() => (showHidden = !showHidden)}>Hidden sections ({hidden.length})</button>{/if}
    </div>
    {#if sid}<AddSection open={adding} {sid} onclose={() => (adding = false)} />{/if}
    {#if showHidden}<div class="hid">{#each hidden as s (s.cid)}<div class="hr"><span class="hn">{s.name ?? arr.coll(s.cid)?.name}</span><button type="button" class="fb" onclick={() => arr.patchSection(sid, s.cid, { hidden: false })}>Show</button></div>{/each}</div>{/if}
  {:else}
    <Section sid={null} sec={{ cid: "recent-all" }} editable={false} {selected} {onopen} {oncollection} />
  {/if}
  {#if line}<div class="line" class:v={line.width === 3} style:left="{line.left}px" style:top="{line.top}px" style:width="{line.width}px" style:height="{line.height}px" data-dropline></div>{/if}
</div>

<style>
  .body { container-type: inline-size; } @container (max-width: 230px) { .body :global(.tm), .body :global(.cn) { display: none; } }
  .body { position: relative; flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; padding: var(--s-2) 0 var(--s-10);
    /* the foot's fade is one row tall, with as much room under the last row, so no row or heading is ever cut through its middle */
    -webkit-mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--row-h)), transparent); mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--row-h)), transparent); }
  .body.narrow { --bar: calc(var(--pill-h) + var(--s-9) + var(--tb-h) + var(--s-5) + env(safe-area-inset-bottom, 0px)); padding-bottom: var(--bar); -webkit-mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--bar) - var(--s-8)), transparent calc(100% - var(--bar) + var(--s-3))); mask-image: linear-gradient(to bottom, transparent, #000 var(--s-4), #000 calc(100% - var(--bar) - var(--s-8)), transparent calc(100% - var(--bar) + var(--s-3))); }
  .act { display: flex; align-items: center; gap: var(--lead-gap); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); color: var(--ink); font-size: var(--text-ui); } .act:hover { background: var(--hover); }
  .lead { flex: none; display: grid; place-items: center; width: var(--lead); }
  .none { margin: var(--s-6) var(--s-3); color: var(--ink-3); font-size: var(--text-ui); line-height: 1.45; }
  .foot { display: flex; flex-wrap: wrap; gap: var(--s-3); margin-top: var(--s-6); padding: 0 var(--s-3); } .fb { display: inline-flex; align-items: center; gap: var(--s-3); min-height: 28px; padding: 0 var(--s-4); border-radius: var(--r-pill); color: var(--ink-2); font-size: var(--text-label); } .fb:hover { background: var(--hover); color: var(--ink); }
  .hid { display: grid; gap: var(--s-1); margin: var(--s-3) var(--s-3) 0; } .hr { display: flex; align-items: center; justify-content: space-between; min-height: var(--row-h); padding-inline-start: var(--s-3); } .hn { color: var(--ink-3); font-size: var(--text-ui); text-decoration: line-through; }
  .line { position: absolute; border-radius: var(--r-pill); background: var(--accent); pointer-events: none; z-index: 3; box-shadow: 0 0 0 3px var(--accent-soft); }
</style>
