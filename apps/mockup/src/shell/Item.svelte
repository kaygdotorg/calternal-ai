<script lang="ts">
  // One item drawn with the look of its kind (note, mail, chat, task, file, photo, event, contact, transaction, workout; labels and settings are plain rows), in the shape the place needs: a one-line `row` (a sidebar section; in a window `big` allows the quiet
  // second line), a `strip` tile (a sidebar strip), a `card` (a board) or a `cell` (a gallery). A mixed collection draws each item through here, so every kind looks like itself wherever it appears. Notes and documents are Craft-style page thumbnails in strips,
  // cards and galleries; an agent's chat carries its provider's mark; a Cal carries the icon and colour its person chose. Fixture only: nothing is stored here.
  import AppIcon from "./AppIcon.svelte";
  import Av from "./Av.svelte";
  import Mark from "./Mark.svelte";
  import PageThumb from "./PageThumb.svelte";
  import { arr } from "../arrange.svelte";
  import { looks } from "../apps/registry";
  import type { Item } from "../data/collections";
  import { itemWhen } from "../lib/when";
  let { item, mode = "row", big = false, two = false, selected = false, onopen, oncontext }: { item: Item; mode?: "row" | "strip" | "card" | "cell" | "face"; big?: boolean; /** In a big row: the comfortable density, the secondary text on a second line. */ two?: boolean; selected?: boolean; onopen?: (i: Item) => void; oncontext?: (i: Item, el: HTMLElement) => void } = $props();
  const GLYPH: Record<string, string> = { note: "notes", mail: "mail", chat: "chat", task: "check", file: "file", photo: "image", event: "calendar", contact: "user", transaction: "coin", workout: "pulse", label: "folder", setting: "settings" };
  const KIND: Record<string, string> = { note: "Note", mail: "Mail", chat: "Chat", task: "Task", file: "File", photo: "Photo", event: "Event", contact: "Contact", transaction: "Transaction", workout: "Workout", label: "Label", setting: "Setting" };
  const glyph = $derived(item.icon ?? (item.folder ? "folder" : GLYPH[item.kind]!));
  const person = $derived((item.kind === "chat" && !item.provider && !item.cal) || item.kind === "contact" || item.kind === "mail");
  const cal = $derived(item.cal ? (arr.cal[item.id] ?? { icon: "spark", hue: 205 }) : undefined);
  const page = $derived(item.kind === "note" || (item.kind === "file" && !item.folder && !!item.body));
  const day = $derived((item.due ?? item.when ?? "").slice(0, 3).toUpperCase());
  /** An event's lead mark is its day of the month (fits the lead slot); the weekday is in the row's trailing text. */
  const dom = $derived(item.date ? String(new Date(item.date + "T12:00").getDate()) : day.slice(0, 1));
  const tint = $derived(`oklch(0.78 0.08 ${item.hue ?? 205} / 0.55)`);
  const pos = $derived(item.amount?.startsWith("+"));
  const st = $derived(arr.statusOf(item.id) ?? item.status);
  const when = $derived(itemWhen(item));
  const open = () => onopen?.(item);
  const Look = $derived.by(() => { const l = looks[item.kind]; if (!l) return undefined; if (mode === "face") return item.kind === "file" || item.kind === "photo" ? (typeof l === "function" ? l : l.face) : undefined; return typeof l === "function" ? l : l[mode]; });
</script>

{#snippet lead()}
  {#if cal}<Mark emoji={cal.icon} hue={cal.hue} />
  {:else if item.provider}<Mark provider={item.provider} />
  {:else if person}<Av name={item.title} />
  {:else if item.kind === "task"}<span class="chk" class:done={st === "Done"}>{#if st === "Done"}<AppIcon name="check" inline />{/if}</span>
  {:else if item.kind === "photo" && !item.icon}<span class="th" style:background={tint}></span>
  {:else if item.kind === "event"}<span class="dchip">{dom}</span>
  {:else if item.kind === "label" && !item.icon}<span class="dot" style:--hue={item.hue ?? 205} aria-hidden="true"></span>
  {:else}<span class="ic"><AppIcon name={glyph} /></span>{/if}
{/snippet}

{#if Look}
  <Look {item} {mode} {big} {two} {selected} {onopen} />
{:else if mode === "row"}
  <span class="irow" class:big class:two={big && two} class:sel={selected} data-kind={item.kind} data-item={item.id}>
    <button type="button" class="ib" aria-current={selected ? "true" : undefined} onclick={open} oncontextmenu={(e) => { if (oncontext) { e.preventDefault(); oncontext(item, e.currentTarget as HTMLElement); } }}>
      <span class="lead">{@render lead()}</span>
      <span class="txt"><span class="t" class:b={item.unread} title={item.title}>{item.title}</span>{#if item.sub && (big ? !two : item.kind === "mail")}<span class="s1" title={item.sub}>{item.sub}</span>{/if}{#if big && two && item.sub}<span class="s">{item.sub}</span>{/if}</span>
      <span class="tr">
        {#if item.kind === "transaction"}<span class="amt" class:pos>{item.amount}</span>
        {:else if item.kind === "task"}{#if item.due}<span class="chip">{item.due}</span>{/if}
        {:else}{#if when}<span class="tm">{when}</span>{/if}{#if item.count}<span class="tm">{item.count.toLocaleString()}</span>{/if}{#if item.n}<span class="cn" class:q={item.muted}>{item.n}</span>{/if}{/if}
      </span>
    </button>
    {#if oncontext && item.cal}<button type="button" class="rm" aria-label={`${item.title}: options`} onclick={(e) => oncontext?.(item, e.currentTarget as HTMLElement)}><AppIcon name="more" inline /></button>{/if}
  </span>
{:else if mode === "face"}
  <!-- only the picture of the item, for the frame a gallery tile gives it (every tile is one frame; see canvas/views/GalleryView.svelte) -->
  {#if page}<span class="fpg"><PageThumb {item} w="62%" /></span>
  {:else if cal}<Mark emoji={cal.icon} hue={cal.hue} size="3.25rem" />
  {:else if item.provider}<Mark provider={item.provider} size="3.25rem" />
  {:else if person}<Av name={item.title} size="3.25rem" />
  {:else if item.kind === "event"}<b class="bigday">{day}</b>
  {:else if item.kind === "transaction"}<b class="amtc" class:pos>{item.amount}</b>
  {:else if item.kind === "photo" && !item.icon}<span class="fph" style:background={tint}></span>
  {:else}<span class="ic big"><AppIcon name={glyph} /></span>{/if}
{:else if mode === "strip"}
  <button type="button" class="tile" class:sel={selected} aria-label={item.title} aria-current={selected ? "true" : undefined} data-kind={item.kind} data-item={item.id} onclick={open}>
    {#if page}<span class="pgw"><PageThumb {item} w="52px" /></span><span class="cap">{item.title}</span>
    {:else if person || item.provider}<span class="av">{#if item.provider}<Mark provider={item.provider} size="var(--pin-size, 44px)" />{:else}<Av name={item.title} size="var(--pin-size, 44px)" />{/if}{#if item.n}<span class="badge">{item.n}</span>{/if}</span>
    {:else}<span class="sq" style:background={tint}>{#if item.kind === "photo" && !item.icon}{:else if item.kind === "event"}<b>{day}</b>{:else}<AppIcon name={glyph} />{/if}</span><span class="cap">{item.title}</span>{/if}
  </button>
{:else if mode === "card"}
  <button type="button" class="icard" data-kind={item.kind} data-item={item.id} onclick={open}>
    <span class="k"><AppIcon name={glyph} inline />{KIND[item.kind]}{#if item.unread}<span class="ud"></span>{/if}<span class="sp"></span>{#if when}<span class="kt">{when}</span>{/if}</span>
    {#if page}<span class="pgc"><PageThumb {item} w="72px" /><strong class="two">{item.title}</strong></span>
    {:else if item.kind === "photo" && !item.icon}<span class="thumb" style:background={tint}></span><strong>{item.title}</strong>
    {:else if item.kind === "transaction"}<span class="big" class:pos>{item.amount}</span><strong>{item.title}</strong><span class="s">{item.sub}</span>
    {:else if item.kind === "event"}<span class="evt"><span class="dchip lg">{day}</span><span class="col"><strong>{item.title}</strong><span class="s">{item.sub}</span></span></span>
    {:else if item.kind === "task"}<span class="tk"><span class="chk" class:done={st === "Done"}>{#if st === "Done"}<AppIcon name="check" inline />{/if}</span><strong class="two">{item.title}</strong></span><span class="s">{item.sub}</span>
    {:else if item.kind === "mail"}<span class="pr"><Av name={item.title} size="20px" /><span class="s">{item.title}</span></span><strong class="two">{item.sub}</strong>
    {:else if item.kind === "workout"}<strong>{item.title}</strong><span class="big">{item.sub}</span>
    {:else if item.provider}<span class="pr"><Mark provider={item.provider} /><strong class="two">{item.title}</strong></span><span class="s">{item.sub}</span>
    {:else if person}<span class="pr"><Av name={item.title} /><strong class="two">{item.title}</strong></span><span class="s">{item.sub}</span>
    {:else}<strong class="two">{item.title}</strong><span class="s">{item.sub}</span>{/if}
    {#if item.owner}<span class="ft"><Av name={item.owner} size="18px" />{#if item.n}<span class="cn" class:q={item.muted}>{item.n}</span>{/if}</span>{/if}
  </button>
{:else}
  <button type="button" class="cell" data-kind={item.kind} data-item={item.id} onclick={open}>
    {#if page}<PageThumb {item} />
    {:else}<span class="face" style:background={item.kind === "contact" || item.kind === "chat" ? undefined : tint}>
      {#if item.provider}<Mark provider={item.provider} size="3rem" />
      {:else if person}<Av name={item.title} size="3.25rem" />
      {:else if item.kind === "event"}<b class="bigday">{day}</b>
      {:else if item.kind === "transaction"}<b class="amtc" class:pos>{item.amount}</b>
      {:else if item.kind === "photo" && !item.icon}
      {:else}<AppIcon name={glyph} />{/if}
    </span>{/if}
    <span class="cp"><strong>{item.title}</strong><span class="s">{KIND[item.kind]}{item.sub && !page ? ` · ${item.sub}` : ""}</span></span>
  </button>
{/if}

<style>
  button { text-align: start; color: var(--ink); }
  /* row: ONE line (a mark, the title, what trails). Mail adds the subject after the sender on the same line. */
  .irow { position: relative; display: flex; align-items: center; width: 100%; border-radius: var(--r-select); transition: background-color var(--dur-fast) var(--ease-out); }
  .ib { flex: 1; min-width: 0; max-width: 100%; display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); line-height: 1.25; transition: scale var(--dur-fast) var(--ease-out); }
  .irow.two .ib { min-height: var(--row-h-settings); } .ib:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .irow:not(.sel):hover { background: var(--hover); } }
  .irow.sel { background: var(--accent-soft); }
  .lead { flex: none; display: grid; place-items: center; width: 22px; } .ic { display: grid; color: var(--ink-2); } .irow.sel .ic { color: var(--accent); }
  .txt { flex: 1; min-width: 0; display: flex; align-items: baseline; gap: var(--s-3); } .irow.two .txt { display: grid; gap: 0; } .t, .s1, .s { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .t { flex: 0 1 auto; font-weight: var(--ui-weight); } .t.b { font-weight: var(--strong-weight); } .s1, .s { color: var(--ink-3); font-size: var(--text-label); font-weight: var(--ui-weight); } .s1 { flex: 1 1 0; min-width: 0; }
  .tr { flex: none; display: inline-flex; align-items: center; gap: var(--s-3); } .tm { flex: none; white-space: nowrap; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .cn { min-width: 20px; height: 20px; padding: 0 var(--s-3); box-sizing: border-box; border-radius: var(--r-select); display: inline-grid; place-items: center; font-size: var(--text-label); font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; background: var(--badge-bg); color: #fff; } .cn.q { background: rgb(var(--ink-rgb) / 0.22); color: var(--ink); }
  .ud { width: 8px; height: 8px; border-radius: 50%; background: var(--accent-base); }
  .chk { width: 18px; height: 18px; box-sizing: border-box; border-radius: 50%; border: 2px solid var(--ink-3); display: grid; place-items: center; color: #fff; flex: none; } .chk.done { background: var(--badge-bg); border-color: var(--badge-bg); }
  .chip { flex: none; white-space: nowrap; padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; }
  .th { width: 20px; height: 20px; border-radius: var(--r-select); } .dot { width: 12px; height: 12px; border-radius: 50%; background: oklch(0.72 0.1 var(--hue)); }
  .dchip { min-width: var(--lead); box-sizing: border-box; padding: var(--s-2) 0; border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-section); font-weight: var(--strong-weight); text-align: center; letter-spacing: 0; white-space: nowrap; overflow: hidden; } .dchip.lg { min-width: 44px; padding: var(--s-5) 0; font-size: var(--text-ui); }
  .amt { font-variant-numeric: tabular-nums; font-size: var(--text-label); } .amt.pos, .big.pos, .amtc.pos { color: var(--good, oklch(0.62 0.14 150)); }
  .rm { flex: none; display: grid; place-items: center; width: 26px; height: 26px; margin-inline-end: var(--s-2); border-radius: var(--r-select); color: var(--ink-3); }
  @media (hover: hover) and (pointer: fine) { .rm { opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); } .irow:hover .rm, .irow:focus-within .rm { opacity: 1; } }
  /* strip */
  .tile { flex: none; display: grid; justify-items: center; align-content: start; gap: var(--s-2); width: 56px; border-radius: var(--r-menu); transition: scale var(--dur-fast) var(--ease-out); } .tile:active { scale: var(--press-scale); } .tile.sel .av, .tile.sel .sq { box-shadow: 0 0 0 2px var(--accent); border-radius: 50%; } .tile.sel .sq { border-radius: var(--r-menu); } .tile.sel .pgw { outline: 2px solid var(--accent); outline-offset: 1px; border-radius: var(--r-select); }
  .av { position: relative; display: inline-grid; padding: 2px; border-radius: 50%; } .badge { position: absolute; top: -2px; right: -2px; min-width: 18px; height: 18px; padding: 0 var(--s-3); box-sizing: border-box; border-radius: var(--r-pill); background: var(--badge-bg); color: #fff; font-size: var(--text-label); display: grid; place-items: center; }
  .sq { width: 44px; height: 44px; border-radius: var(--r-menu); display: grid; place-items: center; color: var(--ink-2); } .sq b { font-size: var(--text-label); } .pgw { display: block; padding-top: 2px; } .cap { max-width: 56px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-section); line-height: 1.2; color: var(--ink-3); text-align: center; }
  /* card */
  .icard { display: grid; gap: var(--s-3); width: 100%; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); box-shadow: 0 1px 2px rgb(0 0 0 / 0.08); font-size: var(--text-ui); line-height: 1.25; transition: scale var(--dur-fast) var(--ease-out); } .icard:active { scale: var(--press-scale); }
  .k { display: flex; align-items: center; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; } .k .sp { flex: 1; } .kt { text-transform: none; letter-spacing: 0; font-variant-numeric: tabular-nums; }
  .icard .s { color: var(--ink-3); font-size: var(--text-label); } .icard strong { font-weight: var(--strong-weight); } .two { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .thumb { aspect-ratio: 16 / 9; max-height: 9rem; border-radius: var(--r-select); outline: 1px solid rgb(0 0 0 / 0.1); outline-offset: -1px; } .big { font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; }
  .evt, .tk, .pr, .pgc { display: flex; align-items: center; gap: var(--s-4); } .pgc { align-items: flex-start; } .col { display: grid; } .tk { align-items: flex-start; }
  .ft { display: flex; align-items: center; justify-content: space-between; }
  /* cell */
  .cell { display: grid; gap: var(--s-3); width: 100%; align-content: start; } .face { aspect-ratio: 4 / 3; border-radius: var(--r-menu); display: grid; place-items: center; overflow: hidden; color: var(--ink-2); padding: var(--s-4); box-sizing: border-box; }
  .cp { text-align: start; justify-items: start; display: grid; } .cp .s { color: var(--ink-3); font-size: var(--text-label); } .cp strong { font-weight: var(--strong-weight); font-size: var(--text-ui); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .fpg { display: block; width: 100%; padding-top: var(--s-4); box-sizing: border-box; display: grid; justify-items: center; } .fph { display: block; width: 100%; height: 100%; } .ic.big { transform: scale(1.5); }
  .bigday { font-size: var(--text-title); letter-spacing: 0.05em; } .amtc { font-size: var(--text-title); font-variant-numeric: tabular-nums; }
</style>
