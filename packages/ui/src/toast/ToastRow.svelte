<script lang="ts">
  // What a toast says: a mark for waits, Undone and errors, the words (each letter its own span, so
  // a change ripples through), a rolling ×count for a burst, and its action (Undo, Retry). Drawn by
  // the stack's capsule (ToastCapsule) or by the app's bottom bar (#145), the same in both.
  //
  // Screen readers get one plain sentence (the letters you see are decoration), and only once it
  // has been still for a moment, so a burst of twelve isn't read out twelve times.
  import Roll from "./Roll.svelte";
  import type { ToastView } from "./notify.svelte";

  let { v }: { v: ToastView } = $props();

  // The sentence for screen readers settles a moment after the last change.
  let said = $state("");
  let sayTimer = 0;
  const sentence = $derived(v ? `${v.text}${v.count > 1 ? `, ${v.count} times` : ""}` : "");
  $effect(() => {
    const s = sentence;
    clearTimeout(sayTimer);
    sayTimer = window.setTimeout(() => (said = s), said ? 700 : 0);
    return () => clearTimeout(sayTimer);
  });

  // Each letter is its own span so a change of words can ripple through (keyed by the words, so
  // only a real change replays it). Letters are grouped by word, and a word never breaks (#434):
  // the line can only wrap at the spaces between words. `at` is the letter's place in the whole
  // sentence, so the ripple runs straight on across words.
  const words = $derived.by(() => {
    let at = 0;
    return (v?.text ?? "").split(" ").map((w) => {
      const start = at;
      at += [...w].length + 1;
      return { start, letters: [...w] };
    });
  });
</script>

{#if v.phase === "loading" || v.phase === "undoing"}
  <span class="lead spin" aria-hidden="true"></span>
{:else if v.phase === "undone"}
  <span class="lead tick" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M6 12.6l4 4L18.2 8.3" /></svg></span>
{:else if v.severity === "error"}
  <span class="lead bang" aria-hidden="true">!</span>
{/if}
{#key v.text}
  <span class="text" aria-hidden="true">
    {#each words as w, n (n)}{#if n > 0}{" "}{/if}<span class="word">{#each w.letters as ch, i (i)}<span class="ch" style="--i: {w.start + i}">{ch}</span>{/each}</span>{/each}
  </span>
{/key}
{#if v.count > 1}<span class="count" aria-hidden="true" data-count={v.count}>×<Roll value={v.count} /></span>{/if}
{#if v.action}
  <button class="act bounce" type="button" disabled={v.action.busy} onclick={() => v.action?.run()}>
    {v.action.busy ? "Undoing…" : v.action.label}
  </button>
{/if}
<span class="sr">{said}</span>

<style>
  .text {
    min-width: 0;
  }
  /* A word is one piece: the text wraps between words, never inside one (#434). */
  .word {
    white-space: nowrap;
  }
  .ch {
    display: inline-block;
    filter: blur(0);
    animation: letter 260ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
    animation-delay: calc(var(--i) * 9ms);
  }
  @keyframes letter {
    from {
      opacity: 0;
      translate: 0 0.25em;
      filter: blur(2px);
    }
    to {
      filter: blur(0);
    }
  }
  .count {
    display: inline-flex;
    align-items: baseline;
    color: var(--ink-3, #8a847c);
    font-weight: 600;
  }
  .lead {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    margin-left: -4px;
  }
  .spin {
    box-sizing: border-box;
    border-radius: 50%;
    border: 2px solid color-mix(in oklab, currentColor 22%, transparent);
    border-top-color: currentColor;
    animation: spin 700ms linear infinite;
  }
  @keyframes spin {
    to {
      rotate: 1turn;
    }
  }
  .tick svg {
    width: calc(16 * var(--icon-px, 1px));
    height: calc(16 * var(--icon-px, 1px));
    fill: none;
    stroke: var(--accent, #c8862a);
    stroke-width: 2.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .bang {
    border-radius: 50%;
    background: var(--danger, #c4473a);
    color: #fff;
    font-size: var(--text-section, 0.6875rem);
    font-weight: 700;
  }
  .act {
    flex: none;
    height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 999px;
    font: inherit;
    font-weight: 600;
    color: var(--toast-action-ink, var(--accent-ink, #fff));
    background: var(--toast-action, var(--accent, #c8862a));
    cursor: pointer;
    transition:
      opacity 150ms ease-out,
      scale 150ms ease-out;
  }
  .act:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .act:focus-visible {
    outline: 2px solid var(--ink, #111);
    outline-offset: 2px;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  @media (prefers-reduced-motion: reduce) {
    .ch {
      animation: none;
    }
    .spin {
      animation-duration: 1.6s;
    }
  }
</style>
