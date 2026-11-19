<script lang="ts">
  // The info panel: one plain sentence in a tinted panel with an icon, for what went wrong (`bad`), what needs care (`warn`) or what to know (`info`). It is always in the page, empty and closed, so the card around it
  // never jumps: it opens by growing its row from nothing (the height animates, never snaps), the panel fades and rises into it, and on leaving it goes the same way back. A screen reader hears the sentence ONCE, when it is
  // set: the announcement lives in a hidden live region (`alert` for `bad`, `status` otherwise) that is filled when `message` is set and emptied the moment it is cleared, apart from the painted copy, which stays through
  // the leaving animation. Motion: the shell's tokens (`--dur-base`, `--dur-leave`, `--ease-out`); under reduced motion the durations are 1 ms (tokens), so it changes at once. Interruptible: a message set while it is
  // leaving turns it round from where it is (CSS transitions reverse; there is no timer to wait for except the one that clears the painted copy, which a new message cancels).
  import Icon from "./Icon.svelte";
  import { haptic } from "../lib/haptics";
  import type { IconName } from "../icons";
  let { message = "", tone = "bad", icon, class: klass }: { message?: string | null; tone?: "bad" | "warn" | "info"; icon?: IconName; class?: string } = $props();
  const glyph = $derived(icon ?? (tone === "info" ? "info" : "warning"));
  /** The sentence being painted: kept for the leaving animation after `message` is cleared. */
  let shown = $state("");
  const open = $derived(!!message);
  let timer = 0;
  $effect(() => {
    if (message) { clearTimeout(timer); if (shown !== message) haptic(tone === "bad" ? "error" : tone === "warn" ? "warning" : "light"); shown = message; }
    else if (shown) { clearTimeout(timer); timer = window.setTimeout(() => { shown = ""; }, 260); }
    return () => clearTimeout(timer);
  });
</script>

<div class={["callout", klass]} data-open={open} data-tone={tone}>
  <div class="clip">
    <div class="panel">
      <span class="ic" aria-hidden="true"><Icon name={glyph} /></span>
      <p aria-hidden="true">{shown}</p>
    </div>
  </div>
  <div class="live" role={tone === "bad" ? "alert" : "status"} aria-atomic="true">{message ?? ""}</div>
</div>

<style>
  /* In a grid with a gap the closed panel would leave one gap of empty space: the negative margin cancels it while closed (`--callout-gap` is the parent's gap) and grows back to zero as the panel opens. */
  .callout { display: grid; grid-template-rows: 0fr; margin-top: calc(var(--callout-gap, var(--s-5)) * -1); transition: grid-template-rows var(--dur-leave) var(--ease-out), margin-top var(--dur-leave) var(--ease-out); }
  .callout[data-open="true"] { grid-template-rows: 1fr; margin-top: 0; transition-duration: var(--dur-base); }
  .clip { min-height: 0; overflow: hidden; }
  .panel {
    display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--s-4); align-items: start;
    padding: var(--s-4) var(--s-5); border-radius: var(--r-card); color: var(--ink); background: color-mix(in srgb, var(--tone) 13%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--tone) 30%, transparent);
    opacity: 0; translate: 0 calc(var(--s-3) * -1); transition: opacity var(--dur-leave) var(--ease-out), translate var(--dur-leave) var(--ease-out);
  }
  .callout[data-open="true"] .panel { opacity: 1; translate: 0 0; transition-duration: var(--dur-base); }
  .callout[data-tone="bad"] { --tone: var(--danger); }
  .callout[data-tone="warn"] { --tone: var(--warn); }
  .callout[data-tone="info"] { --tone: var(--ink-2); }
  .ic { display: grid; place-items: center; height: calc(var(--text-ui) * 1.4); color: var(--tone); }
  p { margin: 0; font-size: var(--text-ui); line-height: 1.4; overflow-wrap: anywhere; user-select: text; }
  /* The announcement: out of sight, in the accessibility tree. */
  .live { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .callout { position: relative; }
</style>
