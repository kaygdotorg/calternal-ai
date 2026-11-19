<script lang="ts">
  // The root of a drawn app: sidebar from `chrome.sidebar`, header title from `chrome.header`, content from `view.root`, over one open host session. It builds one WindowStore per bind (lazily, shared by every
  // place that draws the bind), holds the selected row, and refuses a description it cannot draw. It holds no app-specific code: what is drawn is only what the description declares.
  import { onDestroy, untrack } from "svelte";
  import type { AppDescription, Bind } from "../model";
  import type { HostSession } from "../wire/session";
  import { refusalOf } from "../check";
  import { WindowStore } from "../windows/window-store.svelte";
  import { Selection } from "../windows/selection.svelte";
  import Sidebar from "./Sidebar.svelte";
  import Node from "./Node.svelte";
  import ActionHost from "../invoke/ActionHost.svelte";
  import { ActionUi } from "../invoke/actions.svelte";
  import { provideActions } from "../invoke/context";
  import ActionButton from "./ActionButton.svelte";

  // `description` and `session` are fixed for the life of one AppView (a new description or session is a new mount): they are read once.
  let props: {
    description: AppDescription; session: HostSession;
    /** The icon each kind declares (the host catalogue's for host kinds, a manifest's for a plugin's), for rows whose icon is `kindIcon`. */
    kindIcons?: Record<string, string>; route?: Record<string, string>; role?: "viewer" | "editor" | "manager" | "owner";
    onopen?: (bind: string, id: string, index: number) => void;
    /** `then: {route}` after an action created an item: set the route parameter `name` to the host-issued `created` ref. */
    onroute?: (name: string, created: string) => void;
    /** A shell action (route, reload, sign out, sheet): the shell's own controls. */
    onshell?: (action: unknown) => void;
  } = $props();
  const { description, session } = untrack(() => ({ description: props.description, session: props.session }));
  const kindIcons = $derived(props.kindIcons ?? {}), route = $derived(props.route ?? {}), role = $derived(props.role);
  const onopen = (b: string, id: string, i: number) => props.onopen?.(b, id, i);

  const ui = new ActionUi(session);
  ui.onok = () => { for (const st of stores.values()) st.refresh(); };
  ui.onroute = (name, created) => props.onroute?.(name, created);
  provideActions({ ui, session, onshell: (a) => props.onshell?.(a) });
  const refusal = refusalOf(description);
  const binds = new Map<string, Bind>((description.binds ?? []).map((b) => [b.id, b]));
  const stores = new Map<string, WindowStore>();
  /** The store of a list bind, made on first use. Only list binds read windows here; item and search binds belong to later steps. */
  function storeFor(id: string): WindowStore | undefined {
    const b = binds.get(id);
    if (!b || b.source.type !== "list") return undefined;
    let s = stores.get(id);
    if (!s) { s = new WindowStore({ bind: b, source: session }); stores.set(id, s); }
    return s;
  }
  const selections = new Map<string, Selection>();
  /** The selection of a bind, shared by every place that draws it (the selected row is that bind's `selection` source). */
  function selectionFor(id: string): Selection { let x = selections.get(id); if (!x) { x = new Selection(id, session); selections.set(id, x); } return x; }
  const kindIconFor = (id: string): string | undefined => { const k = binds.get(id)?.source.kind; return k ? kindIcons[k] : undefined; };
  session.held = () => Object.fromEntries([...stores.values()].filter((s) => s.windowRef && s.resultVersion).map((s) => [s.windowRef!, s.resultVersion!]));
  onDestroy(() => { for (const s of stores.values()) s.destroy(); session.close(); });

  const header = description.chrome?.header?.title;
  const title = typeof header === "string" ? header : description.view?.title;
</script>

{#if refusal}
  <div class="refused" role="alert" data-refused>This app cannot be drawn here: {refusal.reason}.</div>
{:else}
  <div class="app" data-app>
    {#if description.chrome?.sidebar}
      <aside class="side" style:width="{description.chrome.sidebar.width ?? 280}px">
        <Sidebar sidebar={description.chrome.sidebar} {storeFor} {selectionFor} {kindIconFor} {route} {role} {onopen} />
      </aside>
    {/if}
    <main class="main">
      <div class="head">{#if title}<h1 class="h1">{title}</h1>{/if}
        {#each [...(description.chrome?.header?.leading ?? []), ...(description.chrome?.header?.trailing ?? [])] as a (a.id)}<ActionButton action={a} context={session.viewContexts[a.id]} />{/each}</div>
      {#if description.view}<Node node={description.view.root} {storeFor} {selectionFor} {kindIconFor} {onopen} />{/if}
    </main>
  </div>
{/if}
<ActionHost {ui} />

<style>
  .app { display: flex; height: 100%; min-height: 0; }
  .side { flex: none; min-height: 0; border-inline-end: 1px solid color-mix(in oklab, var(--ink) 10%, transparent); }
  .main { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: var(--s-4); padding: var(--s-5); }
  .head { display: flex; align-items: center; gap: var(--s-3); }
  .h1 { margin: 0; font-size: var(--text-title, 28px); }
  .refused { padding: var(--s-6); color: var(--danger); }
</style>
