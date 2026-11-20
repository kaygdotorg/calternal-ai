<script lang="ts">
  // One item drawn with its kind's row look: leading (avatars of the other people, or the app icon), title, detail, and the
  // trailing marks. `compact` is the one-line sidebar row; otherwise the detail is a quiet second line. The row's menu holds
  // the actions derived from the kind's operations and the host's own item actions. Knows no kind.
  import { Icon, IconButton, Menu, type MenuItem } from "@calternal/ui";
  import Av from "./Av.svelte";
  import { draw, rowActions } from "../lib/look";
  import { glyph } from "../lib/icons";
  import { call } from "../lib/calls.svelte";
  import type { Row } from "../lib/model";
  let { row, compact = false, selected = false, onopen }: { row: Row; compact?: boolean; selected?: boolean; onopen: (r: Row) => void } = $props();
  const d = $derived(draw(row, "row"));
  const actions = $derived(rowActions(row));
  const HOST = [{ id: "host.rename", label: "Rename", icon: "edit" }, { id: "host.move", label: "Move…", icon: "folder" }, { id: "host.share", label: "Share…", icon: "share" }, { id: "host.trash", label: "Move to Trash", icon: "trash", danger: true }];
  const items = $derived<MenuItem[]>([...actions.map((a) => ({ id: a.op, label: a.label, icon: glyph(a.icon) })), ...(HOST as MenuItem[])]);
  let menu = $state(false), trig = $state<HTMLElement | null>(null);
  const pick = (id: string) => { menu = false; call(id, { item: row.item }); };
  const swipe = $derived(actions.find((a) => a.place === "swipe"));
</script>

<div class="rw" class:compact class:sel={selected} data-item={row.item}>
  <button type="button" class="main" aria-current={selected ? "true" : undefined} onclick={() => onopen(row)} oncontextmenu={(e) => { e.preventDefault(); trig = e.currentTarget as HTMLElement; menu = true; }}>
    <span class="lead">
      {#if d.avatars.length > 1}<span class="stack"><Av name={d.avatars[0]!.name} size={20} /><Av name={d.avatars[1]!.name} size={20} /></span>
      {:else if d.avatars.length === 1}<Av name={d.avatars[0]!.name} size={compact ? 22 : 30} />
      {:else}<span class="ic"><Icon name={glyph(d.icon)} /></span>{/if}
    </span>
    <span class="txt">
      <span class="t" class:b={d.trailing.some((m) => "dot" in m || ("count" in m && !m.quiet))}>{d.title}</span>
      {#if d.detail.length && !compact}<span class="s">{d.detail.join(" · ")}</span>{/if}
    </span>
    <span class="tr">
      {#each d.trailing as m, i (i)}
        {#if "text" in m}<span class="tm">{m.text}</span>{:else if "dot" in m}<span class="dot" aria-label="Unread"></span>{:else}<span class="cn" class:q={m.quiet}>{m.count}</span>{/if}
      {/each}
    </span>
  </button>
  {#if !compact}
    {#if swipe}<IconButton icon={glyph(swipe.icon)} label={swipe.label} small onclick={() => call(swipe.op, swipe.inputs)} />{/if}
    <IconButton icon="more" label={`${d.title}: actions`} small onclick={(e) => { trig = e.currentTarget as HTMLElement; menu = true; }} />
  {/if}
  <Menu open={menu} {items} label={`${d.title}: actions`} onpick={pick} onclose={() => (menu = false)} trigger={trig} />
</div>

<style>
  .rw { display: flex; align-items: center; gap: var(--s-2); border-radius: var(--r-select); padding-inline-end: var(--s-2); }
  .rw:hover { background: var(--hover); } .rw.sel { background: var(--accent-soft); }
  .main { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: var(--s-3) var(--s-5); text-align: start; }
  .compact .main { padding-block: 0; }
  .lead { flex: none; display: grid; place-items: center; min-width: 28px; } .compact .lead { min-width: 22px; }
  .ic { color: var(--ink-2); display: grid; }
  .stack { display: inline-flex; } .stack :global(> *:last-child) { margin-inline-start: -8px; box-shadow: 0 0 0 2px var(--surface, transparent); }
  .txt { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .t, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .t.b { font-weight: var(--strong-weight); }
  .s { color: var(--ink-3); font-size: var(--text-label); }
  .tr { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
  .cn { min-width: 18px; height: 18px; padding: 0 5px; box-sizing: border-box; border-radius: 9px; display: grid; place-items: center; background: var(--accent-base); color: #fff; font-size: 11px; font-weight: var(--strong-weight); }
  .cn.q { background: rgb(var(--ink-rgb) / 0.22); color: var(--ink); }
</style>
