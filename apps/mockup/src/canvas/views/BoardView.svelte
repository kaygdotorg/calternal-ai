<script lang="ts">
  // The board (kanban): delegates to `@calternal/canvas-shell`'s `BoardShell` with mockup fixture columns and `Item` cards.
  import { BoardShell, type BoardColumn } from "@calternal/canvas-shell";
  import Item from "../../shell/Item.svelte";
  import { arr } from "../../arrange.svelte";
  import type { Item as ItemT } from "../../data/collections";

  let { items, onopen }: { items: ItemT[]; onopen: (i: ItemT) => void } = $props();
  const COLS = ["To do", "Doing", "Done"] as const;
  const st = (i: ItemT) => arr.statusOf(i.id) ?? COLS[[...i.id].reduce((a, c) => a + c.charCodeAt(0), 0) % 3]!;
  const columns = $derived<BoardColumn<ItemT>[]>(
    COLS.map((c) => ({ id: c, label: c, items: items.filter((i) => st(i) === c) })),
  );
</script>

<BoardShell {columns} emptyText="No cards" onmove={(item, to) => arr.setStatus(item.id, to)}>
  {#snippet renderCard(item, quiet)}
    <Item item={{ ...item, status: st(item) }} mode="card" onopen={(i) => { if (!quiet) onopen(i); }} />
  {/snippet}
</BoardShell>
