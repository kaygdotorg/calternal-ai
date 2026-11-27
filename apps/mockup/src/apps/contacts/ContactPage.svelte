<script lang="ts">
  // The Contacts window: an open person (their page, with a way back to the list), or the list itself: everyone A to Z under letter headings, one line each (picture, name, what they are to me, quietly), my own card first, with a search field. Searching
  // matches names, roles and organisations; no match says so in words. The groups are the sidebar's collections. MOTION: the list and a card fade in and up over the base duration on the out curve; reduced motion is instant.
  import { SearchField, Selector } from "@calternal/ui";
  import ContactLook from "./ContactLook.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import type { PageProps } from "../registry";
  import ContactRoute from "./ContactRoute.svelte";
  import { CONTACT_ITEMS, PEOPLE } from "./people";
  import { isNewContact } from "./new";
  let { win, onopen, oncollection }: PageProps = $props();
  let q = $state(""), view = $state<"list" | "cards">("list");
  const item = $derived(win.sel ? ITEMS[win.sel] : undefined);
  const match = (c: Item) => { const p = PEOPLE[c.id]!, s = q.trim().toLowerCase(); return !s || [p.name, p.role, p.org ?? ""].some((x) => x.toLowerCase().includes(s)); };
  /** The sort and index letter of a person: the name without a title. */
  const key = (c: Item) => c.title.replace(/^Dr\. /, "");
  const me = $derived(CONTACT_ITEMS.find((c) => PEOPLE[c.id]!.me)!);
  const list = $derived(CONTACT_ITEMS.filter((c) => !PEOPLE[c.id]!.me && match(c)).sort((a, b) => key(a).localeCompare(key(b))));
  const groups = $derived(Object.entries(Object.groupBy(list, (c) => key(c)[0]!.toUpperCase())));
</script>

{#if item && item.kind === "contact" && (PEOPLE[item.id] || isNewContact(item.id))}
  {#key item.id}<div class="in"><ContactRoute {item} {win} onback={() => (win.sel = null)} onall={oncollection} /></div>{/key}
{:else}
  <div class="cl" data-contacts-list>
    <header class="top"><h2>Contacts</h2><p class="m">{CONTACT_ITEMS.length} people</p><div class="s"><div class="sf"><SearchField bind:value={q} label="Search contacts" placeholder="Search" /></div><Selector options={[{ value: "list", label: "List" }, { value: "cards", label: "Cards" }]} value={view} label="Show as" size="sm" fill={false} onchange={(v: string) => (view = v as typeof view)} /></div></header>
    <div class="sc">
      {#if view === "cards"}
        <div class="gal"><ContactLook item={me} mode="cell" {onopen} />{#each list as c (c.id)}<ContactLook item={c} mode="cell" {onopen} />{/each}</div>
        {#if !list.length}<p class="none">No one matches “{q}”. Check the spelling, or search for a role or a company.</p>{/if}
      {:else}
        {#if !q.trim()}<section class="g"><h3>Me</h3><ContactLook item={me} mode="row" {onopen} /></section>{/if}
        {#each groups as [l, cs] (l)}<section class="g"><h3>{l}</h3>{#each cs! as c (c.id)}<ContactLook item={c} mode="row" {onopen} />{/each}</section>
        {:else}<p class="none">No one matches “{q}”. Check the spelling, or search for a role or a company.</p>{/each}
      {/if}
    </div>
  </div>
{/if}

<style>
  .in { flex: 1; min-height: 0; display: flex; flex-direction: column; animation: pin var(--dur-base) var(--ease-out); } @keyframes pin { from { opacity: 0; translate: 0 6px; } }
  .cl { flex: none; display: flex; flex-direction: column; max-width: 36rem; container-type: inline-size; animation: pin var(--dur-base) var(--ease-out); } .top { flex: none; display: flex; align-items: baseline; flex-wrap: wrap; gap: var(--s-2) var(--s-5); padding: var(--s-2) 0 var(--s-5); } h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; } .m { margin: 0; color: var(--ink-3); font-size: var(--text-label); } .s { flex-basis: 100%; display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3) var(--s-4); padding-top: var(--s-3); } .sf { flex: 1 1 12rem; min-width: 0; } .gal { display: grid; grid-template-columns: repeat(auto-fill, minmax(8rem, 1fr)); gap: var(--s-5); padding: var(--s-2) var(--s-3) var(--s-4); }
  .sc { flex: none; /* the window scrolls the whole page, header included (one scroller: no header band to cut rows at) */ padding-bottom: var(--s-9); } .g { padding-bottom: var(--s-3); } h3 { margin: var(--gap-before-head) 0 var(--gap-head-items); padding-inline: var(--s-5); color: var(--ink-3); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .none { margin: var(--s-8) 0 0; text-align: center; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.5; }
</style>
