<script lang="ts">
  // A hand-held, step by step form inside a page: a numbered stepper (done steps become ticks), one step at a time with its heading focused when it appears, Back and Next, and a last button that finishes.
  // Keyboard: Enter in a field moves on when the step is valid. Motion is the shared tokens; reduced motion fades only. Used by the Skills and Automations pages.
  import { Button, Tick } from "@calternal/ui";
  import type { Snippet } from "svelte";
  let { steps, index = $bindable(0), valid, busy = false, finish, onfinish, oncancel, children }: {
    /** The step names, in order. */ steps: string[];
    /** The step being shown. */ index?: number;
    /** Whether the step can be left forward; `error` is what to say when not (shown under the step once the person tries). */ valid: (i: number) => string | null;
    /** True while the last step is working. */ busy?: boolean;
    /** The last button's text. */ finish: string;
    /** Called when the last step is confirmed. */ onfinish: () => void;
    /** Called when the person cancels. */ oncancel: () => void;
    /** The step's content, given its index. */ children: Snippet<[number]>;
  } = $props();
  let tried = $state(false), heading = $state<HTMLElement>();
  const problem = $derived(valid(index));
  const last = $derived(index === steps.length - 1);
  function next() { tried = true; if (problem) return; tried = false; if (!last) index += 1; else onfinish(); }
  $effect(() => { void index; queueMicrotask(() => heading?.focus({ preventScroll: true })); });
</script>

<div class="wz" onkeydown={(e) => { if (e.key === "Enter" && !(e.target instanceof HTMLTextAreaElement) && !(e.target instanceof HTMLButtonElement)) { e.preventDefault(); next(); } }} role="presentation">
  <ol class="st" aria-label="Steps">
    {#each steps as s, i (s)}
      <li class:now={i === index} aria-current={i === index ? "step" : undefined}><span class="d">{#if i < index}<Tick checked animate={false} size="var(--s-8)" />{:else}{i + 1}{/if}</span><span class="l">{s}</span></li>
    {/each}
  </ol>
  <section class="in">
    {#key index}
      <h3 bind:this={heading} tabindex="-1">{steps[index]}</h3>
      {@render children(index)}
    {/key}
    {#if tried && problem}<p class="e" role="alert">{problem}</p>{/if}
    <footer>
      <Button variant="quiet" text="Cancel" onclick={oncancel} />
      {#if index > 0}<Button variant="quiet" text="Back" onclick={() => { tried = false; index -= 1; }} />{/if}
      <Button variant="primary" text={last ? finish : "Next"} disabled={busy} onclick={next} />
    </footer>
  </section>
  <p class="sr" role="status" aria-live="polite">Step {index + 1} of {steps.length}: {steps[index]}</p>
</div>

<style>
  /* A long line (a description) wraps inside the card; it never widens it and pushes the buttons out of view. */
  .wz :global(pre) { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; }
  .wz, .in { min-width: 0; }
  .wz { display: grid; gap: var(--s-5); padding: var(--s-5) var(--s-6); }
  .st { display: flex; gap: var(--s-5); list-style: none; margin: 0; padding: 0; flex-wrap: wrap; }
  .st li { display: flex; align-items: center; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-label); }
  .st li.now { color: var(--ink); }
  .d { width: var(--s-8); height: var(--s-8); display: grid; place-items: center; border-radius: 50%; background: rgb(var(--ink-rgb) / 0.08); font-weight: var(--strong-weight); }
  .now .d { background: var(--accent-soft, rgb(var(--ink-rgb) / 0.16)); }
  .in { display: grid; gap: var(--s-4); animation: enter var(--dur-base) var(--ease-out); }
  h3 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); outline: none; }
  footer { display: flex; gap: var(--s-4); flex-wrap: wrap; justify-content: flex-end; }
  .e { margin: 0; color: var(--danger); font-size: var(--text-label); }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @keyframes enter { from { opacity: 0; translate: 0 var(--s-3); } }
  @media (prefers-reduced-motion: reduce) { .in { animation: fade var(--dur-fast) linear; } @keyframes fade { from { opacity: 0; } } }
</style>
