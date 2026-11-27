<script lang="ts">
  // A person's three looks. ROW: one line (picture, name, what they are to me, quietly). CARD: picture, name and role, then the first phone and email quietly, for a board. CELL: a large picture over the name and role, for a gallery. Fixture only.
  import Av from "../../shell/Av.svelte";
  import type { Item } from "../../data/collections";
  import { PEOPLE } from "./people";
  let { item, mode, onopen }: { item: Item; mode: "row" | "card" | "cell"; onopen?: (i: Item) => void } = $props();
  const p = $derived(PEOPLE[item.id]);
</script>

{#if mode === "row"}
  <button type="button" class="row" data-item={item.id} onclick={() => onopen?.(item)}><Av name={item.title} size="32px" /><span class="n">{item.title}</span><span class="s">{p?.me ? "My card" : (item.sub ?? "")}</span></button>
{:else if mode === "card"}
  <button type="button" class="card" data-item={item.id} onclick={() => onopen?.(item)}><span class="h"><Av name={item.title} size="40px" /><span class="id"><strong>{item.title}</strong><span class="s">{item.sub ?? ""}</span></span></span>{#if p?.phones[0]}<span class="s">{p.phones[0][1]}</span>{/if}{#if p?.emails[0]}<span class="s">{p.emails[0][1]}</span>{/if}</button>
{:else}
  <button type="button" class="cell" data-item={item.id} onclick={() => onopen?.(item)}><span class="face"><Av name={item.title} size="4.5rem" /></span><span class="cp"><strong>{item.title}</strong><span class="s">{item.sub || " "}</span></span></button>
{/if}

<style>
  button { color: var(--ink); text-align: start; }
  .row { display: flex; align-items: center; gap: var(--s-5); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .row:active { scale: var(--press-scale); } @media (hover: hover) and (pointer: fine) { .row:hover { background: var(--hover); } }
  .n { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--strong-weight); } .s { color: var(--ink-3); font-size: var(--text-label); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .row .s { flex: 1 1 0; min-width: 0; }
  .card { display: grid; gap: var(--s-2); width: 100%; box-sizing: border-box; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(0 0 0 / 0.08); font-size: var(--text-ui); transition: scale var(--dur-fast) var(--ease-out); } .card:active { scale: var(--press-scale); } .h { display: flex; align-items: center; gap: var(--s-4); margin-bottom: var(--s-2); } .id { display: grid; min-width: 0; } .id strong, .cp strong { font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; } .face { display: grid; place-items: center; aspect-ratio: 4 / 3; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); transition: background-color var(--dur-fast) var(--ease-out); } @media (hover: hover) and (pointer: fine) { .cell:hover .face { background: rgb(var(--ink-rgb) / 0.1); } } .cp { display: grid; justify-items: center; text-align: center; min-width: 0; }
</style>
