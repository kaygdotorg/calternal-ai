<script lang="ts">
  import PanelFrame from "../../shell/PanelFrame.svelte";
  // The Notes right panel, for the open window: four tabs. OUTLINE (the headings of the page; the one you are in is marked and follows the scroll; choosing one scrolls there over `--dur-slow` on `--ease-out`, instantly under reduced motion), LINKS (what links here, what this note links to),
  // HISTORY (versions, who and when, a restore action) and SHARING (who is on it with a role, the link). With no note open: how many notes, the folders. Flat on the background like the shell's panel.
  import { Badge, Button, Selector } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import NoteChip from "./NoteChip.svelte";
  import { DOCS, FOLDERS, docOf, headings, linksFrom, linksTo, notes } from "./data";
  import { notesUi } from "./ui.svelte";
  import type { Win } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { win: Win | undefined; item?: Item } = $props();
  type Tab = "outline" | "links" | "history" | "sharing";
  let tab = $state<Tab>("outline"), active = $state(0), link = $state(false);
  const TABS = [{ value: "outline", label: "Outline", iconName: "list", iconOnly: true }, { value: "links", label: "Links", iconName: "link", iconOnly: true }, { value: "history", label: "History", iconName: "clock", iconOnly: true }, { value: "sharing", label: "Sharing", iconName: "share", iconOnly: true }] as const;
  const doc = $derived(item ? docOf(item) : undefined);
  const outline = $derived(doc ? doc.blocks.flatMap((b, i) => (b.t === "h1" || b.t === "h2" || b.t === "h3" ? [{ i, ...headings([b])[0]! }] : [])) : []);
  const to = (i: number) => { const el = document.getElementById(`h-${item?.id}-${i}`); if (el) el.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth", block: "start" }); };
  $effect(() => {
    void item?.id; void tab; active = 0;
    const els = outline.map((o) => document.getElementById(`h-${item?.id}-${o.i}`)).filter((e): e is HTMLElement => !!e);
    if (!els.length) return;
    const root = els[0]!.closest<HTMLElement>("[data-note-page]");
    const on = () => { const top = (root?.getBoundingClientRect().top ?? 0) + 24; let k = 0; els.forEach((e, n) => { if (e.getBoundingClientRect().top <= top + 4) k = n; }); active = k; };
    root?.addEventListener("scroll", on, { passive: true }); on();
    return () => root?.removeEventListener("scroll", on);
  });
</script>

<PanelFrame title={item ? item.title : "Notes"} suggestions={["Summarise this note", "Find related notes", "Suggest a title"]}>
    {#if item && doc}<Selector options={[...TABS]} value={tab} label="Note panel" size="sm" class="equal" onchange={(v: string) => (tab = v as Tab)} />{/if}
    {#if !item || !doc}
      <section><h3>Library</h3><dl><dt>Notes</dt><dd>{notes.length}</dd><dt>Shared</dt><dd>{notes.filter((n) => DOCS[n.id]!.shared.length > 1).length}</dd><dt>Tags</dt><dd>5</dd></dl></section>
      <section><h3>Folders</h3><ul>{#each FOLDERS as f (f)}<li><Glyph name="folder" inline /><span class="pn">{f}</span><span class="rl">{notes.filter((n) => DOCS[n.id]!.folder === f).length || ""}</span></li>{/each}</ul></section>
    {:else if tab === "outline"}
      <section class="tx"><h3>Text</h3><Selector options={[{ value: "std", label: "Standard" }, { value: "read", label: "Reading" }]} value={notesUi.reading ? "read" : "std"} label="Text size" size="sm" class="equal" onchange={(v: string) => notesUi.set(v === "read")} /></section>
      {#if outline.length}<nav aria-label="Outline"><ul class="ol">{#each outline as o, k (o.i)}<li class:sub={o.level === 3}><button type="button" class:on={k === active} aria-current={k === active ? "location" : undefined} onclick={() => to(o.i)}>{o.text}</button></li>{/each}</ul></nav>
      {:else}<p class="none">This note has no headings yet. A line that starts with <code>#</code> becomes one.</p>{/if}
    {:else if tab === "links"}
      <section><h3>Linked from <span>{linksTo(item.id).length}</span></h3><ul class="chips">{#each linksTo(item.id) as n (n.id)}<li><NoteChip id={n.id} /></li>{:else}<li class="none">No other note links here.</li>{/each}</ul></section>
      <section><h3>Links to <span>{linksFrom(item.id).length}</span></h3><ul class="chips">{#each linksFrom(item.id) as n (n.id)}<li><NoteChip id={n.id} /></li>{:else}<li class="none">This note links to nothing.</li>{/each}</ul></section>
    {:else if tab === "history"}
      <section><h3>Versions</h3><ul class="hist">{#each doc.history as h, k (k)}<li><Av name={h.who} size="var(--s-8)" /><span class="hx"><b>{h.what}</b><span>{h.who} · {h.when}</span></span>{#if k > 0}<button type="button" class="rs">Restore</button>{:else}<Badge text="Current" tone="accent" />{/if}</li>{/each}</ul></section>
    {:else}
      <section><h3>People</h3><ul>{#each doc.shared as p (p.name)}<li><Av name={p.name} size="var(--s-8)" /><span class="pn">{p.name}</span><span class="rl">{p.role}</span></li>{/each}</ul><div class="act"><Button text="Invite" icon="plus" variant="secondary" /></div></section>
      <section><h3>Link</h3><div class="lk"><span class="lt"><span>Anyone with the link can read</span><small>{link ? "On: the link works for anyone." : "Off: only the people above."}</small></span><button type="button" class="tg" role="switch" aria-checked={link} aria-label="Anyone with the link can read" onclick={() => (link = !link)}><i></i></button></div></section>
    {/if}
</PanelFrame>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); } h3 span { color: var(--ink-3); font-weight: var(--ui-weight); font-variant-numeric: tabular-nums; }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); } dt { color: var(--ink-3); } dd { margin: 0; text-align: end; font-variant-numeric: tabular-nums; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-2); font-size: var(--text-ui); } li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--s-9); } .pn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .rl { color: var(--ink-3); font-size: var(--text-label); }
  .ol { gap: 0; } .ol li { display: block; min-height: 0; } .ol button { display: block; width: 100%; padding: var(--s-3) var(--s-4); border-radius: var(--r-select); color: var(--ink-3); text-align: start; font-size: var(--text-ui); line-height: 1.3; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; transition: background-color var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out); }
  .ol .sub button { padding-inline-start: var(--s-8); } .ol button.on { color: var(--ink); background: var(--accent-soft); font-weight: var(--strong-weight); } @media (hover: hover) and (pointer: fine) { .ol button:not(.on):hover { background: var(--hover); color: var(--ink-2); } }
  .chips { gap: var(--s-3); } .chips li { min-height: 0; } .none { margin: 0; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.5; } li.none { display: block; }
  code { font-family: var(--font-mono); font-size: var(--text-label); padding: 0 var(--s-2); border-radius: var(--s-2); background: rgb(var(--ink-rgb) / 0.07); }
  .hist li { align-items: flex-start; padding-block: var(--s-2); } .hx { flex: 1; min-width: 0; display: grid; line-height: 1.3; } .hx b { font-weight: var(--ui-weight); } .hx span { color: var(--ink-3); font-size: var(--text-label); } .rs { color: var(--accent); font-size: var(--text-label); padding: var(--s-2) var(--s-3); border-radius: var(--r-select); } .rs:hover { background: var(--accent-soft); }
  /* the link switch is drawn here: packages/ui's Switch has a class (.sw) the shell's global rules also match */
  .lk { display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); min-height: var(--pill-h); } .lt { display: grid; min-width: 0; font-size: var(--text-ui); line-height: 1.3; } .lt small { color: var(--ink-3); font-size: var(--text-label); }
  .tg { position: relative; flex: none; width: calc(var(--s-9) + var(--s-4)); height: calc(var(--s-8) + var(--s-1) * 2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.18); transition: background-color var(--dur-base) var(--ease-out); } .tg[aria-checked="true"] { background: var(--accent-base); }
  .tg i { position: absolute; top: var(--s-1); left: var(--s-1); width: var(--s-8); height: var(--s-8); border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 0.3); transition: translate var(--dur-base) var(--ease-out); } .tg[aria-checked="true"] i { translate: calc(var(--s-9) + var(--s-4) - var(--s-8) - var(--s-1) * 2) 0; }
  .act { margin-top: var(--s-4); } footer { flex: none; padding: 0 var(--s-10) var(--s-3) 0; }
</style>
