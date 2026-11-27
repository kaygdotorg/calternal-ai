<script lang="ts">
  // A file's picture, drawn from what it is (never a stock icon on a grey box): a folder with a tab, an image's own picture, a sheet of paper with its heading and lines (PDF and documents), a spreadsheet's grid, a slide, a video's poster with its length, an audio waveform,
  // a code panel, an archive. `icon` is the 24 px tile for a row (the family's colour and glyph, an image's own picture); `face` fills the box it is given (a gallery cell, a card, the panel's preview) and scales its drawing with the box (container units), so
  // it reads the same at any size. Colours come from the family's hue; the paper follows the theme. Decoration: hidden from screen readers, the name is beside it.
  import AppIcon from "../../shell/AppIcon.svelte";
  import Scene from "../photos/Scene.svelte";
  import { rng } from "../photos/scene";
  import { metaOf } from "./data";
  let { id, size = "face" }: { id: string; size?: "icon" | "face" } = $props();
  const m = $derived(metaOf(id)!);
  const f = $derived(m.type.family), h = $derived(m.type.hue);
  const lines = $derived.by(() => { const g = rng([...id].reduce((a, c) => a + c.charCodeAt(0) * 31, 5)); return Array.from({ length: 9 }, () => 52 + Math.round(g() * 40)); });
  const wave = $derived.by(() => { const g = rng([...id].reduce((a, c) => a + c.charCodeAt(0), 9)); return Array.from({ length: 28 }, (_, i) => 14 + Math.round((Math.sin(i / 3) * 0.5 + 0.5) * 40 + g() * 30)); });
  const cells = $derived.by(() => { const g = rng([...id].reduce((a, c) => a + c.charCodeAt(0), 2)); return Array.from({ length: 5 * 6 }, () => 30 + Math.round(g() * 60)); });
  const toks = $derived.by(() => { const g = rng([...id].reduce((a, c) => a + c.charCodeAt(0), 4)); return Array.from({ length: 8 }, () => ({ pad: Math.floor(g() * 3) * 8, parts: Array.from({ length: 2 + Math.floor(g() * 3) }, () => ({ w: 8 + Math.round(g() * 20), c: Math.floor(g() * 3) })) })); });
</script>

{#if size === "icon"}
  {#if f === "image" && m.scene}<span class="ic img" aria-hidden="true"><Scene spec={m.scene} /></span>
  {:else if f === "folder"}<span class="ic fol" aria-hidden="true" style:--h={h}><svg viewBox="0 0 24 20"><path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h5.2l2 2.4H20.5A1.5 1.5 0 0 1 22 6.9V16.5A1.5 1.5 0 0 1 20.5 18h-17A1.5 1.5 0 0 1 2 16.5Z" /></svg></span>
  {:else}<span class="ic tile" aria-hidden="true" style:--h={h}><AppIcon name={m.type.glyph} inline /></span>{/if}
{:else}
  <span class="fc" data-family={f} style:--h={h} aria-hidden="true">
    {#if f === "image" && m.scene}<Scene spec={m.scene} />
    {:else if f === "folder"}<svg class="fold" viewBox="0 0 120 90" preserveAspectRatio="xMidYMid meet"><path class="back" d="M8 14a6 6 0 0 1 6-6h24l10 10h58a6 6 0 0 1 6 6v54a6 6 0 0 1-6 6H14a6 6 0 0 1-6-6Z" /><path class="front" d="M8 32a6 6 0 0 1 6-6h92a6 6 0 0 1 6 6v46a6 6 0 0 1-6 6H14a6 6 0 0 1-6-6Z" /></svg>
    {:else if f === "pdf" || f === "doc" || f === "text"}
      <span class="paper"><span class="band" class:none={f === "text"}>{#if f !== "text"}<b>{m.type.tag}</b>{/if}</span><span class="ti"></span>{#each lines as w, i (i)}<i style:width="{i % 4 === 3 ? w - 25 : w}%" class:gap={i === 4}></i>{/each}</span>
    {:else if f === "sheet"}<span class="paper xl"><span class="grid">{#each cells as w, i (i)}<i class:hd={i < 5} class:lead={i % 5 === 0} style:--w="{w}%"></i>{/each}</span></span>
    {:else if f === "slide"}<span class="slide"><span class="s1"></span><span class="s2"></span><span class="s3"></span></span>
    {:else if f === "video"}<span class="vid"><span class="play"><AppIcon name="play" /></span><span class="len">{Math.floor(m.bytes / 6e6 / 60)}:{String(Math.floor((m.bytes / 6e6) % 60)).padStart(2, "0")}</span></span>
    {:else if f === "audio"}<span class="aud">{#each wave as v, i (i)}<i style:height="{v}%"></i>{/each}</span>
    {:else if f === "code"}<span class="code">{#each toks as t, i (i)}<span class="ln" style:padding-left="{t.pad}cqw">{#each t.parts as p, j (j)}<i class={`c${p.c}`} style:width="{p.w}cqw"></i>{/each}</span>{/each}</span>
    {:else}<span class="arc"><AppIcon name="archive" /></span>{/if}
  </span>
{/if}

<style>
  /* icon tile */
  .ic { display: inline-grid; place-items: center; flex: none; width: 24px; height: 24px; border-radius: var(--r-select); overflow: hidden; }
  .tile { background: oklch(0.9 0.05 var(--h)); color: oklch(0.45 0.13 var(--h)); } :global(html[data-scheme="dark"]) .tile { background: oklch(0.38 0.07 var(--h)); color: oklch(0.88 0.08 var(--h)); }
  .fol svg { width: 24px; height: 20px; fill: oklch(0.72 0.12 var(--h)); } :global(html[data-scheme="dark"]) .fol svg { fill: oklch(0.66 0.12 var(--h)); }
  .img { box-shadow: inset 0 0 0 1px rgb(var(--ink-rgb) / 0.12); }
  /* face */
  .fc { --paper-bg: var(--paper, #fff); display: grid; place-items: center; width: 100%; height: 100%; overflow: hidden; container-type: size; background: oklch(0.94 0.02 var(--h)); }
  :global(html[data-scheme="dark"]) .fc { --paper-bg: #2b2b2e; background: oklch(0.3 0.03 var(--h)); }
  .fc[data-family="image"] { background: none; }
  .fold { width: 62%; height: auto; max-height: 82%; } .fold .back { fill: oklch(0.66 0.13 var(--h)); } .fold .front { fill: oklch(0.76 0.12 var(--h)); } :global(html[data-scheme="dark"]) .fold .back { fill: oklch(0.5 0.12 var(--h)); } :global(html[data-scheme="dark"]) .fold .front { fill: oklch(0.6 0.12 var(--h)); }
  .paper { display: flex; flex-direction: column; gap: 3.6cqh; width: 52cqh; max-width: 70%; height: 86%; box-sizing: border-box; padding: 0 7cqh 7cqh; overflow: hidden; border-radius: var(--r-select); background: var(--paper-bg); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 4px 12px rgb(0 0 0 / 0.1); }
  .band { display: flex; align-items: center; margin: 0 -7cqh 2cqh; height: 14cqh; padding: 0 7cqh; background: oklch(0.7 0.14 var(--h)); } .band.none { background: none; height: 6cqh; } .band b { color: #fff; font-size: 7cqh; letter-spacing: 0.06em; }
  .ti { flex: none; height: 5.6cqh; width: 58%; border-radius: 1px; background: rgb(var(--ink-rgb) / 0.55); margin-bottom: 1.6cqh; } .paper i { flex: none; height: 2.8cqh; border-radius: 1px; background: rgb(var(--ink-rgb) / 0.16); } .paper i.gap { margin-bottom: 3cqh; }
  .xl { padding: 0; gap: 0; } .grid { display: grid; grid-template-columns: 1.1fr repeat(4, 1fr); grid-auto-rows: 1fr; flex: 1; } .grid i { height: auto; border-radius: 0; background: none; border-right: 1px solid rgb(var(--ink-rgb) / 0.1); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.1); position: relative; } .grid i::after { content: ""; position: absolute; left: 12%; top: 38%; width: var(--w); max-width: 76%; height: 22%; border-radius: 1px; background: rgb(var(--ink-rgb) / 0.2); } .grid i.hd { background: oklch(0.82 0.1 var(--h) / 0.6); } .grid i.lead::after { background: rgb(var(--ink-rgb) / 0.36); }
  .slide { display: grid; grid-template-rows: 1fr 1fr; width: 74%; aspect-ratio: 16 / 10; box-sizing: border-box; padding: 9cqh 9cqw; border-radius: var(--r-select); background: linear-gradient(135deg, oklch(0.82 0.1 var(--h)), oklch(0.7 0.15 calc(var(--h) + 40))); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 4px 12px rgb(0 0 0 / 0.1); } .slide span { display: block; border-radius: var(--r-pill); background: rgb(255 255 255 / 0.9); } .s1 { width: 60%; height: 22%; align-self: end; } .s2 { width: 38%; height: 12%; opacity: 0.7; margin-top: 6%; } .s3 { display: none !important; }
  .vid { position: relative; width: 100%; height: 100%; display: grid; place-items: center; background: linear-gradient(160deg, oklch(0.5 0.1 var(--h)), oklch(0.28 0.06 calc(var(--h) + 30))); } .play { display: grid; place-items: center; width: 22cqh; height: 22cqh; border-radius: 50%; background: rgb(0 0 0 / 0.5); color: #fff; } .play :global(svg) { width: 50%; height: 50%; }
  .len { position: absolute; right: 5cqh; bottom: 5cqh; padding: 1.5cqh 4cqh; border-radius: 99px; background: rgb(0 0 0 / 0.55); color: #fff; font-size: 8cqh; font-variant-numeric: tabular-nums; }
  .aud { display: flex; align-items: center; gap: 1.4cqw; width: 70%; height: 46%; } .aud i { flex: 1; border-radius: 99px; background: oklch(0.62 0.15 var(--h)); }
  .code { display: grid; align-content: center; gap: 4cqh; width: 74%; height: 78%; box-sizing: border-box; padding: 8cqh 8cqw; border-radius: var(--r-select); background: oklch(0.24 0.02 var(--h)); box-shadow: 0 1px 2px rgb(0 0 0 / 0.2), 0 4px 12px rgb(0 0 0 / 0.12); } .ln { display: flex; gap: 2cqw; } .ln i { height: 3cqh; border-radius: 99px; } .c0 { background: oklch(0.75 0.12 var(--h)); } .c1 { background: oklch(0.75 0.12 calc(var(--h) + 120)); } .c2 { background: oklch(0.7 0.01 250); }
  .arc { display: grid; place-items: center; width: 34cqh; height: 34cqh; border-radius: 8cqh; background: oklch(0.78 0.1 var(--h)); color: oklch(0.3 0.08 var(--h)); } .arc :global(svg) { width: 55%; height: 55%; }
</style>
