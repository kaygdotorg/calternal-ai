<script lang="ts">
  // The snapshot nodes of a checked item view (docs/description-contract.md 5.1 and "Entry views"): stack, text (literal, or the closed inline subset), code, heading, image and attachment, and for a conversation the
  // entry (a message: author, time, edited mark, "asked by", queued or failed, its content), the date and system markers, a checked quote and an item link. Consecutive entries with the same author key within five minutes
  // and no marker between them group under one name. Own entries and a provider mark come only from the host's `entry_controls`, never from page content. Text and code are literal: nothing here makes HTML, and a node type
  // this renderer does not know draws nothing.
  import { haptic, Icon } from "@calternal/ui";
  import type { SnapshotNode } from "../draft";
  import { knownIcon } from "../render/icons";
  import { parseInline } from "./inline";
  import { useEntry } from "./entry-context";
  import { endsGroup, startsGroup } from "./groups";
  import EntryRequest from "./EntryRequest.svelte";
  import Snapshot from "./Snapshot.svelte";
  let { node, first = true, last = true, entry, endText = false }: { node: SnapshotNode; first?: boolean; last?: boolean; /** The anchor of the entry this node sits in. */ entry?: string; /** The text ends the bubble: the time follows it on the same line. */ endText?: boolean } = $props();
  const ctx = useEntry();
  const t = (ts: string | undefined): number => (ts ? new Date(ts).getTime() : NaN);
  /** The viewer's local time for a UTC instant, in the viewer's locale: the time today, the date and time otherwise. */
  function local(ts: string | undefined): string {
    if (!ts) return "";
    const d = new Date(ts); if (Number.isNaN(d.getTime())) return "";
    const same = new Date().toDateString() === d.toDateString();
    return new Intl.DateTimeFormat(undefined, same ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" }).format(d);
  }
  /** The viewer's local clock time for a UTC instant (an entry's time: the date is the separator's). */
  const clock = (ts: string | undefined): string => { const d = ts ? new Date(ts) : null; return d && !Number.isNaN(d.getTime()) ? new Intl.DateTimeFormat(undefined, { timeStyle: "short" }).format(d) : ""; };
  /** The day a date marker names, in the viewer's locale: Today, Yesterday, or the date. */
  function day(ts: string): string {
    const d = new Date(ts), n = new Date(), y = new Date(n.getTime() - 86400000);
    if (d.toDateString() === n.toDateString()) return "Today";
    if (d.toDateString() === y.toDateString()) return "Yesterday";
    return new Intl.DateTimeFormat(undefined, { dateStyle: "long" }).format(d);
  }
  const quoteOf = (e: string | undefined, span: string) => ctx?.state.reply?.quotes?.find((q) => q.entry === e && q.source.span === span);
  const unreadHere = $derived(node.type === "snapshot-entry" && (ctx?.state.reply?.reading?.first_unread === node.anchor || ctx?.state.leaving === node.anchor));
  const unread = $derived(node.type === "snapshot-entry" && ctx?.state.reply?.reading?.first_unread === node.anchor ? ctx.state.reply.reading.unread : undefined);
  const leaving = $derived(node.type === "snapshot-entry" && ctx?.state.leaving === node.anchor && ctx?.state.reply?.reading?.first_unread !== node.anchor);
  const arriving = $derived(node.type === "snapshot-entry" && !!ctx?.state.fresh.has(node.anchor));
  // Touch: holding a message opens its menu (iOS fires no contextmenu); a tick of haptic feedback on opening. A move cancels the hold; the release after it opens nothing else.
  let hold = 0, hx = 0, hy = 0, held = false;
  function down(ev: PointerEvent) { if (ev.pointerType !== "touch" || !ctx || ctx.state.selecting || !control?.actions?.length || node.type !== "snapshot-entry") return; hx = ev.clientX; hy = ev.clientY; held = false; const a = node.anchor; clearTimeout(hold); hold = window.setTimeout(() => { held = true; haptic("selection"); ctx.menu(a, { x: hx, y: hy }); }, 450); }
  function moved(ev: PointerEvent) { if (hold && Math.hypot(ev.clientX - hx, ev.clientY - hy) > 8) { clearTimeout(hold); hold = 0; } }
  function up() { clearTimeout(hold); hold = 0; }
  const control = $derived(node.type === "snapshot-entry" ? ctx?.state.controlOf(node.anchor) : undefined);
  const mark = $derived(control?.provider_icon ? knownIcon(control.provider_icon) ?? "threads" : undefined);
  const picked = $derived(node.type === "snapshot-entry" && !!ctx?.state.picked.length && inPick(node.anchor));
  function inPick(anchor: string): boolean { const p = ctx!.state.picked; if (p.length === 1) return p[0] === anchor; const ids = [...(ctx!.state.reply ? anchors() : [])]; const a = ids.indexOf(p[0]!), b = ids.indexOf(p[1]!), i = ids.indexOf(anchor); return i >= Math.min(a, b) && i <= Math.max(a, b); }
  function anchors(): string[] { const out: string[] = []; const walk = (n: SnapshotNode | undefined) => { if (!n) return; if (n.type === "snapshot-entry") out.push(n.anchor); else if (n.type === "stack") n.children.forEach(walk); }; walk(ctx?.state.reply?.view?.content); return out; }
  const lastIsText = $derived(node.type === "snapshot-entry" && node.children.at(-1)?.type === "snapshot-text");
  const state = $derived(node.type === "snapshot-entry" ? node.state : undefined);
  const label = $derived(node.type === "snapshot-entry" ? `${control?.own ? "You" : node.author.name}, ${clock(node.timestamp)}${node.edited_at ? ", edited" : ""}${node.asked_by ? `, asked by ${node.asked_by}` : ""}${state === "queued" ? ", sending" : state === "failed" ? ", not sent" : ""}` : "");
</script>

{#snippet inline(text: string)}{#each parseInline(text) as s, k (k)}{#if s.t === "strong"}<strong>{s.text}</strong>{:else if s.t === "em"}<em>{s.text}</em>{:else if s.t === "code"}<code class="ic">{s.text}</code>{:else if s.t === "link"}<a href={s.href} target="_blank" rel="noopener noreferrer nofollow">{s.text}</a>{:else}{s.text}{/if}{/each}{/snippet}

{#if node.type === "stack"}
  <div class="s" class:row={node.axis === "horizontal"} data-snapshot="stack">{#if node.title}<h3 class="st">{node.title}</h3>{/if}{#each node.children as c, k (c.type === "snapshot-entry" ? c.anchor : `${c.type}:${k}`)}<Snapshot node={c} first={startsGroup(node.children, k)} last={endsGroup(node.children, k)} />{/each}</div>
{:else if node.type === "snapshot-marker"}
  {#if node.kind === "date"}<div class="mk" data-snapshot="marker" data-marker="date"><time datetime={node.timestamp}>{day(node.timestamp)}</time></div>
  {:else}<div class="mk sys" data-snapshot="marker" data-marker="system"><span>{node.text}</span></div>{/if}
{:else if node.type === "snapshot-entry"}
  {#if unreadHere}<div class="un" class:gone={leaving} data-unread-separator role="separator"><span>{unread ? `${unread} unread message${unread === 1 ? "" : "s"}` : "Unread messages"}</span></div>{/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events, a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_to_interactive_role -->
  <article class="e" class:arrive={arriving} class:own={!!control?.own} class:first class:last class:picked class:selecting={ctx?.state.selecting} class:queued={state === "queued"} data-entry data-anchor={node.anchor} data-own={control?.own || undefined} data-author={node.author.key}
    role={ctx?.state.selecting ? "option" : "article"} aria-label={label} aria-selected={ctx?.state.selecting ? !!picked : undefined} tabindex={ctx && ctx.state.active === node.anchor ? 0 : -1}
    onfocus={() => { if (ctx) ctx.state.active = node.anchor; }} onpointerdown={down} onpointermove={moved} onpointerup={up} onpointercancel={up}
    oncontextmenu={(ev) => { if (ctx && control?.actions?.length) { ev.preventDefault(); ctx.menu(node.anchor, { x: ev.clientX, y: ev.clientY }); } }}
    onclick={(ev) => { if (held) { held = false; ev.stopPropagation(); return; } if (ctx?.state.selecting) ctx.pick(node.anchor); }}>
    {#if first && !control?.own}<header class="eh"><span class="nm">{node.author.name}</span>{#if mark}<span class="pm" data-provider-mark title="Agent"><Icon name={mark} inline /></span>{/if}{#if node.asked_by}<span class="ask" data-asked-by>asked by {node.asked_by}</span>{/if}</header>{/if}
    <div class="bub">
      {#each node.children as c, k (k)}
        {#if c.type === "snapshot-quote"}
          {@const q = quoteOf(node.anchor, c.source.span)}
          {#if q}<button type="button" class="qt" data-quote data-target={q.target?.anchor} disabled={!q.target} onclick={(ev) => { ev.stopPropagation(); if (q.target) ctx?.jump(q.target.anchor); }}>{#if q.author}<span class="qa">{q.author}</span>{/if}<span class="qx">{q.text}</span></button>{/if}
        {:else if c.type === "snapshot-link"}
          <button type="button" class="lk" data-item-link onclick={(ev) => { ev.stopPropagation(); ctx?.link(c.target); }}><Icon name="link" inline />{c.label}</button>
        {:else}<Snapshot node={c} entry={node.anchor} endText={lastIsText && k === node.children.length - 1} />{/if}
      {/each}
      <span class="meta" class:flow={!lastIsText}><time datetime={node.timestamp} title={local(node.timestamp)}>{clock(node.timestamp)}</time>{#if node.edited_at}<span class="ed" data-edited title={`Edited ${local(node.edited_at)}`}>edited</span>{/if}{#if state === "queued"}<span class="sd" data-state-mark>Sending…</span>{:else if state === "failed"}<span class="sd bad" data-state-mark role="alert">Not sent</span>{/if}</span>
      {#if ctx && control?.actions?.length && !ctx.state.selecting}<button type="button" class="more" data-entry-menu aria-label="Message actions" onclick={(ev) => { ev.stopPropagation(); const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(); ctx.menu(node.anchor, { x: r.left, y: r.bottom }); }}><Icon name="chevron-down" inline /></button>{/if}
    </div>
    {#each ctx?.state.reply?.requests?.filter((r) => r.anchor === node.anchor) ?? [] as r (r.card.request_ref)}<EntryRequest card={r.card} />{/each}
  </article>
{:else if node.type === "snapshot-heading"}
  <div class="h" data-snapshot="heading" data-anchor={node.anchor}><strong>{node.title}</strong>{#if node.author}<span class="a">{node.author}</span>{/if}{#if node.timestamp}<time datetime={node.timestamp}>{local(node.timestamp)}</time>{/if}</div>
{:else if node.type === "snapshot-text"}
  <p class="t {node.style ?? 'body'} {node.tone ?? ''}" data-snapshot="text" data-anchor={node.anchor}>{#if node.markup === "inline"}{@render inline(node.text)}{:else}{node.text}{/if}{#if endText}<span class="sp" aria-hidden="true"></span>{/if}</p>
{:else if node.type === "snapshot-code"}
  <pre class="c" data-snapshot="code" data-anchor={node.anchor}><code>{node.text}</code></pre>
{:else if node.type === "snapshot-attachment"}
  <div class="att" data-snapshot="attachment" data-anchor={node.anchor} role="group" aria-label={node.label}><Icon name="paperclip" inline /><span>{node.label}</span></div>
{:else if node.type === "snapshot-image"}
  <div class="att" data-snapshot="image" data-anchor={node.anchor} role="img" aria-label={node.alt ?? "Image"}>{node.alt ?? "Image"}</div>
{/if}

<style>
  /* Everything here is the app's own tokens: type scale, spacing, concentric radii (a bubble is the card radius, a nested block the select radius), ink and the accent. */
  .s { display: flex; flex-direction: column; gap: var(--s-1); } .s.row { flex-direction: row; flex-wrap: wrap; gap: var(--s-3); }
  .st { margin: 0; font-size: var(--text-section); color: var(--ink-3); font-weight: var(--strong-weight); }
  .h { display: flex; flex-wrap: wrap; align-items: baseline; gap: var(--s-3); margin-top: var(--s-4); }
  .h .a { color: var(--ink-2); } .h time { color: var(--ink-3); font-size: var(--text-label); margin-inline-start: auto; }
  .t { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--font-chat); font-size: var(--text-body); font-weight: var(--body-weight); line-height: 1.4; } .t.title { font-size: var(--text-title); font-weight: var(--strong-weight); } .t.heading { font-weight: var(--strong-weight); } .t.caption { color: var(--ink-3); font-size: var(--text-label); }
  .t.good { color: var(--good); } .t.warn { color: var(--warn); } .t.bad { color: var(--danger); }
  .ic { font-family: var(--font-mono); font-size: var(--text-code); padding: 0 var(--s-1); border-radius: 4px; background: rgb(var(--ink-rgb) / 0.08); }
  .c { margin: 0; max-width: 100%; box-sizing: border-box; padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); overflow: auto; font-size: var(--text-code); line-height: 1.45; }
  .c code { font-family: var(--font-mono); }
  .att { display: inline-flex; align-items: center; gap: var(--s-3); padding: var(--s-2) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); width: fit-content; max-width: 100%; font-size: var(--text-label); }
  .att span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }
  .ic { font-family: var(--font-mono); }
  /* An entry: a bubble whose time ends its last line (as Telegram's does), names above the first of a group, own messages to the end in the accent's soft tint, others to the start in the ink tint. A group's inner corners are tighter, so a run reads as one. */
  .e { display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-1); margin-top: var(--s-1); max-width: min(82%, 38rem); scroll-margin: var(--s-6); position: relative; }
  /* Messages off screen are skipped by the browser (not laid out or painted) until they near the viewport: a long thread costs what is in view. */
  :global(html:not([data-no-cv])) .e { content-visibility: auto; contain-intrinsic-size: auto 4.5rem; }
  .e.first { margin-top: var(--s-4); } .e.own { align-self: flex-end; align-items: flex-end; }
  .e.selecting { cursor: pointer; } .e.picked .bub { outline: 2px solid var(--accent); outline-offset: 1px; }
  :global(.hl) { animation: hl var(--dur-slow) var(--ease-out); } @keyframes hl { from { background: var(--accent-soft); } to { background: transparent; } }
  .eh { display: flex; align-items: center; gap: var(--s-2); padding-inline: var(--s-4); font-size: var(--text-label); } .nm { font-weight: var(--strong-weight); } .ask, .pm { color: var(--ink-3); } .pm { display: inline-grid; }
  .bub { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-2); padding: var(--s-3) var(--s-4); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); min-width: 0; max-width: 100%; }
  .e:not(.own):not(.first) .bub { border-top-left-radius: var(--r-select); } .e:not(.own):not(.last) .bub { border-bottom-left-radius: var(--r-select); } .e:not(.own).last .bub { border-bottom-left-radius: var(--r-select); }
  .e.own .bub { background: var(--accent-soft); } .e.own:not(.first) .bub { border-top-right-radius: var(--r-select); } .e.own:not(.last) .bub { border-bottom-right-radius: var(--r-select); } .e.own.last .bub { border-bottom-right-radius: var(--r-select); }
  .e.queued .bub { opacity: 0.6; }
  .e:focus-visible { outline: none; } .e:focus-visible .bub { outline: 2px solid var(--accent); outline-offset: 1px; }
  @media (pointer: coarse) { .e { -webkit-touch-callout: none; user-select: none; } }
  /* A message that arrives while you read eases in: it rises 8 px and fades over --dur-base on --ease-out (interruptible, 1 ms under reduced motion), once. */
  .e.arrive { animation: arrive var(--dur-base) var(--ease-out) both; } @keyframes arrive { from { opacity: 0; translate: 0 var(--s-3); } to { opacity: 1; translate: 0 0; } }
  /* The time sits at the end of the last line: a spacer the width of the time (and of "edited" or "Sending…") closes the text, the time is placed over it. A bubble that ends in a quote, code, a link or a file has the time on a line of its own, at the end. */
  .sp { display: inline-block; width: calc(1ch * (var(--mw, 9))); }
  .bub:has(.ed) { --mw: 17; } .bub:has(.sd) { --mw: 20; }
  .meta { display: inline-flex; align-items: baseline; gap: var(--s-2); color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; white-space: nowrap; position: absolute; right: var(--s-4); bottom: var(--s-2); }
  .meta.flow { position: static; align-self: flex-end; margin-top: calc(var(--s-1) * -1); }
  .ed, .sd { font-style: italic; } .sd.bad { color: var(--danger); font-style: normal; }
  .more { position: absolute; top: var(--s-1); right: var(--s-1); width: 24px; height: 24px; display: grid; place-items: center; border-radius: 50%; color: var(--ink-2); background: rgb(var(--tint) / 0.85); opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); }
  .e:hover .more, .more:focus-visible { opacity: 1; }
  /* The quote's leading bar is a straight, square-ended strip (a pseudo element the full height of the quote), never a rounded border. */
  .qt { position: relative; display: grid; gap: 2px; align-self: stretch; text-align: start; padding: var(--s-2) var(--s-3) var(--s-2) calc(var(--s-3) + 3px); border-radius: 0 var(--r-select) var(--r-select) 0; background: var(--accent-soft); color: inherit; }
  .qt::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; width: 3px; background: var(--accent); }
  .qt:disabled { cursor: default; } .qa { font-weight: var(--strong-weight); font-size: var(--text-label); color: var(--accent); } .qx { display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; color: var(--ink-2); font-size: var(--text-label); }
  .lk { display: inline-flex; align-items: center; gap: var(--s-2); width: fit-content; color: var(--accent); text-align: start; font-size: var(--text-body); } .lk:hover { text-decoration: underline; }
  .mk { align-self: center; margin-block: var(--s-4) var(--s-1); color: var(--ink-2); font-size: var(--text-label); } .mk time, .mk.sys span { display: inline-block; padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); }
  .un { display: flex; align-items: center; gap: var(--s-3); margin-block: var(--s-4) var(--s-1); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); }
  /* The separator leaves once its messages are read: it fades and gathers its height over --dur-slow on --ease-out. */
  .un { max-height: 3rem; overflow: clip; transition: opacity var(--dur-slow) var(--ease-out), max-height var(--dur-slow) var(--ease-out), margin var(--dur-slow) var(--ease-out); } .un.gone { opacity: 0; max-height: 0; margin-block: 0; }
  .un::before, .un::after { content: ""; flex: 1; height: 1px; background: color-mix(in oklab, var(--accent) 40%, transparent); }
</style>
