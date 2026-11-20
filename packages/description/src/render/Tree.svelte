<script lang="ts">
  // The sidebar as a list of sections the person arranges (DRAFT shapes, issue #78/#99; flat, no subsections and no indentation): sections with collapsible bodies, one pinned strip of round thumbnails, avatar marks, and the person's own layout of all of it: reorder
  // sections, subsections and rows (by drag, or Move up and Move down in the menu), pin by dragging a row onto the strip and unpin by dragging it off, collapse, hide (with a way back at the foot), rename, and make sections of their own
  // (a folder is simply another section) to drag rows into. The arrangement is the person's and the host's to store (`layout-state.svelte.ts`); a description only says what exists and what may be changed. A row under a section is
  // drawn exactly as everywhere else (RowActions, the bind's template); only the rows of folders and the order are the person's. Motion: folding, a section opening to receive a drag, the insertion line and a drop's
  // rearrangement all run on the motion tokens, in the order drag starts, targets open, line follows, drop lands (FLIP); reduced motion turns them to nothing. Every drag has a menu or keyboard equivalent.
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { Icon, Menu, Row, type MenuItem } from "@calternal/ui";
  import type { Action, RowTemplate, WindowRow } from "../model";
  import type { SectionD } from "../draft";
  import type { WindowStore } from "../windows/window-store.svelte";
  import { useApp, useActions } from "../invoke/context";
  import { Dnd, flip, type Over, type Source } from "./dnd.svelte";
  import { LayoutState } from "./layout-state.svelte";
  import { knownIcon } from "./icons";
  import RowActions from "./RowActions.svelte";
  import Avatar from "./Avatar.svelte";
  import ActionButton from "./ActionButton.svelte";

  let { sections, shown }: { sections: SectionD[]; shown: (s: SectionD) => boolean } = $props();
  const app = useApp(), { ui, session, viewContext } = useActions();
  const layout = new LayoutState();
  onMount(() => { void layout.load(session); });

  // ---- the stores of every bound section, made now (not while drawing) and asked for their rows
  const leaves: SectionD[] = [];
  const collect = (list: SectionD[]) => { for (const s of list) { if (s.bind) leaves.push(s); if (s.sections) collect(s.sections); } };
  collect(untrack(() => sections));
  const stores = new Map<string, WindowStore | undefined>(leaves.map((s) => [s.id, app.storeFor(s.bind!)]));
  const consumer = {};
  $effect(() => { for (const st of stores.values()) st?.ensure(consumer, 0, st.size); return () => { for (const st of stores.values()) st?.ensure(consumer, 0, 0); }; });
  const rowsIn = (id: string): { r: WindowRow; store: WindowStore }[] => {
    const st = stores.get(id); if (!st) return [];
    void st.tick; const out: { r: WindowRow; store: WindowStore }[] = [];
    for (let i = 0; i < Math.min(st.length, st.size); i++) { const r = st.row(i); if (r) out.push({ r, store: st }); }
    return out;
  };
  const templateOf = (st: WindowStore): RowTemplate => st.bind.row ?? { title: { field: "title" } };
  const routeOf = (t: RowTemplate) => (t.open && "route" in t.open ? t.open.route.name : "item");
  /** The rows shown in a section or folder, in the person's order: a folder shows the rows placed in it (from any section beneath its parent); a section hides rows placed in a folder. */
  function shownRows(s: { id: string; bind?: string }, parentId: string): { r: WindowRow; store: WindowStore }[] {
    const folder = layout.folders.some((f) => f.id === s.id);
    let rows: { r: WindowRow; store: WindowStore }[];
    if (folder) rows = leavesUnder(parentId).flatMap((id) => rowsIn(id)).filter((x) => layout.placed[x.r.id] === s.id);
    else rows = rowsIn(s.id).filter((x) => !layout.placed[x.r.id]);
    const order = layout.rowOrder(s.id, rows.map((x) => x.r.id));
    return order.map((id) => rows.find((x) => x.r.id === id)!);
  }
  const topOf = (id: string): string => id;
  const collectIds = (s: SectionD): string[] => (s.sections ?? []).flatMap((c) => [c.id, ...collectIds(c)]);
  const leavesUnder = (_top: string): string[] => leaves.map((l) => l.id);
  const findSection = (id: string): SectionD | undefined => { let f: SectionD | undefined; const walk = (s: SectionD) => { if (s.id === id) f = s; s.sections?.forEach(walk); }; sections.forEach(walk); return f; };
  const folderOf = (id: string) => layout.folders.find((f) => f.id === id);
  const titleOf = (id: string, own?: string) => layout.titles[id] ?? own ?? folderOf(id)?.title ?? "";
  const isFolded = (s: { id: string; collapsed?: boolean }) => layout.collapsed[s.id] ?? s.collapsed ?? false;
  const may = (s: SectionD | undefined, what: "reorder" | "collapse" | "hide" | "rename" | "folders") => (s ? s.customise !== false && (typeof s.customise !== "object" || s.customise[what] !== false) : true);

  // ---- what is shown at each level: the person's order, hidden ones left out
  const visibleTop = $derived(layout.sorted("", [...sections.filter((s) => shown(s) && !layout.hidden[s.id]).map((s) => s.id), ...layout.folders.filter((f) => !layout.hidden[f.id]).map((f) => f.id)]).map((id) => sections.find((s) => s.id === id) ?? { id, folder: true as const }));
  const hiddenAll = $derived([...sections.filter((s) => shown(s) && layout.hidden[s.id]), ...layout.folders.filter((f) => layout.hidden[f.id]).map((f) => ({ id: f.id, title: f.title } as SectionD))]);

  // ---- opening a row, and the action a drop runs
  function openRow(store: WindowStore, r: WindowRow) {
    const t = templateOf(store), o = t.open;
    void app.selectionFor(store.bind.id).settle(r.id);
    if (o && "route" in o) app.setRoute(o.route.name, r.id);
    else if (o && "invoke" in o && "operation" in o.invoke) void ui.invoker.run(o.invoke, { context: store.contexts(r.id)?.[o.invoke.id] });
  }
  const actionsOf = (t: RowTemplate): Action[] => [...(t.leadingActions ?? []), ...(t.trailingActions ?? []), ...(t.menu ?? [])] as Action[];
  /** The row's own action a section's `drop` or `dragOut` names: its id, or an id that begins with it and a dash (`pin`, `pin-people`). */
  const rowAction = (t: RowTemplate, name: string): Action | undefined => actionsOf(t).find((a) => a.id === name || a.id.startsWith(`${name}-`));
  async function runRowAction(store: WindowStore, r: WindowRow, name: string): Promise<boolean> {
    const a = rowAction(templateOf(store), name); if (!a || !("operation" in a)) return false;
    const o = await ui.invoker.run(a, { context: store.contexts(r.id)?.[a.id] });
    return "state" in o && o.state === "ok";
  }
  const findRow = (id: string): { r: WindowRow; store: WindowStore } | undefined => { for (const l of leaves) { const x = rowsIn(l.id).find((y) => y.r.id === id); if (x) return x; } return undefined; };
  const stripSec = (id: string) => findSection(id);

  // ---- drag and drop
  let root = $state<HTMLElement>(), dnd = $state<Dnd>(), line = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  const dragging = $derived(dnd?.active ?? null);
  const accepts = (s: Source, o: Over): boolean => {
    if (s.kind === "section") return o.type === "head" && o.section !== s.id && !findSection(o.section)?.presentation && may(findSection(s.id) ?? undefined, "reorder");
    const src = findRow(s.id); if (!src) return false;
    const fromStrip = !!stripSec(s.section)?.presentation && stripSec(s.section)?.presentation === "strip";
    if (o.type === "strip") { const st = stripSec(o.section); return !fromStrip && !!st?.drop && !!rowAction(templateOf(src.store), st.drop.action); }
    if (o.type === "row") {
      const targetStrip = stripSec(o.in)?.presentation === "strip";
      if (targetStrip) { if (fromStrip && o.in === s.section) return o.id !== s.id; const st = stripSec(o.in); return !fromStrip && !!st?.drop && !!rowAction(templateOf(src.store), st.drop.action); }
      if (fromStrip) { const st = stripSec(s.section); return !!st?.dragOut && !!rowAction(templateOf(src.store), st.dragOut.action); }
      if (o.in === s.section) return o.id !== s.id && may(findSection(o.in), "reorder");
      return !!folderOf(o.in) && may(findSection(topOf(folderOf(o.in)!.parent)), "folders");
    }
    if (o.type === "head" || o.type === "section") {
      if (fromStrip) { const st = stripSec(s.section); return !!st?.dragOut && !!rowAction(templateOf(src.store), st.dragOut.action) && !stripSec(o.section); }
      return (!!folderOf(o.section) && may(findSection(topOf(folderOf(o.section)!.parent)), "folders")) || (!!layout.placed[s.id] && leavesUnder(topOf(o.section)).includes(o.section) && stores.get(o.section) === src.store);
    }
    return false;
  };
  async function drop(s: Source, o: Over) {
    if (!root) return;
    if (s.kind === "section") { const parent = "", ids = visibleTop.map((x) => x.id); const idx = ids.indexOf(o.type === "head" ? o.section : ""); const before = o.type === "head" ? (o.after ? ids[idx + 1] ?? null : o.section) : null; await flip(root, () => layout.moveSection(parent, ids, s.id, before)); return; }
    const src = findRow(s.id); if (!src) return;
    const fromStrip = stripSec(s.section)?.presentation === "strip";
    if (o.type === "strip" || (o.type === "row" && stripSec(o.in)?.presentation === "strip" && !fromStrip)) { const st = stripSec(o.type === "strip" ? o.section : o.in)!; if (await runRowAction(src.store, src.r, st.drop!.action)) { const ids = rowsOfStrip(st.id).map((x) => x.r.id); if (o.type === "row") { const at = ids.indexOf(o.id); layout.rows[st.id] = [...ids.slice(0, o.after ? at + 1 : at), s.id, ...ids.slice(o.after ? at + 1 : at)]; layout.save(); } } return; }
    if (fromStrip && !(o.type === "row" && o.in === s.section)) { const st = stripSec(s.section)!; await runRowAction(src.store, src.r, st.dragOut!.action); return; }
    if (o.type === "row" && o.in === s.section) { const ids = (stripSec(o.in)?.presentation === "strip" ? rowsOfStrip(o.in) : shownRows(findSection(o.in) ?? { id: o.in }, ctxOf(o.in))).map((x) => x.r.id); const at = ids.indexOf(o.id); await flip(root, () => layout.moveRow(o.in, ids, s.id, o.after ? ids[at + 1] ?? null : o.id)); return; }
    const target = o.type === "row" ? o.in : o.section;
    if (folderOf(target)) { const ids = shownRows({ id: target }, folderOf(target)!.parent).map((x) => x.r.id); await flip(root, () => { layout.place(s.id, target); layout.rows[target] = [...ids.filter((x) => x !== s.id), s.id]; }); }
    else await flip(root, () => layout.place(s.id, null));
  }
  const rowsOfStrip = (id: string) => shownRows({ id, bind: findSection(id)?.bind }, id);
  const ctxOf = (id: string): string => id;
  onMount(() => { dnd = new Dnd(root!, { accepts, drop, open: (id) => { if (layout.collapsed[id] ?? findSection(id)?.collapsed) layout.fold(id, false); } }); });
  onDestroy(() => dnd?.destroy());
  // The insertion line follows the target (a transform, so it glides); the targets that can take the drop show it by a class.
  $effect(() => {
    const a = dnd?.active; if (!a?.over || !a.ok || !root) { line = null; return; }
    const o = a.over, b = root.getBoundingClientRect(); let x = o.rect.left - b.left, y = o.rect.top - b.top, w = o.rect.width, h = 2;
    if (o.type === "row" && o.rect.height && stripSec(o.in)?.presentation === "strip") { x = (o.after ? o.rect.right : o.rect.left) - b.left - 1; y = o.rect.top - b.top; w = 2; h = o.rect.height; }
    else if (o.type === "row" || o.type === "head") y = (o.after ? o.rect.bottom : o.rect.top) - b.top - 1;
    else { line = null; return; }
    line = { x, y, w, h };
  });
  const armed = (id: string) => !!dragging?.ok && ((dragging.over?.type === "strip" && dragging.over.section === id) || (dragging.over?.type === "row" && dragging.over.in === id) || ((dragging.over?.type === "head" || dragging.over?.type === "section") && dragging.over.section === id));
  const mayPin = (id: string) => { const st = stripSec(id); const s = dragging?.source; if (!st || !s || s.kind !== "row") return false; const src = findRow(s.id); return !!src && stripSec(s.section)?.presentation !== "strip" && !!st.drop && !!rowAction(templateOf(src.store), st.drop.action); };

  // ---- menus (the keyboard equivalents)
  let menu = $state<{ id: string; parent: string; folder: boolean; x: number; y: number } | null>(null);
  const menuItems = $derived.by((): MenuItem[] => {
    if (!menu) return [];
    const s = findSection(menu.id), items: MenuItem[] = [];
    const sibs = visibleTop.filter((x) => !(x as SectionD).presentation).map((x) => x.id), i = sibs.indexOf(menu.id);
    if (may(s, "reorder")) { items.push({ id: "up", label: "Move up", disabled: i <= 0 }, { id: "down", label: "Move down", disabled: i < 0 || i >= sibs.length - 1 }); }
    if (may(s, "rename") || menu.folder) items.push({ id: "rename", label: "Rename" });
    items.push({ id: "folder", label: "New section" });
    if (menu.folder) items.push({ id: "delete", label: "Delete section", danger: true });
    else if (may(s, "hide")) items.push({ id: "hide", label: "Hide" });
    return items;
  });
  async function pickMenu(id: string) {
    if (id === "folder-new") { const v = await ui.prompt({ name: "title", title: "New section", placeholder: "Section name", kind: "text" }); if (v) layout.addFolder("", v); return; }
    const m = menu; menu = null; if (!m || !root) return;
    const sibs = visibleTop.filter((x) => !(x as SectionD).presentation).map((x) => x.id), i = sibs.indexOf(m.id);
    if (id === "up") await flip(root, () => layout.moveSection(m.parent, sibs, m.id, sibs[i - 1] ?? null));
    else if (id === "down") await flip(root, () => layout.moveSection(m.parent, sibs, m.id, sibs[i + 2] ?? null));
    else if (id === "hide") layout.hide(m.id, true);
    else if (id === "delete") await flip(root, () => layout.removeFolder(m.id));
    else if (id === "rename") { const v = await ui.prompt({ name: "title", title: "Rename", kind: "text", initial: titleOf(m.id, findSection(m.id)?.title) }); if (v !== null) layout.rename(m.id, v); }
    else if (id === "folder") { const v = await ui.prompt({ name: "title", title: "New section", placeholder: "Section name", kind: "text" }); if (v) { const f = layout.addFolder("", v); void f; } }
  }
  async function moveRow(section: string, rowId: string, dir: -1 | 1) {
    const ids = (stripSec(section)?.presentation === "strip" ? rowsOfStrip(section) : shownRows(findSection(section) ?? { id: section }, ctxOf(section))).map((x) => x.r.id), i = ids.indexOf(rowId); if (i < 0 || !root) return;
    await flip(root, () => layout.moveRow(section, ids, rowId, dir < 0 ? ids[i - 1] ?? null : ids[i + 2] ?? null));
  }
  let hiddenOpen = $state(false), hiddenEl = $state<HTMLElement | null>(null);
  const hiddenItems = $derived<MenuItem[]>(hiddenAll.map((h) => ({ id: h.id, label: titleOf(h.id, h.title) || h.id })));

  // ---- strip fades: a soft fade at an end only while there is more to scroll that way
  function fades(el: HTMLElement, _n?: number) { const f = () => { el.dataset.fl = el.scrollLeft > 2 ? "1" : ""; el.dataset.fr = el.scrollLeft + el.clientWidth < el.scrollWidth - 2 ? "1" : ""; }; f(); el.addEventListener("scroll", f, { passive: true }); const ro = new ResizeObserver(f); ro.observe(el); const mo = new MutationObserver(f); mo.observe(el, { childList: true }); return { update: f, destroy() { el.removeEventListener("scroll", f); ro.disconnect(); mo.disconnect(); } }; }
  const literalOpen = (r: NonNullable<SectionD["rows"]>[number]) => {
    const o = r.open as unknown as { route?: { name: string; value: unknown }; invoke?: import("../model").Invoke; shell?: unknown } | undefined; if (!o) return;
    if (o.route) { const v = o.route.value; app.setRoute(o.route.name, v && typeof v === "object" && "literal" in v ? String((v as { literal: unknown }).literal) : String(v)); }
    else if (o.invoke) void ui.invoker.run(o.invoke, { context: viewContext(o.invoke.id) });
    else if (o.shell) app.onshell?.(o);
  };
</script>

{#snippet head(s: { id: string; title?: string; actions?: SectionD["actions"] }, parent: string, folder: boolean, depth: number)}
  {@const own = findSection(s.id)}
  <div class="sh" data-sec-head={s.id} data-drag={may(own, "reorder") ? "section" : undefined} data-sec-id={s.id} data-parent={parent} data-flip={`head:${s.id}`}
    oncontextmenu={(e) => { e.preventDefault(); menu = { id: s.id, parent, folder, x: e.clientX, y: e.clientY }; }}>
    <button type="button" class="fold-btn" aria-expanded={!isFolded(own ?? { id: s.id })} aria-label={`${isFolded(own ?? { id: s.id }) ? "Expand" : "Collapse"} ${titleOf(s.id, s.title)}`} disabled={!may(own, "collapse") && !folder} onclick={() => layout.fold(s.id, !isFolded(own ?? { id: s.id }))}>
      <span class="chev" class:shut={isFolded(own ?? { id: s.id })}><Icon name="chevron-down" inline /></span><span class="ttl">{titleOf(s.id, s.title)}</span>
    </button>
    {#each s.actions ?? [] as a (a.id)}<span class="ha" data-nodrag><ActionButton action={a} context={viewContext(a.id)} iconOnly /></span>{/each}
    <button type="button" class="more" data-nodrag aria-label={`${titleOf(s.id, s.title)} options`} aria-haspopup="menu" onclick={(e) => { const r = (e.currentTarget as HTMLElement).getBoundingClientRect(); menu = { id: s.id, parent, folder, x: r.left, y: r.bottom }; }}><Icon name="more" inline /></button>
  </div>
{/snippet}

{#snippet rowsOf(s: { id: string; bind?: string }, topId: string)}
  {#each shownRows(s, topId) as x (x.r.id)}
    {@const t = templateOf(x.store)}
    <div class="dr" data-drag="row" data-row-id={x.r.id} data-in={s.id} data-flip={`row:${x.r.id}`}>
      <RowActions flat template={t} row={x.r} store={x.store} kindIcon={app.kindIconFor(x.store.bind.id)} selected={app.route[routeOf(t)] === x.r.id} onclick={() => openRow(x.store, x.r)}
        extra={may(findSection(s.id), "reorder") || !!folderOf(s.id) ? [{ id: "\u0000up", label: "Move up" }, { id: "\u0000down", label: "Move down" }] : []} onextra={(id) => void moveRow(s.id, x.r.id, id === "\u0000up" ? -1 : 1)} />
    </div>
  {/each}
{/snippet}

{#snippet body(s: SectionD | { id: string; folder: true }, parent: string, depth: number)}
  {@const own = findSection(s.id)}
  {@const real = own ?? undefined}
  <section class="sec" class:drop-on={armed(s.id) && !real?.presentation} data-sec={s.id} data-flip={`sec:${s.id}`}>
    {#if titleOf(s.id, real?.title)}{@render head(real ?? { id: s.id }, parent, !real, depth)}{/if}
    <div class="sfold" class:shut={isFolded(real ?? { id: s.id })} inert={isFolded(real ?? { id: s.id })}><div class="in">
      {#if real?.rows}{#each real.rows as r (r.id)}
        {@const ic = typeof r.icon === "string" ? knownIcon(r.icon) : undefined}
        <div class="lit"><Row title={r.title} icon={ic} onclick={() => literalOpen(r)} /></div>
      {/each}{/if}
      {#if real?.bind || !real}{@render rowsOf(real ?? { id: s.id }, real ? topOf(s.id) : parent)}{#if !real && shownRows({ id: s.id }, parent).length === 0}<p class="hint" data-folder-empty>Drag conversations here.</p>{/if}{/if}
    </div></div>
  </section>
{/snippet}

{#snippet stripOf(s: SectionD)}
      {@const items = rowsOfStrip(s.id)}
      <div class="stripw" class:open={items.length > 0 || mayPin(s.id)} class:armed={armed(s.id) || (mayPin(s.id) && dragging?.over?.type === "strip" && dragging.over.section === s.id)} data-flip={`sec:${s.id}`}>
        <div class="strip" data-strip={s.id} use:fades={items.length} role="list" aria-label={titleOf(s.id, s.title) || "Pinned"}>
          {#each items as x (x.r.id)}
            {@const t = templateOf(x.store)}
            {@const av = t.icon && typeof t.icon === "object" && "avatar" in t.icon ? (t.icon as { avatar: { label: string; image?: string } }).avatar : null}
            <div class="pin" role="listitem" data-drag="row" data-row-id={x.r.id} data-in={s.id} data-flip={`row:${x.r.id}`}>
              <button type="button" class="pb" aria-label={String(x.r.fields.title ?? x.r.id)} aria-current={app.route[routeOf(t)] === x.r.id ? "true" : undefined} onclick={() => openRow(x.store, x.r)}
                oncontextmenu={(e) => e.preventDefault()} onkeydown={(e) => { if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) { e.preventDefault(); void moveRow(s.id, x.r.id, e.key === "ArrowLeft" ? -1 : 1); } }}>
                <Avatar label={String(av ? x.r.fields[av.label] ?? x.r.fields.title ?? "" : x.r.fields.title ?? "")} id={x.r.id} size="var(--pin-size, 44px)" />
                {#each [x.r.fields.unread, x.r.fields.mentions] as n, k (k)}{#if typeof n === "number" && n > 0 && !(k === 1 && x.r.fields.muted !== true) && !(k === 0 && x.r.fields.muted === true)}<span class="pbadge" class:q={false} data-mark="count">{n}</span>{/if}{/each}
              </button>
            </div>
          {/each}
          {#if items.length === 0 && mayPin(s.id)}<p class="hint pinhint" data-pin-hint>Drop here to pin</p>{/if}
        </div>
      </div>
{/snippet}

<div class="tree" bind:this={root} data-tree>
  {#each visibleTop as s (s.id)}
    {#if "presentation" in s && s.presentation === "strip"}{@render stripOf(s as SectionD)}
    {:else}{@render body(s as SectionD | { id: string; folder: true }, "", 0)}{/if}
  {/each}
  <button type="button" class="hid newsec" data-new-section onclick={() => void pickMenu("folder-new")}><Icon name="plus" inline /> New section</button>
  {#if hiddenAll.length}<button type="button" class="hid" bind:this={hiddenEl} data-hidden-entry aria-haspopup="menu" onclick={() => (hiddenOpen = true)}><Icon name="more" inline /> Hidden ({hiddenAll.length})</button>{/if}
  <div class="dropline" class:on={!!line} style:transform={line ? `translate(${line.x}px, ${line.y}px)` : undefined} style:width={line ? `${line.w}px` : undefined} style:height={line ? `${line.h}px` : undefined} aria-hidden="true" data-dropline></div>
  <Menu open={!!menu} items={menuItems} label="Section options" onpick={pickMenu} onclose={() => (menu = null)} trigger={null} anchor={menu ? { left: menu.x, top: menu.y, right: menu.x, bottom: menu.y } : null} />
  <Menu open={hiddenOpen} items={hiddenItems} label="Hidden sections" onpick={(id) => { hiddenOpen = false; layout.hide(id, false); }} onclose={() => (hiddenOpen = false)} trigger={hiddenEl} />
</div>

<style>
  .tree { position: relative; display: flex; flex-direction: column; }
  .sec { display: flex; flex-direction: column; border-radius: var(--r-select); transition: background-color var(--dur-fast) var(--ease-out); }
  .sec.drop-on { background: var(--accent-soft); }
  .sh { display: flex; align-items: center; gap: var(--s-1); min-height: var(--row-h); padding: 0 var(--s-2) 0 var(--s-3); margin-top: var(--s-3); touch-action: pan-y; user-select: none; cursor: default; }
  .fold-btn { flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--s-2); min-height: var(--row-h); text-align: start; color: var(--ink-3); font-size: var(--text-section); font-weight: var(--strong-weight); border-radius: var(--r-select); }
  .ttl { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chev { display: inline-grid; transition: rotate var(--dur-base) var(--ease-out); } .chev.shut { rotate: -90deg; }
  .more, .ha { opacity: 0; transition: opacity var(--dur-fast) var(--ease-out); color: var(--ink-3); } .sh:hover .more, .sh:focus-within .more, .sh:hover .ha, .sh:focus-within .ha { opacity: 1; }
  .more { display: inline-grid; place-items: center; min-width: 28px; height: 28px; border-radius: 50%; } .more:hover { background: rgb(var(--ink-rgb) / 0.08); }
  /* A section folds by gathering its height (a grid track from its full height to nothing) and fading, over --dur-base on --ease-out: interruptible, since a transition in flight reverses from where it is. */
  .sfold { display: grid; grid-template-rows: 1fr; transition: grid-template-rows var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); } .sfold.shut { grid-template-rows: 0fr; opacity: 0; } .sfold > .in { min-height: 0; overflow: clip; }
  .dr { border-radius: var(--r-select); } .dr[data-dragging], .pin[data-dragging] { opacity: 0.35; }
  .lit { border-radius: var(--r-select); }
  .hint { margin: 0; padding: var(--s-3) var(--s-5); color: var(--ink-3); font-size: var(--text-label); }
  .hid { display: flex; align-items: center; gap: var(--s-2); min-height: var(--row-h); margin-top: var(--s-3); padding: 0 var(--s-5); color: var(--ink-3); font-size: var(--text-label); border-radius: var(--r-select); } .hid:hover { background: var(--hover); }
  /* The pinned strip: round thumbnails in a row that scrolls sideways, a soft fade at an end only while there is more that way. It opens (its height grows) when there are pins or a pin is being dragged. */
  .stripw { display: grid; grid-template-rows: 0fr; margin-top: var(--s-2); border-radius: var(--r-card); transition: grid-template-rows var(--dur-base) var(--ease-out), background-color var(--dur-fast) var(--ease-out), outline-color var(--dur-fast) var(--ease-out); outline: 2px solid transparent; outline-offset: -2px; }
  .stripw.open { grid-template-rows: 1fr; } .stripw.armed { background: var(--accent-soft); outline-color: var(--accent); }
  .strip { min-height: 0; overflow-x: auto; overflow-y: clip; display: flex; gap: var(--s-3); padding: var(--s-2) var(--s-4); scrollbar-width: none; overscroll-behavior-x: contain; scroll-snap-type: x proximity; scroll-padding-inline: var(--s-4); --fl: 0px; --fr: 0px;
    -webkit-mask-image: linear-gradient(to right, transparent 0, #000 var(--fl), #000 calc(100% - var(--fr)), transparent 100%); mask-image: linear-gradient(to right, transparent 0, #000 var(--fl), #000 calc(100% - var(--fr)), transparent 100%); }
  .strip:global([data-fl="1"]) { --fl: 18px; } .strip:global([data-fr="1"]) { --fr: 18px; }
  .pin { flex: none; scroll-snap-align: start; touch-action: pan-x; animation: arrive var(--dur-base) var(--ease-out) both; }
  .pb { position: relative; display: inline-grid; padding: 2px; border-radius: 50%; transition: scale var(--dur-fast) var(--ease-out); } .pb:active { scale: var(--press-scale); }
  .pb[aria-current="true"] { box-shadow: 0 0 0 2px var(--accent); }
  .pbadge { position: absolute; top: -2px; right: -2px; min-width: 18px; height: 18px; padding: 0 5px; box-sizing: border-box; border-radius: 9px; background: var(--accent-base); color: #fff; font-size: var(--text-label); display: grid; place-items: center; }
  .pinhint { align-self: center; padding: var(--s-3) 0; }
  @keyframes arrive { from { opacity: 0; scale: 0.8; } to { opacity: 1; scale: 1; } }
  .dropline { position: absolute; left: 0; top: 0; background: var(--accent); border-radius: 2px; opacity: 0; pointer-events: none; transition: transform var(--dur-fast) var(--ease-out), width var(--dur-fast) var(--ease-out), height var(--dur-fast) var(--ease-out), opacity var(--dur-fast) var(--ease-out); z-index: 5; } .dropline.on { opacity: 1; }
  :global([data-dnd]) { user-select: none; cursor: grabbing; } :global([data-dnd] *) { cursor: grabbing !important; }
</style>
