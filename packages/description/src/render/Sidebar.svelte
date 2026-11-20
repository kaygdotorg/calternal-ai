<script lang="ts">
  // The sidebar of an app description: its title, selectors (a segmented control that sets a route parameter, an option's count from a count bind), sections (literal shortcut rows, a bound list, or one section per row of a
  // layout bind), and the footer (search, apps, account). Bound sections are windowed with `inside`, so the sidebar scrolls once. A section without a title draws no header. `showWhen` hides a section the route or role does
  // not call for. The footer search sends the settled text (250 ms after the last key) and draws the search bind's windows in place of the sections; a cleared field returns to them.
  import { onDestroy } from "svelte";
  import { Menu, Row, SearchField, Selector, type MenuItem } from "@calternal/ui";
  import { shellAllowed } from "../invoke/shell";
  import type { Section, Sidebar, Footer } from "../model";
  import { useApp, useActions } from "../invoke/context";
  import ContentList from "./ContentList.svelte";
  import Tree from "./Tree.svelte";
  import type { SectionD } from "../draft";
  import ActionButton from "./ActionButton.svelte";
  import { knownIcon } from "./icons";
  import { marksOf } from "./eval";

  let { sidebar, footer, role, initial }: { sidebar: Sidebar; footer?: Footer; role?: "viewer" | "editor" | "manager" | "owner"; /** The person's initial, from the session (a template holds none). */ initial?: string } = $props();
  const app = useApp(), { ui, session, viewContext } = useActions();
  const ranks = { viewer: 0, editor: 1, manager: 2, owner: 3 } as const;
  function shown(s: Section): boolean {
    const w = s.showWhen;
    if (!w) return true;
    if ("route" in w) return app.route[w.route] === w.equals;
    return (ranks[role ?? "viewer"] ?? 0) >= ranks[w.minRole];
  }

  // DRAFT: a sidebar whose sections have subsections, a strip or customisation is drawn as a tree the person arranges (Tree.svelte); the others as before.
  const tree = $derived((sidebar.sections ?? []).some((s) => { const d = s as unknown as SectionD; return !!d.sections || !!d.presentation || d.customise !== undefined; }));

  // On a phone the search field and the bar float over the foot of the list (it scrolls beneath them, faded); the list keeps a bottom inset the height of both, so its last rows can always be scrolled clear of them.
  let footEl = $state<HTMLElement>(), footH = $state(0);
  $effect(() => { const f = footEl; if (!f) return; const ro = new ResizeObserver(() => { footH = Math.round(f.getBoundingClientRect().height); }); ro.observe(f); return () => ro.disconnect(); });

  // Search: settled text goes to the host, then the search bind's windows are read again.
  let text = $state(""), timer = 0, sentText = $state(""), seq = 0;
  const searchBind = $derived(footer?.search?.bind);
  function typed() {
    clearTimeout(timer);
    timer = window.setTimeout(async () => {
      if (!searchBind) return;
      const mine = ++seq, t = text.trim();
      sentText = t;
      if (!t) return;
      await session.exchange("search", { type: "search", bind: searchBind, text: t }).catch(() => {});
      if (mine === seq) app.storeFor(searchBind)?.reset(); // only the newest search reads its results: an older answer landing late must not reset the list again
    }, 250);
  }
  onDestroy(() => { clearTimeout(timer); seq++; });
  $effect(() => { if (text.trim() === "") sentText = ""; });
  const searching = $derived(!!searchBind && sentText !== "");

  let accountOpen = $state(false), accountEl = $state<HTMLElement | null>(null);
  const accountItems = $derived<MenuItem[]>((footer?.account?.menu ?? []).map((a) => ({ id: a.id, label: a.title ?? a.id, danger: a.role === "destructive" })));
  // An option's count is a bind with an exact count. Its store is made here, at set-up, not while a template runs (a signal created inside a reaction is not tracked by it, so the label would never update); it is asked
  // for once the selector is drawn (one row is enough), and the label says "Archived 2" when the host gave the count (an unavailable count is omitted).
  const counts = new Map(sidebar.selectors?.flatMap((sel) => sel.options.filter((o) => o.count).map((o) => [o.id, app.storeFor(o.count!.bind)] as const)) ?? []);
  $effect(() => { for (const [id, st] of counts) st?.ensure(`count:${id}`, 0, 1); });
  const options = (sel: NonNullable<Sidebar["selectors"]>[number]) => sel.options.map((o) => { const n = counts.get(o.id)?.total; return { value: o.id, label: n ? `${o.title} ${n}` : o.title }; });
</script>

<nav class="sidebar" aria-label={sidebar.title ?? "Sidebar"} data-sidebar style:--foot-h="{footH}px">
  {#if sidebar.title}<h2 class="title">{sidebar.title}</h2>{/if}
  {#each sidebar.selectors ?? [] as sel (sel.id)}
    <div class="sel" data-selector={sel.id}>
      <Selector options={options(sel)} value={app.route[sel.route ?? sel.id] ?? sel.selected ?? sel.options[0]!.id} label={sel.id} onchange={(v: string) => app.setRoute(sel.route ?? sel.id, v)} />
      {#each sel.options as o (o.id)}{#if o.count}{@const c = counts.get(o.id)}{#if c}<span class="cnt" data-count-of={o.id} hidden>{c.total ?? ""}</span>{/if}{/if}{/each}
    </div>
  {/each}
  <div class="scroll">
    {#if searching && searchBind}
      {@const st = app.storeFor(searchBind)}
      {#if st}<ContentList store={st} selection={app.selectionFor(searchBind)} kindIcon={app.kindIconFor(searchBind)} label="Search results" inside />{/if}
    {:else}
      {#if tree}<Tree sections={(sidebar.sections ?? []) as unknown as SectionD[]} shown={(s) => shown(s as unknown as Section)} />
      {:else}{#each sidebar.sections ?? [] as s (s.id)}
        {#if shown(s)}
          <section class="sec" data-section={s.id}>
            {#if s.title}<h3 class="sh">{s.title}</h3>{/if}
            {#each s.rows ?? [] as r (r.id)}
              {@const icon = typeof r.icon === "string" ? knownIcon(r.icon) : undefined}
              <Row title={r.title} {icon}>{#snippet trailing()}{#each marksOf(r.marks, {}) as m, k (k)}<span data-mark={m.kind}>{m.text ?? ""}</span>{/each}{/snippet}</Row>
            {/each}
            {#if s.bind}
              {@const store = app.storeFor(s.bind)}
              {#if store}<ContentList {store} selection={app.selectionFor(s.bind)} kindIcon={app.kindIconFor(s.bind)} label={s.title ?? s.id} inside />{/if}
            {/if}
            {#if s.repeat}
              {@const over = app.storeFor(s.repeat.over)}
              {#if over}{#each Array.from({ length: Math.min(over.length, over.size) }, (_, i) => i) as i (i)}
                {@const sr = over.row(i)}
                {#if sr}{@const kids = app.storeFor(s.repeat.rows, sr.parent_refs?.[s.repeat.rows])}
                  <section class="sec" data-section-row={sr.id}><h3 class="sh">{String(sr.fields.title ?? "")}</h3>
                    {#if kids}<ContentList store={kids} selection={app.selectionFor(s.repeat.rows)} kindIcon={app.kindIconFor(s.repeat.rows)} label={String(sr.fields.title ?? "")} inside />{/if}</section>
                {/if}
              {/each}{/if}
            {/if}
          </section>
        {/if}
      {/each}{/if}
    {/if}
  </div>
  {#if footer}
    <div class="foot" data-footer bind:this={footEl}>
      {#if footer.search}<SearchField bind:value={text} label={footer.search.placeholder ?? "Search"} placeholder={footer.search.placeholder} oninput={typed} />{/if}
      <div class="bar">
        {#each footer.apps ?? [] as a (a.id)}{@const ic = typeof a.icon === "string" ? knownIcon(a.icon) : undefined}<button type="button" class="app" data-app-entry={a.id} aria-label={a.title} title={a.title} onclick={() => app.openApp?.(a.id)}>{a.title}</button>{/each}
        {#if footer.account}<button type="button" class="acct" bind:this={accountEl} aria-label={footer.account.title ?? "Account"} onclick={() => (accountOpen = true)}>{initial ?? "·"}</button>
          <Menu open={accountOpen} items={accountItems} label={footer.account.title ?? "Account"} trigger={accountEl} onclose={() => (accountOpen = false)} onpick={(id) => { accountOpen = false; const a = (footer.account!.menu ?? []).find((x) => x.id === id); if (a) { if ("operation" in a) void ui.invoker.run(a, { context: viewContext(a.id) }); else if (shellAllowed(a)) app.onshell?.(a); } }} />{/if}
      </div>
    </div>
  {/if}
</nav>

<style>
  .sidebar { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .title { margin: 0; padding: var(--s-5) var(--s-5) var(--s-3); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .sel { padding: 0 var(--s-4) var(--s-3); }
  .scroll { flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding: 0 var(--s-3) var(--s-6); }
  .sh { margin: var(--s-5) 0 var(--s-2); padding: 0 var(--s-5); font-size: var(--text-section); font-weight: var(--strong-weight); color: var(--ink-3); }
  @media (max-width: 720px) {
    .sidebar { position: relative; }
    .scroll { padding-bottom: calc(var(--foot-h, 0px) + var(--s-2) + var(--s-5) + var(--s-4)); -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - var(--foot-h, 0px) - var(--s-5)), transparent calc(100% - var(--foot-h, 0px))); mask-image: linear-gradient(to bottom, #000 calc(100% - var(--foot-h, 0px) - var(--s-5)), transparent calc(100% - var(--foot-h, 0px))); }
    .foot { position: absolute; inset-inline: var(--s-2); bottom: var(--s-2); border-radius: var(--r-card); background: rgb(var(--tint) / 0.82); box-shadow: var(--shadow-float); }
  }
  .foot { display: grid; gap: var(--s-3); padding: var(--s-3) var(--s-4) var(--s-4); }
  .bar { display: flex; align-items: center; gap: var(--s-2); }
  .app { min-height: 44px; padding: 0 var(--s-3); color: var(--ink-2); border-radius: var(--r-select); } .app:hover { background: var(--hover); }
  .acct { margin-inline-start: auto; width: 44px; height: 44px; border-radius: 50%; background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
</style>
