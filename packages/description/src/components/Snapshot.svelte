<script lang="ts">
  // The snapshot nodes of a checked item view (docs/description-contract.md 5.1 and "Entry views"): stack, text (literal, or the closed inline subset), code, heading, image and attachment, and for a conversation the
  // entry (a message: author, time, edited mark, "asked by", queued or failed, its content), the date and system markers, a checked quote and an item link. Consecutive entries with the same author key within five minutes
  // and no marker between them group under one name. Own entries and a provider mark come only from the host's `entry_controls`, never from page content. Text and code are literal: nothing here makes HTML, and a node type
  // this renderer does not know draws nothing.
  import { Icon } from "@calternal/ui";
  import type { SnapshotNode } from "../draft";
  import { knownIcon } from "../render/icons";
  import { parseInline } from "./inline";
  import { useEntry } from "./entry-context";
  import EntryRequest from "./EntryRequest.svelte";
  import Snapshot from "./Snapshot.svelte";
  let { node, first = true, entry }: { node: SnapshotNode; first?: boolean; /** The anchor of the entry this node sits in. */ entry?: string } = $props();
  const ctx = useEntry();
  const t = (ts: string | undefined): number => (ts ? new Date(ts).getTime() : NaN);
  /** The viewer's local time for a UTC instant, in the viewer's locale: the time today, the date and time otherwise. */
  function local(ts: string | undefined): string {
    if (!ts) return "";
    const d = new Date(ts); if (Number.isNaN(d.getTime())) return "";
    const same = new Date().toDateString() === d.toDateString();
    return new Intl.DateTimeFormat(undefined, same ? { timeStyle: "short" } : { dateStyle: "medium", timeStyle: "short" }).format(d);
  }
  /** The day a date marker names, in the viewer's locale: Today, Yesterday, or the date. */
  function day(ts: string): string {
    const d = new Date(ts), n = new Date(), y = new Date(n.getTime() - 86400000);
    if (d.toDateString() === n.toDateString()) return "Today";
    if (d.toDateString() === y.toDateString()) return "Yesterday";
    return new Intl.DateTimeFormat(undefined, { dateStyle: "long" }).format(d);
  }
  /** Whether the child at `i` starts a group of entries: no entry before it from the same author within five minutes. */
  function startsGroup(kids: SnapshotNode[], i: number): boolean {
    const a = kids[i], b = kids[i - 1];
    if (a?.type !== "snapshot-entry" || b?.type !== "snapshot-entry") return true;
    return a.author.key !== b.author.key || t(a.timestamp) - t(b.timestamp) > 5 * 60_000 || t(a.timestamp) < t(b.timestamp);
  }
  const quoteOf = (e: string | undefined, span: string) => ctx?.state.reply?.quotes?.find((q) => q.entry === e && q.source.span === span);
  const unread = $derived(node.type === "snapshot-entry" && ctx?.state.reply?.reading?.first_unread === node.anchor ? ctx.state.reply.reading.unread : undefined);
  const control = $derived(node.type === "snapshot-entry" ? ctx?.state.controlOf(node.anchor) : undefined);
  const mark = $derived(control?.provider_icon ? knownIcon(control.provider_icon) ?? "threads" : undefined);
  const picked = $derived(node.type === "snapshot-entry" && !!ctx?.state.picked.length && inPick(node.anchor));
  function inPick(anchor: string): boolean { const p = ctx!.state.picked; if (p.length === 1) return p[0] === anchor; const ids = [...(ctx!.state.reply ? anchors() : [])]; const a = ids.indexOf(p[0]!), b = ids.indexOf(p[1]!), i = ids.indexOf(anchor); return i >= Math.min(a, b) && i <= Math.max(a, b); }
  function anchors(): string[] { const out: string[] = []; const walk = (n: SnapshotNode | undefined) => { if (!n) return; if (n.type === "snapshot-entry") out.push(n.anchor); else if (n.type === "stack") n.children.forEach(walk); }; walk(ctx?.state.reply?.view?.content); return out; }
  const state = $derived(node.type === "snapshot-entry" ? node.state : undefined);
</script>

{#snippet inline(text: string)}{#each parseInline(text) as s, k (k)}{#if s.t === "strong"}<strong>{s.text}</strong>{:else if s.t === "em"}<em>{s.text}</em>{:else if s.t === "code"}<code class="ic">{s.text}</code>{:else if s.t === "link"}<a href={s.href} target="_blank" rel="noopener noreferrer nofollow">{s.text}</a>{:else}{s.text}{/if}{/each}{/snippet}

{#if node.type === "stack"}
  <div class="s" class:row={node.axis === "horizontal"} data-snapshot="stack">{#if node.title}<h3 class="st">{node.title}</h3>{/if}{#each node.children as c, k (k)}<Snapshot node={c} first={startsGroup(node.children, k)} />{/each}</div>
{:else if node.type === "snapshot-marker"}
  {#if node.kind === "date"}<div class="mk" data-snapshot="marker" data-marker="date"><time datetime={node.timestamp}>{day(node.timestamp)}</time></div>
  {:else}<div class="mk sys" data-snapshot="marker" data-marker="system"><span>{node.text}</span></div>{/if}
{:else if node.type === "snapshot-entry"}
  {#if unread !== undefined || ctx?.state.reply?.reading?.first_unread === node.anchor}<div class="un" data-unread-separator role="separator"><span>{unread ? `${unread} unread message${unread === 1 ? "" : "s"}` : "Unread messages"}</span></div>{/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events -->
  <article class="e" class:own={!!control?.own} class:first class:picked class:selecting={ctx?.state.selecting} class:queued={state === "queued"} data-entry data-anchor={node.anchor} data-own={control?.own || undefined} data-author={node.author.key}
    oncontextmenu={(ev) => { if (ctx && control?.actions?.length) { ev.preventDefault(); ctx.menu(node.anchor, { x: ev.clientX, y: ev.clientY }); } }}
    onclick={() => { if (ctx?.state.selecting) ctx.pick(node.anchor); }}>
    {#if first && !control?.own}<header class="eh"><span class="nm">{node.author.name}</span>{#if mark}<span class="pm" data-provider-mark title="Agent"><Icon name={mark} inline /></span>{/if}{#if node.asked_by}<span class="ask" data-asked-by>asked by {node.asked_by}</span>{/if}</header>{/if}
    <div class="bub">
      {#each node.children as c, k (k)}
        {#if c.type === "snapshot-quote"}
          {@const q = quoteOf(node.anchor, c.source.span)}
          {#if q}<button type="button" class="qt" data-quote data-target={q.target?.anchor} disabled={!q.target} onclick={(ev) => { ev.stopPropagation(); if (q.target) ctx?.jump(q.target.anchor); }}>{#if q.author}<span class="qa">{q.author}</span>{/if}<span class="qx">{q.text}</span></button>{/if}
        {:else if c.type === "snapshot-link"}
          <button type="button" class="lk" data-item-link onclick={(ev) => { ev.stopPropagation(); ctx?.link(c.target); }}>{c.label}</button>
        {:else}<Snapshot node={c} entry={node.anchor} />{/if}
      {/each}
      <footer class="ef"><time datetime={node.timestamp}>{local(node.timestamp)}</time>{#if node.edited_at}<span class="ed" data-edited title={`Edited ${local(node.edited_at)}`}>edited</span>{/if}{#if state === "queued"}<span class="sd" data-state-mark>Sending…</span>{:else if state === "failed"}<span class="sd bad" data-state-mark role="alert">Not sent</span>{/if}
        {#if ctx && control?.actions?.length && !ctx.state.selecting}<button type="button" class="more" data-entry-menu aria-label="Message actions" onclick={(ev) => { ev.stopPropagation(); const r = (ev.currentTarget as HTMLElement).getBoundingClientRect(); ctx.menu(node.anchor, { x: r.left, y: r.bottom }); }}>⋯</button>{/if}</footer>
    </div>
    {#each ctx?.state.reply?.requests?.filter((r) => r.anchor === node.anchor) ?? [] as r (r.card.request_ref)}<EntryRequest card={r.card} />{/each}
  </article>
{:else if node.type === "snapshot-heading"}
  <div class="h" data-snapshot="heading" data-anchor={node.anchor}><strong>{node.title}</strong>{#if node.author}<span class="a">{node.author}</span>{/if}{#if node.timestamp}<time datetime={node.timestamp}>{local(node.timestamp)}</time>{/if}</div>
{:else if node.type === "snapshot-text"}
  <p class="t {node.style ?? 'body'} {node.tone ?? ''}" data-snapshot="text" data-anchor={node.anchor}>{#if node.markup === "inline"}{@render inline(node.text)}{:else}{node.text}{/if}</p>
{:else if node.type === "snapshot-code"}
  <pre class="c" data-snapshot="code" data-anchor={node.anchor}><code>{node.text}</code></pre>
{:else if node.type === "snapshot-attachment"}
  <div class="att" data-snapshot="attachment" data-anchor={node.anchor} role="group" aria-label={node.label}>{node.label}</div>
{:else if node.type === "snapshot-image"}
  <div class="att" data-snapshot="image" data-anchor={node.anchor} role="img" aria-label={node.alt ?? "Image"}>{node.alt ?? "Image"}</div>
{/if}

<style>
  .s { display: flex; flex-direction: column; gap: var(--s-1); } .s.row { flex-direction: row; flex-wrap: wrap; gap: var(--s-3); }
  .st { margin: 0; font-size: var(--text-label); color: var(--ink-3); font-weight: var(--strong-weight); }
  .h { display: flex; flex-wrap: wrap; align-items: baseline; gap: var(--s-3); margin-top: var(--s-4); }
  .h .a { color: var(--ink-2); } .h time { color: var(--ink-3); font-size: var(--text-label); margin-inline-start: auto; }
  .t { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; } .t.title { font-size: var(--text-title, 28px); font-weight: var(--strong-weight); } .t.heading { font-weight: var(--strong-weight); } .t.caption { color: var(--ink-3); font-size: var(--text-label); }
  .t.good { color: var(--good); } .t.warn { color: var(--warn); } .t.bad { color: var(--danger); }
  .ic { font-family: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, "DejaVu Sans Mono", monospace; font-size: 0.92em; padding: 0 var(--s-1); border-radius: 4px; background: color-mix(in oklab, var(--ink) 9%, transparent); }
  .c { margin: 0; padding: var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 7%, transparent); overflow: auto; font-size: var(--text-label); font-family: ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, "DejaVu Sans Mono", monospace; }
  .att { padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 7%, transparent); width: fit-content; max-width: 100%; }
  /* entries: one bubble per message, a name above the first of a group, own messages to the end in the accent tint */
  .e { display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-1); margin-top: var(--s-1); max-width: min(100%, 44rem); scroll-margin: var(--s-6); position: relative; }
  .e.first { margin-top: var(--s-4); } .e.own { align-self: flex-end; align-items: flex-end; }
  .e.selecting { cursor: pointer; } .e.picked { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: var(--r-select); }
  :global(.hl) { animation: hl var(--dur-slow, 900ms) var(--ease-out, ease-out); } @keyframes hl { from { background: color-mix(in oklab, var(--accent) 30%, transparent); } to { background: transparent; } }
  @media (prefers-reduced-motion: reduce) { :global(.hl) { animation: none; } }
  .eh { display: flex; align-items: center; gap: var(--s-2); padding-inline: var(--s-2); } .nm { font-weight: var(--strong-weight); } .ask, .pm { color: var(--ink-3); font-size: var(--text-label); } .pm { display: inline-grid; }
  .bub { display: flex; flex-direction: column; gap: var(--s-2); padding: var(--s-3) var(--s-4); border-radius: var(--r-select); background: color-mix(in oklab, var(--ink) 8%, transparent); min-width: 0; max-width: 100%; }
  .own .bub { background: color-mix(in oklab, var(--accent) 22%, transparent); }
  .queued .bub { opacity: 0.6; }
  .ef { display: flex; align-items: center; gap: var(--s-3); justify-content: flex-end; color: var(--ink-3); font-size: var(--text-label); }
  .ed, .sd { font-style: italic; } .sd.bad { color: var(--danger); font-style: normal; }
  .more { min-width: 28px; min-height: 28px; border-radius: 14px; color: var(--ink-2); }
  .qt { display: grid; gap: 2px; text-align: start; padding: var(--s-2) var(--s-3); border-inline-start: 3px solid var(--accent); border-radius: 4px; background: color-mix(in oklab, var(--ink) 6%, transparent); color: inherit; }
  .qt:disabled { cursor: default; } .qa { font-weight: var(--strong-weight); font-size: var(--text-label); } .qx { display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; color: var(--ink-2); }
  .t a { color: var(--accent); text-decoration: underline; }
  .lk { width: fit-content; color: var(--accent); text-decoration: underline; text-align: start; }
  .mk { align-self: center; margin-block: var(--s-4) var(--s-1); color: var(--ink-3); font-size: var(--text-label); } .mk time, .mk.sys span { padding: var(--s-1) var(--s-3); border-radius: 999px; background: color-mix(in oklab, var(--ink) 8%, transparent); }
  .un { display: flex; align-items: center; gap: var(--s-3); margin-block: var(--s-4) var(--s-1); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .un::before, .un::after { content: ""; flex: 1; height: 1px; background: color-mix(in oklab, var(--accent) 50%, transparent); }
</style>
