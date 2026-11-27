<script lang="ts">
  // The body of a note: its blocks drawn as a page. Size, measure and rhythm come from the parent (`.np` sets --body, --h2, --code, --lh, --gap: standard or reading); here each block has its own spacing from the tokens: a heading is closer to what it introduces than to what precedes it.
  // Static on purpose: no editor. The caret (a blinking bar) and the selection (an accent wash) are drawn where the markup puts `^` and `==…==`, so the page can be judged in the state it is written in. Tasks in a note are real task rows (checkbox, title, the day).
  import NoteChip from "./NoteChip.svelte";
  import TaskRow from "../tasks/TaskRow.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import { ITEMS } from "../../data/collections";
  import { wins } from "../../windows.svelte";
  import type { Block, Run } from "./data";
  let { blocks, id }: { blocks: Block[]; id: string } = $props();
</script>

{#snippet rs(list: Run[])}{#each list as r, i (i)}{#if typeof r === "string"}{r}{:else if "b" in r}<strong>{r.b}</strong>{:else if "i" in r}<em>{r.i}</em>{:else if "code" in r}<code>{r.code}</code>{:else if "note" in r}<NoteChip id={r.note} />{:else if "sel" in r}<mark>{r.sel}</mark>{:else if "caret" in r}<span class="caret" aria-hidden="true"></span>{:else}<a href={r.href} target="_blank" rel="noreferrer">{r.text}</a>{/if}{/each}{/snippet}

<div class="bl" data-blocks>
  {#each blocks as b, i (i)}
    {#if b.t === "h1" || b.t === "h2"}<h2 id={`h-${id}-${i}`}>{@render rs(b.runs)}</h2>
    {:else if b.t === "h3"}<h3 id={`h-${id}-${i}`}>{@render rs(b.runs)}</h3>
    {:else if b.t === "p"}<p>{@render rs(b.runs)}</p>
    {:else if b.t === "ul"}<ul>{#each b.items as li, k (k)}<li>{@render rs(li)}</li>{/each}</ul>
    {:else if b.t === "ol"}<ol>{#each b.items as li, k (k)}<li>{@render rs(li)}</li>{/each}</ol>
    {:else if b.t === "todo"}<div class="todo">{#each b.ids as t (t)}{#if ITEMS[t]}<TaskRow item={ITEMS[t]!} quiet onopen={(it) => wins.openItem(it)} />{/if}{/each}</div>
    {:else if b.t === "quote"}<blockquote><p>{@render rs(b.runs)}</p>{#if b.by}<cite>{b.by}</cite>{/if}</blockquote>
    {:else if b.t === "code"}<figure class="code"><figcaption>{b.lang}</figcaption><pre><code>{b.text}</code></pre></figure>
    {:else if b.t === "table"}<div class="tbw" tabindex="0" role="region" aria-label="Table"><table><thead><tr>{#each b.head as h (h)}<th>{h}</th>{/each}</tr></thead><tbody>{#each b.rows as r, k (k)}<tr>{#each r as c, j (j)}<td>{c}</td>{/each}</tr>{/each}</tbody></table></div>
    {:else if b.t === "image"}<figure class="img"><div class="pic" style:--h={b.hue} role="img" aria-label={b.caption}><Glyph name="image" /></div><figcaption>{b.caption}</figcaption></figure>
    {:else}<hr />{/if}
  {/each}
</div>

<style>
  .bl { display: grid; gap: var(--gap, var(--s-5)); font-size: var(--body, var(--text-ui)); line-height: var(--lh, 1.6); font-weight: var(--body-weight); }
  .bl > :global(*) { margin: 0; min-width: 0; }
  h2 { margin-top: var(--s-6) !important; margin-bottom: calc(var(--s-3) * -1) !important; font-size: var(--h2, var(--text-heading)); line-height: 1.3; font-weight: var(--strong-weight); text-wrap: balance; scroll-margin-top: var(--s-6); }
  h3 { margin-top: var(--s-4) !important; margin-bottom: calc(var(--s-3) * -1) !important; font-size: var(--body, var(--text-ui)); line-height: 1.4; font-weight: var(--strong-weight); color: var(--ink-2); scroll-margin-top: var(--s-6); }
  h2:first-child, h3:first-child { margin-top: 0 !important; }
  p { text-wrap: pretty; } strong { font-weight: var(--strong-weight); } em { font-style: italic; }
  ul, ol { padding-inline-start: var(--s-8); display: grid; gap: var(--s-2); } ul { list-style: disc; } ol { list-style: decimal; } li::marker { color: var(--ink-3); } ol li::marker { font-variant-numeric: tabular-nums; }
  a { color: var(--accent); text-decoration: underline; text-decoration-color: color-mix(in oklab, var(--accent) 40%, transparent); text-underline-offset: 0.18em; transition: text-decoration-color var(--dur-fast) var(--ease-out); } a:hover { text-decoration-color: currentColor; }
  code { font-family: var(--font-mono); font-size: 0.92em; padding: 0 var(--s-2); border-radius: var(--s-2); background: rgb(var(--ink-rgb) / 0.07); }
  mark { color: inherit; background: color-mix(in oklab, var(--accent-base) 14%, transparent); border-radius: var(--s-1); padding: 0 var(--s-1); margin: 0 calc(var(--s-1) * -1); }
  .caret { display: inline-block; width: 2px; height: 1.15em; margin-inline: 1px; vertical-align: text-bottom; border-radius: 1px; background: var(--accent-base); animation: blink var(--dur-pulse) steps(1) infinite; } @keyframes blink { 50% { opacity: 0; } }
  @media (prefers-reduced-motion: reduce) { .caret { animation: none; } }
  blockquote { padding-inline-start: var(--s-6); border-inline-start: 3px solid rgb(var(--ink-rgb) / 0.22); color: var(--ink-2); } blockquote p { font-size: var(--quote, var(--text-heading)); line-height: 1.45; } cite { display: block; margin-top: var(--s-3); color: var(--ink-3); font-size: var(--text-label); font-style: normal; } cite::before { content: "— "; }
  .code { position: relative; border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); overflow: hidden; } figcaption { color: var(--ink-3); }
  .code figcaption { padding: var(--s-4) var(--s-6) 0; font-size: var(--text-section); text-transform: uppercase; letter-spacing: 0.05em; } pre { margin: 0; padding: var(--s-3) var(--s-6) var(--s-5); overflow-x: auto; tab-size: 2; } pre code { padding: 0; background: none; font-size: var(--code, var(--text-label)); line-height: 1.55; border-radius: 0; }
  .tbw { overflow-x: auto; border-radius: var(--r-menu); box-shadow: inset 0 0 0 1px var(--line); } table { border-collapse: collapse; width: 100%; min-width: 22rem; font-size: var(--body, var(--text-ui)); line-height: 1.4; font-variant-numeric: tabular-nums; }
  th, td { padding: var(--s-3) var(--s-5); text-align: start; border-top: 1px solid var(--line); white-space: nowrap; } th { border-top: 0; background: rgb(var(--ink-rgb) / 0.05); font-weight: var(--strong-weight); color: var(--ink); } td:first-child { font-weight: var(--ui-weight); }
  .img { display: grid; gap: var(--s-3); } .pic { display: grid; place-items: center; aspect-ratio: 16 / 9; border-radius: var(--r-menu); color: rgb(255 255 255 / 0.75); background: radial-gradient(60% 80% at 75% 20%, oklch(0.93 0.1 calc(var(--h) - 120) / 0.9), transparent 60%), linear-gradient(160deg, oklch(0.78 0.09 var(--h)), oklch(0.55 0.1 calc(var(--h) + 30))); }
  .img figcaption { text-align: center; font-size: var(--text-label); line-height: 1.4; }
  hr { border: 0; height: 1px; background: var(--line); margin-block: var(--s-3) !important; }
</style>
