<script lang="ts">
  import PanelFrame from "../../shell/PanelFrame.svelte";
  // Contacts' right panel. For a person: the groups they are in, what the product holds about them (counts, each opening its collection beside), their next birthday, and where the card is stored. With no one open: the groups and a tidy-up hint (people who may be one person). Fixture only.
  import AppIcon from "../../shell/AppIcon.svelte";
  import { COLLS, type Item } from "../../data/collections";
  import { wins, type Win } from "../../windows.svelte";
  import { PEOPLE } from "./people";
  import { SIDE } from "./data";
  let { item }: { win?: Win | undefined; item?: Item } = $props();
  const p = $derived(item ? PEOPLE[item.id] : undefined);
  const NAMES: Record<string, string> = { "g-family": "Family", "g-work": "Work", "g-neighbours": "Neighbours", "g-services": "Services" };
  const ICON = { mail: "mail", chats: "chat", events: "calendar" } as const;
  const WORD = (k: "mail" | "chats" | "events", n: number) => (k === "mail" ? "mail" : n === 1 ? k.slice(0, -1) : k);
</script>

<PanelFrame title={p ? p.name : "Contacts"}>
      {#if p}
        <section><h3>Groups</h3>{#if p.groups.length}<div class="chips">{#each p.groups as g (g)}<span class="chip">{NAMES[g]}</span>{/each}</div>{:else}<p class="q">In no group.</p>{/if}</section>
        <section><h3>Elsewhere</h3><ul>{#each SIDE as s (s.key)}{#if p[s.key].length}<li><button type="button" onclick={() => wins.beside("collection", `ct-${p.id}-${s.key}`)}><AppIcon name={ICON[s.key]} inline /><span>{p[s.key].length} {WORD(s.key, p[s.key].length)}</span></button></li>{/if}{/each}{#if !p.mail.length && !p.chats.length && !p.events.length}<li class="q">Nothing yet.</li>{/if}</ul></section>
        {#if p.birthday}<section><h3>Birthday</h3><dl><dt>Date</dt><dd>{p.birthday}</dd>{#if p.birthdayOn}<dt>Next</dt><dd>{p.birthdayOn === "2026-11-21" ? "Tomorrow" : "In 4 days"}</dd>{/if}</dl></section>{/if}
        <section><h3>Stored</h3><dl><dt>Card</dt><dd>contacts/{p.name.toLowerCase().replace(/[^a-z]+/g, "-")}.md</dd><dt>Shared with</dt><dd>Household</dd></dl></section>
      {:else}
        <section><h3>Groups</h3><ul class="gl">{#each ["ct-family", "ct-work", "ct-neighbours", "ct-services"] as id (id)}<li><button type="button" onclick={() => wins.navigate("collection", id)}><span>{COLLS[id]!.name}</span><b>{COLLS[id]!.items.length}</b></button></li>{/each}</ul></section>
        <section><h3>Tidy up</h3><p class="q">Anna Okafor and A. Okafor may be the same person.</p></section>
      {/if}
</PanelFrame>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); line-height: 1.3; } dt { color: var(--ink-3); } dd { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; font-size: var(--text-ui); } li button { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: 32px; padding: 0 var(--s-3); margin-inline: calc(var(--s-3) * -1); box-sizing: content-box; border-radius: var(--r-select); color: var(--ink); text-align: start; } li button b { margin-inline-start: auto; color: var(--ink-3); font-weight: var(--ui-weight); font-size: var(--text-label); } @media (hover: hover) and (pointer: fine) { li button:hover { background: var(--hover); } }
  .chips { display: flex; flex-wrap: wrap; gap: var(--s-3); } .chip { padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; } .q { margin: 0; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.4; }
  .ai { display: grid; gap: var(--s-4); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .sg { display: flex; flex-wrap: wrap; gap: var(--s-3); } .sg button { min-height: 30px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); }
  .in input { width: 100%; box-sizing: border-box; min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); } footer { flex: none; padding-bottom: var(--s-3); }
</style>
