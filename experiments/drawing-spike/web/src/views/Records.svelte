<script lang="ts">
  // The generic records component: draws one record file's window as the viewer, with no words of any app in it. From the host
  // it uses only the record's stamp (person, run, time), its body, the `edited` and `redacted` flags, the viewer's stored
  // position and unread count, and quotes resolved as the viewer. From the kind it uses the record look (`body`, `quote`) and
  // the page's `actions` (filtered by `when`), and fills the composer slot with the viewer's picked part.
  // Draws: own records at the end side; consecutive records of one author within five minutes grouped under one name; day
  // lines; the unread line after the stored position (opened there); quotes (nothing for a redacted one); the edited mark;
  // redacted records as nothing; agents as "<run> for <person>" with the run's mark; paging upward anchored on the first
  // visible record; jump to the latest with the unread count. Motion: new rows rise in over --dur-base on --ease-out.
  import { onMount, tick } from "svelte";
  import { Icon, Menu, type MenuItem } from "@calternal/ui";
  import { opOf, partFor, pluginOf, viewer, type Json, type Rec, type RecordWindow, type Ref, type Row } from "../lib/model";
  import { clock, dayKey, dayLine, when, type Action } from "../lib/look";
  import { glyph } from "../lib/icons";
  import Av from "./Av.svelte";
  import Inline from "./Inline.svelte";
  import Composer from "./Composer.svelte";
  let { row, win, page, onaction }: { row: Row; win: RecordWindow | null; page: Json; onaction: (a: Action, prefill?: Record<string, string>) => void } = $props();
  const kind = $derived(pluginOf(row.kind)?.kinds?.[row.kind.split("/")[1]!] ?? {});
  const look = $derived(kind.record ?? { body: "body" });
  const slot = $derived(`${row.kind.split("/")[0]}/${page.composer}`);
  const part = $derived(page.composer ? partFor(slot) : null);

  let loaded = $state<{ records: Rec[]; refs: Record<string, Ref>; before: string | null }>({ records: [], refs: {}, before: null });
  $effect.pre(() => { if (win) loaded = { records: [...win.page.records], refs: { ...win.page.refs }, before: win.page.before }; });

  const own = (r: Rec) => r.stamp.person.id === viewer.id && !r.stamp.run;
  const who = (r: Rec) => (r.stamp.run ? `${r.stamp.run.name}:${r.stamp.person.id}` : r.stamp.person.id);
  type Line = { t: "day"; key: string; text: string } | { t: "unread"; key: string; n: number } | { t: "rec"; key: string; r: Rec; first: boolean; last: boolean };
  const lines = $derived.by(() => {
    const out: Line[] = [], shown = loaded.records.filter((r) => !r.redacted);
    let prevDay = "";
    shown.forEach((r, i) => {
      const d = dayKey(r.stamp.at), p = shown[i - 1], n = shown[i + 1];
      if (d !== prevDay) { out.push({ t: "day", key: `d${d}`, text: dayLine(r.stamp.at) }); prevDay = d; }
      const near = (a?: Rec, b?: Rec) => !!a && !!b && who(a) === who(b) && dayKey(a.stamp.at) === dayKey(b.stamp.at) && Math.abs(Date.parse(b.stamp.at) - Date.parse(a.stamp.at)) < 300_000;
      const afterPos = p && win && p.anchor === win.position;
      if (afterPos && win!.unread > 0) out.push({ t: "unread", key: "unread", n: win!.unread });
      out.push({ t: "rec", key: r.anchor, r, first: !near(p, r) || !!afterPos, last: !near(r, n) || (!!win && r.anchor === win.position) });
    });
    // the position may sit on a redacted or hidden record: then the line goes after the last record at or before it
    return out;
  });

  const actions = $derived<Action[]>(Object.entries<Json>(page.actions ?? {}).map(([id, a]) => ({
    id, label: a.op ? (opOf(a.op)?.title ?? id) : id[0]!.toUpperCase() + id.slice(1), place: a.place ?? "menu", op: a.op ?? "", inputs: a.inputs ?? {},
    confirm: a.confirm, defaults: a.defaults, pick: a.pick, compose: a.compose, icon: a.compose ? "quote" : undefined, when: a.when,
  } as Action & { when?: string })));
  /** An action's inputs with the value sources resolved for one record: this (the open item), record, records.selection. */
  const bind = (a: Action, r: Rec): Action => ({ ...a, inputs: Object.fromEntries(Object.entries(a.inputs).map(([k, src]) => [k, src === "this" ? row.item : src === "record" ? r.anchor : src === "records.selection" ? [r.anchor] : src])) });
  const fills = (a: Action, r: Rec) => Object.fromEntries(Object.entries(a.defaults ?? {}).map(([k, src]) => [k, src === "record.body" ? r.body : ""]));
  let menu = $state<{ r: Rec; el: HTMLElement } | null>(null);
  const menuItems = $derived<MenuItem[]>(menu ? actions.filter((a) => when((a as Json).when, {}, own(menu!.r))).map((a) => ({ id: a.id, label: a.label, icon: a.compose ? "quote" : a.confirm ? "trash" : undefined, danger: !!a.confirm })) : []);
  let quoting = $state<{ anchor: string; author: string; excerpt: string } | null>(null);
  function pick(id: string) {
    const m = menu; menu = null; if (!m) return;
    const a = actions.find((x) => x.id === id)!;
    if (a.compose) { quoting = { anchor: m.r.anchor, author: author(m.r), excerpt: m.r.body.split("\n")[0]! }; return; }
    onaction(bind(a, m.r), fills(a, m.r));
  }
  const author = (r: Rec) => r.stamp.run?.name ?? r.stamp.person.name;

  let scroller = $state<HTMLElement>(), top = $state<HTMLElement>(), atEnd = $state(true), loading = false;
  const near = () => { if (scroller) atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40; };
  async function older() {
    if (loading || !loaded.before || !win || !scroller) return;
    loading = true;
    const p = win.pages[loaded.before]!, h = scroller.scrollHeight, y = scroller.scrollTop;
    loaded = { records: [...p.records, ...loaded.records], refs: { ...p.refs, ...loaded.refs }, before: p.before };
    await tick(); scroller.scrollTop = y + (scroller.scrollHeight - h); loading = false; // keep the first visible record in place
  }
  onMount(() => {
    void tick().then(() => {
      const u = scroller?.querySelector<HTMLElement>("[data-unread]");
      if (scroller) scroller.scrollTop = u ? Math.max(0, u.offsetTop - scroller.clientHeight / 3) : scroller.scrollHeight;
      near();
      const io = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) void older(); }, { root: scroller, rootMargin: "200px 0px 0px 0px" });
      if (top) io.observe(top);
    });
  });
  const toEnd = () => scroller?.scrollTo({ top: scroller.scrollHeight, behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
</script>

<section class="cv" aria-label={row.values.title as string}>
  <div class="tr" bind:this={scroller} onscroll={near} role="log" aria-label="Records">
    <div bind:this={top} class="top">{#if loaded.before}<span class="ld">Loading earlier…</span>{/if}</div>
    {#if !win}<p class="empty">Nothing here yet.</p>{/if}
    {#each lines as l (l.key)}
      {#if l.t === "day"}<div class="mk"><time>{l.text}</time></div>
      {:else if l.t === "unread"}<div class="un" data-unread role="separator"><span>{l.n} unread</span></div>
      {:else}
        {@const r = l.r}
        {@const q = look.quote ? r.fields[look.quote] : null}
        {@const ref = q ? loaded.refs[q] : null}
        <article class="e" class:own={own(r)} class:first={l.first} class:last={l.last} data-rec={r.anchor} aria-label={`${author(r)}, ${clock(r.stamp.at)}`}
          oncontextmenu={(e) => { e.preventDefault(); menu = { r, el: e.currentTarget as HTMLElement }; }}>
          {#if l.first && !own(r)}
            <header class="eh">
              {#if r.stamp.run}<span class="mark" data-icon={r.stamp.run.icon}><Icon name={glyph(r.stamp.run.icon)} inline /></span>{:else}<Av name={r.stamp.person.name} size={18} />{/if}
              <span class="nm">{author(r)}</span>{#if r.stamp.run}<span class="for">for {r.stamp.person.id === viewer.id ? "you" : r.stamp.person.name}</span>{/if}
            </header>
          {/if}
          <div class="bub">
            {#if ref && !("gone" in ref)}<button type="button" class="qt" onclick={() => scroller?.querySelector(`[data-rec="${q}"]`)?.scrollIntoView({ block: "center", behavior: "smooth" })}><span class="qa">{ref.author}</span><span class="qx">{ref.excerpt}</span></button>{/if}
            <Inline body={r[look.body as "body"] ?? r.body} />
            <span class="meta">{#if r.edited}<span class="ed">edited</span>{/if}<time>{clock(r.stamp.at)}</time></span>
          </div>
          <button type="button" class="more" aria-label="Actions" onclick={(e) => (menu = { r, el: e.currentTarget as HTMLElement })}><Icon name="more" inline /></button>
        </article>
      {/if}
    {/each}
  </div>
  {#if !atEnd}<button type="button" class="jb" aria-label={`Jump to the latest${win?.unread ? `, ${win.unread} unread` : ""}`} onclick={toEnd}><Icon name="down" inline />{#if win?.unread}<span class="bd">{win.unread}</span>{/if}</button>{/if}
  {#if part?.composer}<Composer {part} {row} bind:quoting />{:else if page.composer}<p class="miss">The part for this place is off. Turn on {pluginOf(row.kind)?.slots?.[page.composer]?.recommended} in Settings.</p>{/if}
  <Menu open={menu !== null} items={menuItems} label="Record actions" onpick={pick} onclose={() => (menu = null)} trigger={menu?.el ?? null} />
</section>

<style>
  .cv { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; height: 100%; }
  .tr { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; padding: 0 var(--s-6) var(--s-4); overflow-anchor: none; }
  .top { min-height: 24px; display: grid; place-items: center; } .ld { color: var(--ink-3); font-size: var(--text-label); padding: var(--s-3); }
  .empty { color: var(--ink-3); text-align: center; margin: auto; }
  .mk { align-self: center; margin-block: var(--s-4) var(--s-2); color: var(--ink-2); font-size: var(--text-label); } .mk time { display: inline-block; padding: var(--s-2) var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); }
  .un { display: flex; align-items: center; gap: var(--s-4); margin-block: var(--s-5) var(--s-2); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); } .un::before, .un::after { content: ""; flex: 1; height: 1px; background: color-mix(in oklab, var(--accent) 40%, transparent); }
  .e { display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-1); margin-top: var(--s-1); max-width: min(82%, 38rem); position: relative; animation: rise var(--dur-base) var(--ease-out); } .e.first { margin-top: var(--s-5); } .e.own { align-self: flex-end; align-items: flex-end; }
  @keyframes rise { from { opacity: 0; translate: 0 6px; } } @media (prefers-reduced-motion: reduce) { .e { animation: none; } }
  .eh { display: flex; align-items: center; gap: var(--s-3); padding-inline: var(--s-4); font-size: var(--text-label); } .nm { font-weight: var(--strong-weight); } .for { color: var(--ink-3); }
  .mark { display: inline-grid; place-items: center; width: 18px; height: 18px; border-radius: 50%; background: oklch(0.78 0.1 45); color: #3a1d0c; font-size: 11px; } .mark[data-icon="codex"] { background: #1c1c1e; color: #fff; }
  .bub { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-3); padding: var(--s-4) var(--s-5) var(--s-6); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); min-width: 7rem; max-width: 100%; box-sizing: border-box; }
  .e:not(.own):not(.first) .bub { border-top-left-radius: var(--r-select); } .e:not(.own):not(.last) .bub { border-bottom-left-radius: var(--r-select); }
  .e.own .bub { background: var(--accent-soft); } .e.own:not(.first) .bub { border-top-right-radius: var(--r-select); } .e.own:not(.last) .bub { border-bottom-right-radius: var(--r-select); }
  .qt { position: relative; display: grid; gap: 2px; align-self: stretch; text-align: start; padding: var(--s-2) var(--s-4) var(--s-2) calc(var(--s-4) + 3px); border-radius: 0 var(--r-select) var(--r-select) 0; background: color-mix(in oklab, var(--accent) 12%, transparent); } .qt::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; width: 3px; background: var(--accent); }
  .qa { font-weight: var(--strong-weight); font-size: var(--text-label); color: var(--accent); } .qx { color: var(--ink-2); font-size: var(--text-label); line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .meta { position: absolute; right: var(--s-5); bottom: var(--s-2); display: inline-flex; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; white-space: nowrap; } .ed { font-style: italic; }
  .more { position: absolute; top: 50%; translate: 0 -50%; inset-inline-end: -30px; width: 26px; height: 26px; display: grid; place-items: center; border-radius: 50%; color: var(--ink-3); opacity: 0; } .e.own .more { inset-inline-end: auto; inset-inline-start: -30px; }
  .e:hover .more, .more:focus-visible { opacity: 1; } @media (hover: none) { .more { display: none; } }
  .jb { position: absolute; right: var(--s-6); bottom: 96px; width: 36px; height: 36px; display: grid; place-items: center; border-radius: 50%; background: var(--paper, #fff); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 4px 14px rgb(0 0 0 / 0.12); } :global(html[data-scheme="dark"]) .jb { --paper: #3a3a3d; }
  .bd { position: absolute; top: -6px; right: -6px; min-width: 18px; height: 18px; padding: 0 5px; box-sizing: border-box; border-radius: 9px; display: grid; place-items: center; background: var(--accent-base); color: #fff; font-size: 11px; }
  .miss { margin: var(--s-5); color: var(--warn, var(--ink-2)); }
  @media (max-width: 719px) { .tr { padding-inline: var(--s-4); } .e { max-width: 86%; } }
</style>
