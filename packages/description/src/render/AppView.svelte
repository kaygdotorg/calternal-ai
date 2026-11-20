<script lang="ts">
  // The root of a drawn app: sidebar (with selectors and footer), header, the view's content and the panel, over one open host session. It keeps one WindowStore per (bind, parent) and one Selection per bind, shared by every
  // place that draws the bind, holds the route parameters (selectors and row `open` set them; binds that read a route parameter start over when it changes), and refuses a description it cannot draw. It holds no app-specific
  // code: what is drawn is only what the description declares; the page supplies the host components and the shell's own controls.
  import { onDestroy, onMount, setContext, untrack, type Component } from "svelte";
  import { Card, Icon, Toasts, navHold } from "@calternal/ui";
  import type { Action, AppDescription, Bind, ContentNode } from "../model";
  import type { HostSession } from "../wire/session";
  import { refusalOf } from "../check";
  import { WindowStore } from "../windows/window-store.svelte";
  import { Selection } from "../windows/selection.svelte";
  import { ActionUi } from "../invoke/actions.svelte";
  import { provideActions, IMAGES_KEY, APP_KEY, type AppContext } from "../invoke/context";
  import ActionHost from "../invoke/ActionHost.svelte";
  import Sidebar from "./Sidebar.svelte";
  import Node from "./Node.svelte";
  import HostComponent from "./HostComponent.svelte";
  import { hostComponents } from "../components/registry";
  import { ItemViews } from "../components/entry-state.svelte";
  import { ImageCache } from "./images";
  import ActionButton from "./ActionButton.svelte";

  // `description` and `session` are fixed for the life of one AppView (a new description or session is a new mount): they are read once.
  let props: {
    description: AppDescription; session: HostSession;
    /** The icon each kind declares (the host catalogue's for host kinds, a manifest's for a plugin's), for rows whose icon is `kindIcon`. */
    kindIcons?: Record<string, string>; role?: "viewer" | "editor" | "manager" | "owner"; initial?: string;
    /** The host components of the page (item editor, transcript, composer, request card, sealed frame), by type. */
    components?: Record<string, Component<{ config: unknown; id: string }>>;
    /** The route changed (a selector, a row that opens). */
    onroute?: (name: string, value: string) => void;
    /** A shell action (route, reload, sign out, sheet), or the app switcher. */
    onshell?: (action: unknown) => void; openApp?: (id: string) => void;
  } = $props();
  const { description, session } = untrack(() => ({ description: props.description, session: props.session }));

  const route = $state<Record<string, string>>({});
  for (const s of description.chrome?.sidebar?.selectors ?? []) route[s.route ?? s.id] = s.selected ?? s.options[0]!.id;

  const setRoute = (name: string, value: string) => { if (route[name] === value) return; route[name] = value; sync(); props.onroute?.(name, value); };
  const ui = new ActionUi(session);
  ui.onok = () => { for (const st of stores.values()) st.refresh(); };
  ui.onroute = (name, created) => setRoute(name, created);
  let ctxVersion = $state(0);
  session.oncontexts = () => { ctxVersion++; };
  const viewContext = (id: string): string | undefined => { void ctxVersion; return session.viewContexts[id]; };
  provideActions({ ui, session, viewContext, onshell: (a) => props.onshell?.(a) });
  const images = new ImageCache((ref, signal) => session.image(ref, signal), { hold: navHold }); // reads wait while a key repeats and stop when a burst begins
  setContext(IMAGES_KEY, (ref: string, signal?: AbortSignal) => images.get(ref, signal));
  const refusal = refusalOf(description);
  const binds = new Map<string, Bind>((description.binds ?? []).map((b) => [b.id, b]));
  const stores = new Map<string, WindowStore>();
  const deps = new Map<string, string[]>();
  /** The route parameters a bind's query reads: when one changes the bind starts over. */
  const routeDeps = (b: Bind): string[] => [...JSON.stringify([b.source, b.filter]).matchAll(/"route":"([a-z0-9_]+)"/g)].map((m) => m[1]!);
  // Only the parameters some bind reads go to the host with its windows (a selector or an open item no bind reads is the shell's own state).
  const read = new Set([...binds.values()].flatMap(routeDeps));
  const sync = () => { session.route = Object.fromEntries(Object.entries(route).filter(([k]) => read.has(k))); };
  sync();
  /** The store of a bind (per parent ref for a child bind), made on first use. */
  function storeFor(id: string, parent?: string): WindowStore | undefined {
    const b = binds.get(id);
    if (!b) return undefined;
    const key = `${id}|${parent ?? ""}`;
    let s = stores.get(key);
    if (!s) { s = new WindowStore({ bind: b, source: session, parent }); stores.set(key, s); deps.set(key, routeDeps(b)); }
    return s;
  }
  // A route parameter changed: the binds that read it ask again from the start.
  let last: Record<string, string> = { ...route };
  $effect(() => {
    const now = { ...route };
    untrack(() => { const changed = Object.keys(now).filter((k) => now[k] !== last[k]); last = now; if (changed.length) for (const [k, s] of stores) if ((deps.get(k) ?? []).some((d) => changed.includes(d))) s.reset(); });
  });
  const selections = new Map<string, Selection>();
  const selectionFor = (id: string): Selection => { let x = selections.get(id); if (!x) { x = new Selection(id, session); selections.set(id, x); } return x; };
  const kindIconFor = (id: string): string | undefined => { const k = binds.get(id)?.source.kind; return k ? props.kindIcons?.[k] : undefined; };
  const app: AppContext = { itemViews: new ItemViews(), storeFor, selectionFor, kindIconFor, route, setRoute, get components() { return { ...hostComponents, ...(props.components ?? {}) } as AppContext["components"]; }, onshell: (a) => props.onshell?.(a), openApp: (id) => props.openApp?.(id) };
  setContext(APP_KEY, app);
  session.held = () => Object.fromEntries([...stores.values()].filter((s) => s.windowRef && s.resultVersion).map((s) => [s.windowRef!, s.resultVersion!]));
  // The lease: while it has lapsed the rows are not drawn as current and offer no contexts; a renewal repaints only the windows it names as retained and may bring replacement windows (section 6).
  session.oncurrent = (current) => { if (!current) { for (const s of stores.values()) s.setCurrent(false); images.clear(); } }; // a renewal never un-hides by itself: `renewed` (retained windows) or a fresh reply does
  session.onrenewed = (r) => { for (const s of stores.values()) { s.renewed(r.retained_results); for (const w of r.windows) s.apply(w); } };
  onDestroy(() => { for (const s of stores.values()) s.destroy(); images.destroy(); session.close(); });

  /** The bind whose ticked rows a header action takes as a host-issued selection reference, if its inputs name one. */
  const setBind = (a: Action): string | undefined => { for (const v of Object.values(("inputs" in a ? a.inputs : undefined) ?? {})) { const x = v as { selection?: string; set?: boolean }; if (x.selection && x.set) return x.selection; } return undefined; };
  const header = description.chrome?.header;
  const title = typeof header?.title === "string" ? header.title : description.view?.title;
  // A header title taken from an item bind's field: the open item's, never a list's first row. The store is made here, not inside the derived value: a signal created while a derived runs is not tracked by it, so a title
  // read from a store born there would never update. It asks the host only once an item is open (the route change resets it).
  const titleSpec = header?.title && typeof header.title !== "string" ? header.title : undefined;
  const titleStore = titleSpec ? storeFor(titleSpec.bind) : undefined;
  const itemTitle = $derived(titleSpec && titleStore ? String(titleStore.row(0)?.fields[titleSpec.field] ?? "") : undefined);
  const panel = description.chrome?.panel;
  // A narrow window (a phone) shows one pane at a time: the list while no item is open, the card (with a Back control) once one is.
  let compact = $state(false);
  onMount(() => { const q = matchMedia("(max-width: 720px)"); compact = q.matches; const f = () => { compact = q.matches; }; q.addEventListener("change", f); return () => q.removeEventListener("change", f); });
  const opened = $derived(!!route.item);
  const listPane = $derived(!compact || !opened), cardPane = $derived(!compact || opened);
</script>

{#if refusal}
  <div class="refused" role="alert" data-refused>This app cannot be drawn here: {refusal.reason}.</div>
{:else}
  <!-- The shell's three regions (docs/ui-shell.md): the sidebar and the panel sit flat on the page's background (the wallpaper and its veil, drawn by the page), the middle card is the only surface on top of it. Each region
       carries the secondary inks solved for the pixels under it. A phone shows one at a time. -->
  <div class="app" data-app data-compact={compact || undefined} data-pane={compact ? (opened ? "card" : "list") : undefined}>
    {#if (description.chrome?.sidebar || description.chrome?.footer) && listPane}
      <aside class="side" style:--sw={compact ? "100%" : description.chrome.sidebar?.width ? `${description.chrome.sidebar.width}px` : undefined}>
        <Sidebar sidebar={description.chrome.sidebar ?? {}} footer={description.chrome.footer} role={props.role} initial={props.initial} />
      </aside>
    {/if}
    {#if cardPane}
      <main class="mid">
        <Card label={itemTitle || title || "Content"} variant="pane" class="card">
          <div class="col">
            <header class="head">{#if compact}<button class="back" type="button" aria-label="Back to the list" data-back onclick={() => setRoute("item", "")}><Icon name="back" /></button>{/if}{#if itemTitle ?? title}<h1 class="h1">{itemTitle ?? title}</h1>{/if}
              <div class="hact">{#each [...(header?.leading ?? []), ...(header?.trailing ?? [])] as a (a.id)}{@const sb = setBind(a)}<ActionButton action={a} context={viewContext(a.id)} selection={sb ? selectionFor(sb) : undefined} setOf={sb} />{/each}</div></header>
            {#if description.view}<div class="body"><Node node={description.view.root} /></div>{/if}
          </div>
        </Card>
      </main>
    {/if}
    {#if panel && panel.open !== false && !compact}
      <aside class="right" data-panel={panel.id ?? "panel"} aria-label={panel.title ?? "Panel"}>
        {#if panel.title}<h2 class="ph">{panel.title}</h2>{/if}
        {#each panel.components ?? [] as c (c.id)}<HostComponent component={c} />{/each}
        {#each (panel as unknown as { content?: ContentNode[] }).content ?? [] as n, k (k)}<Node node={n} />{/each}
      </aside>
    {/if}
  </div>
{/if}
<ActionHost {ui} />
<Toasts />

<style>
  /* The shell's layout (apps/web shell/Shell.svelte): flat sidebar and panel on the background, one inset around the card, the card as the packages/ui pane (its own wallpaper copy and veil, concentric radii). */
  .app { display: flex; height: 100%; min-height: 0; box-sizing: border-box; }
  .side { flex: none; width: var(--sw, var(--side-w)); min-height: 0; overflow: clip; --ink-2: var(--l-ink2, currentcolor); --ink-3: var(--l-ink3, currentcolor); }
  .mid { flex: 1; min-width: 0; min-height: 0; padding: 0 var(--inset); --ink-2: var(--c-ink2, currentcolor); --ink-3: var(--c-ink3, currentcolor); }
  .mid :global(.card) { height: 100%; }
  .right { flex: none; width: var(--right-w); min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: var(--s-3); padding: var(--s-4) var(--s-4) var(--s-6); box-sizing: border-box; --ink-2: var(--r-ink2, currentcolor); --ink-3: var(--r-ink3, currentcolor); }
  .col { height: 100%; min-height: 0; display: flex; flex-direction: column; }
  .head { flex: none; height: var(--bar-h); display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--s-4) 0 var(--s-6); }
  .hact { display: flex; align-items: center; gap: var(--s-2); }
  .h1 { margin: 0; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .body { flex: 1; min-height: 0; display: flex; flex-direction: column; padding: 0 var(--s-4) var(--s-4); }
  .ph { margin: var(--s-2) 0 0; padding: 0 var(--s-2); font-size: var(--text-section); font-weight: var(--strong-weight); color: var(--ink-3); }
  .back { flex: none; display: inline-grid; place-items: center; min-width: var(--pill-h); min-height: var(--pill-h); margin-inline-start: calc(var(--s-4) * -1); color: var(--ink); border-radius: 50%; }
  .app[data-compact] .mid { padding: 0 var(--inset); } .app[data-compact] .side { width: 100%; }
  .refused { padding: var(--s-6); color: var(--danger); }
</style>
