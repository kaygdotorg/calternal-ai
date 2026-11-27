<script lang="ts">
  // A small picture of a note's page, as Craft draws a document: a rounded sheet with a soft shadow, an optional cover, the title in the page's own heading weight, then the first blocks drawn small (text, a heading, bullets, a table's grid, a picture, code on its tinted box).
  // It is a picture of a page, so its text is a scaled drawing (container units), not a text size of the interface, and it is hidden from screen readers: the caption beside it carries the name. Used in strips, galleries and cards.
  import { docOf, plain } from "./data";
  import type { Item } from "../../data/collections";
  let { item, w = "100%" }: { item: Item; w?: string } = $props();
  const doc = $derived(docOf(item)), shown = $derived(doc.blocks.slice(0, 7));
</script>

<span class="pg" style:width={w} style:--h={doc.cover ?? item.hue ?? 205} aria-hidden="true" data-thumb={item.id}>
  {#if doc.cover !== undefined}<span class="cv"></span>{/if}
  <span class="in">
    <span class="ti">{item.title}</span>
    {#each shown as b, i (i)}
      {#if b.t === "p" || b.t === "quote"}<span class="tx" class:q={b.t === "quote"}>{plain(b.runs)}</span>
      {:else if b.t === "h1" || b.t === "h2" || b.t === "h3"}<span class="hh">{plain(b.runs)}</span>
      {:else if b.t === "ul" || b.t === "ol"}<span class="ls">{#each b.items.slice(0, 3) as li, k (k)}<span class="li"><i></i><span>{plain(li)}</span></span>{/each}</span>
      {:else if b.t === "table"}<span class="tb" style:--c={b.head.length}>{#each [b.head, ...b.rows.slice(0, 2)].flat() as c, k (k)}<span class:hd={k < b.head.length}>{c}</span>{/each}</span>
      {:else if b.t === "image"}<span class="im"></span>
      {:else if b.t === "code"}<span class="cd">{b.text.split("\n")[0]}</span>
      {:else if b.t === "todo"}<span class="ls">{#each b.ids.slice(0, 2) as id (id)}<span class="li"><i class="bx"></i><span>{id.replace("tk-", "").replace(/^./, (c) => c.toUpperCase())}</span></span>{/each}</span>{/if}
    {/each}
  </span>
</span>

<style>
  .pg { display: block; position: relative; aspect-ratio: 3 / 4; overflow: hidden; border-radius: var(--r-select); background: var(--paper); box-shadow: 0 1px 2px rgb(var(--shadow-rgb) / 0.12), 0 4px 14px rgb(var(--shadow-rgb) / 0.08), inset 0 0 0 1px rgb(var(--ink-rgb) / 0.06); container-type: inline-size; text-align: start; }
  :global(html[data-scheme="dark"]) .pg { --paper: #2b2b2e; }
  .pg::after { content: ""; position: absolute; inset: auto 0 0; height: 18%; background: linear-gradient(transparent, var(--paper)); }
  .cv { display: block; height: 26%; background: linear-gradient(135deg, oklch(0.82 0.09 var(--h)), oklch(0.7 0.12 calc(var(--h) + 50))); }
  .in { display: grid; align-content: start; gap: 3cqw; padding: 8cqw 9cqw; }
  .ti { font-size: 10.5cqw; line-height: 1.15; font-weight: 700; color: var(--ink); display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .tx { font-size: 6.6cqw; line-height: 1.4; color: var(--ink-2); display: -webkit-box; -webkit-line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; } .tx.q { padding-inline-start: 4cqw; border-inline-start: 0.8cqw solid rgb(var(--ink-rgb) / 0.3); font-style: italic; -webkit-line-clamp: 2; }
  .hh { font-size: 7.6cqw; font-weight: 700; color: var(--ink); line-height: 1.2; margin-top: 1cqw; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .ls { display: grid; gap: 1.6cqw; } .li { display: flex; gap: 3cqw; align-items: baseline; font-size: 6.6cqw; line-height: 1.3; color: var(--ink-2); min-width: 0; } .li > span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; } .li i { flex: none; width: 1.8cqw; height: 1.8cqw; border-radius: 50%; background: var(--ink-3); translate: 0 -0.6cqw; } .li i.bx { width: 4.4cqw; height: 4.4cqw; border: 0.7cqw solid var(--ink-3); background: none; translate: 0 0.2cqw; }
  .tb { display: grid; grid-template-columns: repeat(var(--c), 1fr); border: 0.6cqw solid rgb(var(--ink-rgb) / 0.2); border-radius: 1.5cqw; overflow: hidden; font-size: 5.4cqw; line-height: 1.5; } .tb span { padding: 0.6cqw 2cqw; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: var(--ink-2); border-top: 0.6cqw solid rgb(var(--ink-rgb) / 0.12); } .tb .hd { color: var(--ink); font-weight: 700; background: rgb(var(--ink-rgb) / 0.06); border-top: 0; }
  .im { display: block; height: 22cqw; border-radius: 2cqw; background: linear-gradient(135deg, oklch(0.8 0.09 var(--h)), oklch(0.66 0.12 calc(var(--h) + 40))); }
  .cd { display: block; padding: 2cqw 3cqw; border-radius: 2cqw; background: rgb(var(--ink-rgb) / 0.07); font-family: var(--font-mono); font-size: 5.4cqw; line-height: 1.4; color: var(--ink-2); overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
</style>
