<script lang="ts">
  // Draws parsed Markdown (lib/md.ts). Links open in a new tab with noopener. Code blocks scroll sideways, never wrap.
  import { scrollCue } from "@calternal/ui";
  import { parseMore, type Inline, type Parsed } from "./md";
  let { text, caret = false }: { text: string; caret?: boolean } = $props();
  // A message that grows (a streaming one) keeps the blocks it has finished and parses only its last, unfinished block again.
  let kept: Parsed | undefined;
  const blocks = $derived((kept = parseMore(kept, text)).blocks);
</script>

{#snippet inl(xs: Inline[])}
  {#each xs as x}
    {#if x.t === "text"}{x.v}{:else if x.t === "code"}<code spellcheck="false" translate="no">{x.v}</code>{:else if x.t === "b"}<strong>{x.v}</strong>{:else if x.t === "i"}<em>{x.v}</em>{:else}<a href={x.href} target="_blank" rel="noopener noreferrer">{x.v}</a>{/if}
  {/each}
{/snippet}

<div class="md selectable">
  {#each blocks as b, bi}
    {#if b.t === "p"}<p>{@render inl(b.v)}{#if caret && bi === blocks.length - 1}<span class="caret" aria-hidden="true"></span>{/if}</p>
    {:else if b.t === "h"}<p class="h" role="heading" aria-level="2">{@render inl(b.v)}</p>
    {:else if b.t === "ul"}<ul>{#each b.v as li}<li>{@render inl(li)}</li>{/each}</ul>
    {:else if b.t === "ol"}<ol>{#each b.v as li}<li>{@render inl(li)}</li>{/each}</ol>
    {:else if b.t === "img"}<img class="media" data-media src={b.src} alt={b.alt} loading="lazy" decoding="async" />
    {:else}<!-- svelte-ignore a11y_no_noninteractive_tabindex --><pre use:scrollCue={"x"} tabindex="0" spellcheck="false" translate="no" autocapitalize="off" aria-label="Code{b.lang ? `, ${b.lang}` : ''}"><code>{b.v}</code></pre>{/if}
  {/each}
  {#if caret && !(blocks.at(-1)?.t === "p")}<span class="caret" aria-hidden="true"></span>{/if}
</div>

<style>
  .md { display: grid; gap: var(--s-4); font-size: var(--text-ui); line-height: 1.55; font-weight: var(--body-weight); overflow-wrap: anywhere; }
  .h { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  ul, ol { display: grid; gap: var(--s-2); padding-inline-start: var(--s-7); }
  ul { list-style: disc; } ol { list-style: decimal; }
  li::marker { color: var(--ink-3); }
  code { font-family: var(--font-mono); font-size: var(--text-code); padding: 0 var(--s-2); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.07); }
  .media { display: block; max-width: 100%; max-height: 22rem; border-radius: var(--r-card); object-fit: cover; }
  pre { margin: 0; padding: var(--s-5) var(--s-6); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); overflow-x: auto; overscroll-behavior-x: contain; touch-action: pan-x pan-y; /* a code block scrolls sideways under a finger even inside the card, whose own touch-action is pan-y for the sidebar pull (a scroller is the end of that intersection) */ font-size: var(--text-label); line-height: 1.6; }
  /* Code is never underlined: no spell-check squiggle, no auto-detected link, no translation, whatever the browser would add. */
  pre, pre *, code { text-decoration: none !important; text-decoration-line: none !important; -webkit-text-decoration: none; text-underline-offset: 0; }
  /* A line wider than the block: a soft fade at the side where the code continues (and the thin scrollbar), so it never just looks cut. */
  pre:global([data-more-end]) { -webkit-mask-image: linear-gradient(to right, #000 calc(100% - var(--s-9)), transparent); mask-image: linear-gradient(to right, #000 calc(100% - var(--s-9)), transparent); scrollbar-width: thin; }
  pre:global([data-more-start]) { -webkit-mask-image: linear-gradient(to left, #000 calc(100% - var(--s-9)), transparent); mask-image: linear-gradient(to left, #000 calc(100% - var(--s-9)), transparent); scrollbar-width: thin; }
  pre:global([data-more-start][data-more-end]) { -webkit-mask-image: linear-gradient(to right, transparent, #000 var(--s-9), #000 calc(100% - var(--s-9)), transparent); mask-image: linear-gradient(to right, transparent, #000 var(--s-9), #000 calc(100% - var(--s-9)), transparent); }
  pre code { padding: 0; background: none; font-size: inherit; }
  a { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; }
  strong { font-weight: var(--strong-weight); }
  .caret { display: inline-block; width: 0.5em; height: 1.05em; margin-left: 2px; vertical-align: -0.15em; border-radius: 2px; background: var(--accent); animation: blink 1s steps(2, start) infinite; }
  @keyframes blink { to { visibility: hidden; } }
  @media (prefers-reduced-motion: reduce) { .caret { animation: none; } }
</style>
