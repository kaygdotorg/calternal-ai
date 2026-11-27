<script lang="ts">
  // A conversation's three looks. ROW: one line (a picture, the sender, then the subject and the first words of the latest message quietly, a paperclip if anything is attached, the time); unread is bold with a dot. CARD: for a board or a strip of "pick up where
  // you left off": sender and time, the subject (two lines), the first words (two lines), what is attached. CELL: for a gallery: a large picture of the sender, the sender, the subject on one line, the time. Fixture only.
  import AppIcon from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import type { Item } from "../../data/collections";
  import { THREADS } from "./data";
  let { item, mode, selected = false, onopen }: { item: Item; mode: "row" | "card" | "cell"; selected?: boolean; onopen?: (i: Item) => void } = $props();
  const t = $derived(THREADS[item.id]), last = $derived(t?.msgs.at(-1)), atts = $derived(t?.msgs.flatMap((m) => m.atts ?? []) ?? []);
  const snippet = $derived(last?.paras.join(" ").replace(/\s+/g, " ") ?? "");
  const who = $derived(item.title), subject = $derived(item.sub ?? t?.subject ?? "");
</script>

{#if mode === "row"}
  <button type="button" class="row" class:sel={selected} class:un={item.unread} aria-current={selected ? "true" : undefined} data-mail-row={item.id} onclick={() => onopen?.(item)}>
    <Av name={who} size="28px" /><span class="w">{who}</span><span class="s"><b>{subject}</b>{#if snippet}<i> {snippet}</i>{/if}</span>
    {#if atts.length}<span class="cl" title={`${atts.length} attached`}><AppIcon name="paperclip" inline /></span>{/if}<time>{item.when}</time>
  </button>
{:else if mode === "card"}
  <button type="button" class="card" class:un={item.unread} data-mail-card={item.id} onclick={() => onopen?.(item)}>
    <span class="h"><Av name={who} size="20px" /><span class="w">{who}</span>{#if item.unread && !item.n}<span class="ud" aria-label="Unread"></span>{/if}<time>{item.when}</time></span>
    <strong class="two">{subject}</strong>{#if snippet}<span class="two q">{snippet}</span>{/if}
    {#if atts.length}<span class="at"><AppIcon name="paperclip" inline />{atts.length} attached</span>{/if}
  </button>
{:else}
  <button type="button" class="cell" class:un={item.unread} data-mail-cell={item.id} onclick={() => onopen?.(item)}>
    <span class="face"><Av name={who} size="3.5rem" /></span><span class="cp"><strong>{who}</strong><span class="q">{subject}</span><time>{item.when}</time></span>
  </button>
{/if}

<style>
  button { color: var(--ink); text-align: start; }
  .row { display: flex; align-items: center; gap: var(--s-5); width: 100%; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out); } .row.sel { background: var(--accent-soft); } @media (hover: hover) and (pointer: fine) { .row:not(.sel):hover { background: var(--hover); } }
  .w { flex: none; max-width: 30%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--ui-weight); } .un .w, .un .s b { font-weight: var(--strong-weight); } .s { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-3); font-size: var(--text-label); } .s b { font-weight: var(--ui-weight); color: var(--ink-2); } .s i { font-style: normal; }
  .cl { flex: none; display: grid; color: var(--ink-3); } .ud { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--accent-base); } time { flex: none; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .card { display: grid; gap: var(--s-3); width: 100%; box-sizing: border-box; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(0 0 0 / 0.08); font-size: var(--text-ui); line-height: 1.3; transition: scale var(--dur-fast) var(--ease-out); } .card:active { scale: var(--press-scale); }
  .h { display: flex; align-items: center; gap: var(--s-3); font-size: var(--text-label); } .h .w { flex: 1; max-width: none; } .h time { margin-inline-start: auto; } .card strong { font-weight: var(--strong-weight); } .un strong { font-weight: var(--strong-weight); }
  .two { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; max-height: 2.65em; } .q { color: var(--ink-3); font-size: var(--text-label); } .at { color: var(--ink-2); display: flex; align-items: center; gap: var(--s-2); font-size: var(--text-label); }
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; } .face { display: grid; place-items: center; aspect-ratio: 4 / 3; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); } .cp { display: grid; min-width: 0; } .cp strong { font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cp .q { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
