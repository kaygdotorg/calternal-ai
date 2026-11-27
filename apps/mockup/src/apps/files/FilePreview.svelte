<script lang="ts">
  // A file opened: its preview in the middle, drawn by family (a PDF as stacked pages, a document as a reading page, an image large, a spreadsheet's cells, a slide, a recording's transport, an archive's contents, code). Details and sharing are in the side panel, not here.
  // The page is one scroller with the bar on top (Back to the folder, the name, share and download). The long document and the six-page lease show how a page that scrolls reads. A PDF page is a picture of a page: its text is drawn in container units so the page keeps its proportions at any width.
  import { IconButton } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import FileThumb from "./FileThumb.svelte";
  import Scene from "../photos/Scene.svelte";
  import { ARCHIVE_LIST, docBlocks, pdfPages, sheetRows } from "./content";
  import { fullWhen, metaOf, sizeText, type FileMeta } from "./data";
  let { m, onback }: { m: FileMeta; onback?: () => void } = $props();
  const f = $derived(m.type.family);
  const parent = $derived(m.parent ? metaOf(m.parent) : undefined);
  const pages = $derived(f === "pdf" ? pdfPages(m.name, m.pages ?? 1) : []);
  const blocks = $derived(f === "doc" || f === "text" ? docBlocks(m.name) : []);
  const sheet = $derived(f === "sheet" ? sheetRows(m.name) : undefined);
  let playing = $state(false), pos = $state(0.18);
  const CODE = ["export interface Reply {", "  id: string;", "  total: number;", "  items: Item[];", "}", "", "export async function load(id: string): Promise<Reply> {", "  const res = await fetch(`/api/replies/${id}`);", "  if (!res.ok) throw new Error(\"not found\");", "  return res.json();", "}"];
  const mins = $derived(Math.max(1, Math.round(m.bytes / 6e6)));
  const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
  function key(e: KeyboardEvent) { if (e.key === "Escape" && onback && !(e.target as HTMLElement).closest("input")) onback(); }
</script>

<svelte:window onkeydown={key} />
<div class="pv" data-file-preview={m.id} data-family={f}>
  <header class="bar">
    {#if onback}<button type="button" class="back" onclick={onback}><AppIcon name="back" inline /><span>{parent?.name ?? "Files"}</span></button>{/if}
    <div class="id"><strong>{m.name}</strong><span>{m.type.label} · {sizeText(m)} · {fullWhen(m)}</span></div>
    <span class="sp"></span>
    <IconButton icon="share" label="Share" onclick={() => {}} /><IconButton icon="down" label="Download" onclick={() => {}} />
  </header>
  <div class="body" class:center={f === "image" || f === "video" || f === "audio" || f === "slide" || f === "archive"}>
    {#if f === "image" && m.scene}
      <div class="img" style:--ar="4 / 3" role="img" aria-label={m.name}><Scene spec={m.scene} /></div>
    {:else if f === "pdf"}
      <div class="pages">{#each pages as blocks, p (p)}
        <article class="pdf" aria-label={`Page ${p + 1} of ${pages.length}`}>
          {#each blocks as b, i (i)}{#if b.t === "h"}<h2 class:big={p === 0 && i === 0}>{b.s}</h2>{:else if b.t === "p"}<p>{b.s}</p>{:else if b.t === "li"}<p class="li">{b.s}</p>{:else}<p class="kv"><span>{b.s}</span><b>{b.v}</b></p>{/if}{/each}
          <footer>{m.name.replace(/\.pdf$/, "")}<span>{p + 1} / {pages.length}</span></footer>
        </article>{/each}
      </div>
    {:else if f === "doc" || f === "text"}
      <article class="doc"><h1>{m.name.replace(/\.(docx|md|txt)$/, "")}</h1>{#each blocks as b, i (i)}{#if b.t === "h"}<h2>{b.s}</h2>{:else if b.t === "p"}<p>{b.s}</p>{:else}<p class="li"><span aria-hidden="true">•</span>{b.s}</p>{/if}{/each}</article>
    {:else if sheet}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scroll area must be reachable by keyboard) -->
      <div class="sh" role="region" aria-label="Spreadsheet" tabindex="0"><table><thead><tr><th class="rn"></th>{#each sheet.head as h, i (h)}<th class:num={i > 0 && /^[\d,.]+$/.test(sheet.rows[0]?.[i] ?? "")}>{h}</th>{/each}</tr></thead><tbody>{#each sheet.rows as row, r (r)}<tr><th class="rn">{r + 1}</th>{#each row as c, i (i)}<td class:num={/^[\d,.]+$/.test(c)}>{c}</td>{/each}</tr>{/each}</tbody></table></div>
      <p class="tabs"><span class="on">Sheet 1</span><span>Notes</span></p>
    {:else if f === "slide"}
      <div class="slides"><div class="sl"><FileThumb id={m.id} /></div><p class="cap">Slide 1 of 12</p></div>
    {:else if f === "video" || f === "audio"}
      <div class="media"><div class="mv" class:aud={f === "audio"}><FileThumb id={m.id} /></div>
        <div class="tr"><IconButton icon={playing ? "stop-round" : "play"} label={playing ? "Pause" : "Play"} pressed={playing} onclick={() => (playing = !playing)} /><span class="t">{clock(pos * mins * 60)}</span><div class="track" role="slider" tabindex="0" aria-label="Position" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(pos * 100)} onkeydown={(e) => { if (e.key === "ArrowRight") pos = Math.min(1, pos + 0.05); if (e.key === "ArrowLeft") pos = Math.max(0, pos - 0.05); }} onpointerdown={(e) => { const r = e.currentTarget.getBoundingClientRect(); pos = (e.clientX - r.left) / r.width; }}><i style:width="{pos * 100}%"></i></div><span class="t">{clock(mins * 60)}</span></div></div>
    {:else if f === "archive"}
      <div class="arc"><div class="af"><FileThumb id={m.id} /></div><ul>{#each ARCHIVE_LIST as a (a)}<li><AppIcon name="file" inline /><span>{a}</span></li>{/each}</ul><p class="cap">5 items in this archive</p></div>
    {:else if f === "code"}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scroll area must be reachable by keyboard) -->
      <pre class="cd" tabindex="0">{#each CODE as l, i (i)}<span class="ln"><i>{i + 1}</i>{l}</span>{/each}</pre>
    {:else}<div class="img"><FileThumb id={m.id} /></div>{/if}
  </div>
</div>

<style>
  .pv { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--s-4); container-type: inline-size; }
  .bar { flex: none; display: flex; align-items: center; gap: var(--s-4); } .sp { flex: 1; }
  .back { display: inline-flex; align-items: center; gap: var(--s-2); max-width: 40%; min-height: 32px; padding: 0 var(--s-4) 0 var(--s-3); border-radius: var(--r-pill); color: var(--accent); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .back span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .back:active { scale: var(--press-scale); } @media (hover: hover) and (pointer: fine) { .back:hover { background: var(--accent-soft); } }
  @container (max-width: 26rem) { .bar { flex-wrap: wrap; row-gap: var(--s-2); } .back { max-width: none; } .id { order: 3; flex: 1 0 100%; } .sp { display: none; } .back { margin-inline-end: auto; } }
  .id { display: grid; min-width: 0; line-height: 1.25; } .id strong, .id span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .id strong { font-size: var(--text-ui); font-weight: var(--strong-weight); } .id span { color: var(--ink-3); font-size: var(--text-label); }
  .body { flex: 1; min-height: 0; overflow-y: auto; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); padding: var(--s-6); scrollbar-width: thin; } .body.center { display: grid; place-items: center; align-content: center; }
  .img { width: min(100%, 52rem); aspect-ratio: var(--ar, 4 / 3); overflow: hidden; border-radius: var(--r-select); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12), 0 8px 28px rgb(0 0 0 / 0.14); }
  .pages { display: grid; justify-items: center; gap: var(--s-6); } .pdf { --pp: var(--paper, #fff); container-type: inline-size; width: min(100%, 38rem); aspect-ratio: 1 / 1.414; box-sizing: border-box; padding: 8cqw 9cqw; overflow: hidden; border-radius: var(--r-select); background: var(--pp); color: #222; box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 8px 24px rgb(0 0 0 / 0.12); position: relative; }
  :global(html[data-scheme="dark"]) .pdf { --pp: #f2f2f2; } .pdf h2 { margin: 3.2cqw 0 1.2cqw; font-size: 3.2cqw; line-height: 1.2; font-weight: 650; } .pdf h2.big { margin-top: 0; font-size: 5cqw; } .pdf p { margin: 0 0 1.6cqw; font-size: 2.45cqw; line-height: 1.55; color: #3a3a3a; } .pdf .li { padding-left: 3cqw; position: relative; } .pdf .li::before { content: "•"; position: absolute; left: 0.6cqw; } .pdf .kv { display: flex; justify-content: space-between; gap: 3cqw; padding: 0.9cqw 0; border-bottom: 1px solid rgb(0 0 0 / 0.1); } .pdf .kv b { font-weight: 600; font-variant-numeric: tabular-nums; }
  .pdf footer { position: absolute; left: 9cqw; right: 9cqw; bottom: 4cqw; display: flex; justify-content: space-between; font-size: 1.9cqw; color: #8a8a8a; }
  .doc { width: min(100%, 40rem); margin: 0 auto; padding: var(--s-6) var(--s-7); border-radius: var(--r-card); background: var(--paper, #fff); box-shadow: 0 1px 2px rgb(0 0 0 / 0.1), 0 6px 20px rgb(0 0 0 / 0.08); box-sizing: border-box; } :global(html[data-scheme="dark"]) .doc { background: #2b2b2e; }
  .doc h1 { margin: 0 0 var(--s-5); font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); } .doc h2 { margin: var(--s-7) 0 var(--s-3); font-size: var(--text-heading); line-height: 1.25; font-weight: var(--strong-weight); } .doc p { margin: 0 0 var(--s-4); font-size: var(--text-ui); line-height: 1.6; } .doc .li { display: flex; gap: var(--s-4); margin-bottom: var(--s-2); } .doc .li span { color: var(--ink-3); }
  .sh { overflow: auto; border-radius: var(--r-select); background: var(--paper, #fff); box-shadow: 0 1px 2px rgb(0 0 0 / 0.1), 0 6px 20px rgb(0 0 0 / 0.08); } :global(html[data-scheme="dark"]) .sh { background: #2b2b2e; } table { width: 100%; border-collapse: collapse; font-size: var(--text-ui); font-variant-numeric: tabular-nums; }
  th, td { padding: var(--s-3) var(--s-5); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.08); border-right: 1px solid rgb(var(--ink-rgb) / 0.06); text-align: start; white-space: nowrap; } thead th { background: oklch(0.9 0.05 152 / 0.45); font-weight: var(--strong-weight); font-size: var(--text-label); } .num { text-align: end; } .rn { width: 2.25rem; text-align: center; color: var(--ink-3); font-size: var(--text-label); background: rgb(var(--ink-rgb) / 0.04); font-weight: var(--ui-weight); }
  .tabs { display: flex; gap: var(--s-3); margin: var(--s-4) 0 0; font-size: var(--text-label); } .tabs span { padding: var(--s-1) var(--s-5); border-radius: var(--r-pill); color: var(--ink-3); } .tabs .on { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  .slides { display: grid; gap: var(--s-4); width: min(100%, 48rem); } .sl { aspect-ratio: 16 / 10; overflow: hidden; border-radius: var(--r-select); box-shadow: 0 1px 2px rgb(0 0 0 / 0.12), 0 8px 28px rgb(0 0 0 / 0.14); } .cap { margin: 0; text-align: center; color: var(--ink-3); font-size: var(--text-label); }
  .media { display: grid; gap: var(--s-5); width: min(100%, 44rem); } .mv { aspect-ratio: 16 / 9; overflow: hidden; border-radius: var(--r-select); } .mv.aud { aspect-ratio: 3 / 1; } .tr { display: flex; align-items: center; gap: var(--s-4); } .t { color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; min-width: 2.5rem; } .t:last-child { text-align: end; }
  .track { flex: 1; height: 6px; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.14); cursor: pointer; } .track i { display: block; height: 100%; border-radius: inherit; background: var(--accent-base); }
  .arc { display: grid; justify-items: center; gap: var(--s-5); width: min(100%, 28rem); } .af { width: 7rem; height: 7rem; } .arc ul { list-style: none; margin: 0; padding: 0; width: 100%; display: grid; font-size: var(--text-ui); } .arc li { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-5); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); }
  .cd { margin: 0; padding: var(--s-5) 0; border-radius: var(--r-select); background: oklch(0.24 0.02 215); color: oklch(0.9 0.01 250); font-family: var(--font-mono, ui-monospace, monospace); font-size: var(--text-label); line-height: 1.6; overflow: auto; } .ln { display: block; padding: 0 var(--s-6); white-space: pre; } .ln i { display: inline-block; width: 2rem; color: oklch(0.6 0.02 250); font-style: normal; text-align: end; margin-right: var(--s-5); user-select: none; }
</style>
