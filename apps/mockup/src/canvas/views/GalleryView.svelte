<script lang="ts">
  // The gallery: every tile is ONE fixed frame (the same width, the same 4:3 height, the same radius) with two lines of caption under it (a title, then one muted line), whatever the kind, so the rows never go ragged. What is inside the frame: a note shows its
  // Craft-style page, cropped by the frame; a file and a photo their thumbnail, filling it; a task, a mail and an event their card, centred; a person, an agent and everything else a mark or a big figure centred on a quiet tint. Captions are left-aligned for every kind.
  import Item from "../../shell/Item.svelte";
  import { arr } from "../../arrange.svelte";
  import type { Item as ItemT } from "../../data/collections";
  let { items, onopen }: { items: ItemT[]; onopen: (i: ItemT) => void } = $props();
  const KIND: Record<string, string> = { note: "Note", mail: "Mail", chat: "Chat", task: "Task", file: "File", photo: "Photo", event: "Event", contact: "Contact", transaction: "Transaction", workout: "Workout", label: "Label", setting: "Setting" };
  const CARD = new Set(["task", "mail", "event"]);
  /** One muted line: the kind and the one detail that matters for it. */
  const muted = (i: ItemT) => `${KIND[i.kind] ?? i.kind} · ${(i.kind === "task" ? i.sub : i.kind === "transaction" ? (i.sub ?? i.amount) : i.sub ?? i.when) ?? ""}`.replace(/ · $/, "");
</script>

<div class="gal" data-gallery>
  {#each items as it (it.id)}
    <button type="button" class="t" data-kind={it.kind} data-item={it.id} onclick={() => onopen(it)} style:--hue={it.hue ?? 205}>
      <span class="fr" class:card={CARD.has(it.kind)} class:note={it.kind === "note" || (it.kind === "file" && !!it.body)}>
        {#if CARD.has(it.kind)}<span class="cw"><Item item={{ ...it, status: arr.statusOf(it.id) ?? it.status }} mode="card" /></span>{:else if it.kind === "photo" || it.kind === "file"}<Item item={it} mode="face" />{:else}<Item item={it} mode="face" />{/if}
      </span>
      <span class="cp"><strong title={it.title}>{it.title}</strong><span title={muted(it)}>{muted(it)}</span></span>
    </button>
  {/each}
</div>

<style>
  .gal { display: grid; grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr)); gap: var(--s-6) var(--s-5); align-items: start; padding-bottom: var(--s-6); }
  .t { display: grid; gap: var(--s-3); width: 100%; align-content: start; text-align: start; color: var(--ink); transition: scale var(--dur-fast) var(--ease-out); } .t:active { scale: var(--press-scale); }
  .fr { position: relative; display: grid; place-items: center; aspect-ratio: 4 / 3; border-radius: var(--r-menu); overflow: hidden; background: oklch(0.82 0.04 var(--hue) / 0.35); }
  .fr.note { place-items: start center; background: rgb(var(--ink-rgb) / 0.06); } .fr.note::after { content: ""; position: absolute; inset-inline: 0; bottom: 0; height: var(--s-8); background: linear-gradient(to top, rgb(var(--ink-rgb) / 0.08), transparent); pointer-events: none; }
  .fr.card { background: rgb(var(--ink-rgb) / 0.06); } .cw { display: block; width: 90%; pointer-events: none; } .cw :global(*) { min-width: 0; }
  .cp { display: grid; gap: 1px; justify-items: start; text-align: start; min-width: 0; } .cp strong { max-width: 100%; font-size: var(--text-ui); line-height: 1.25; font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cp span { max-width: 100%; color: var(--ink-3); font-size: var(--text-label); line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
