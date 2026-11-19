<script lang="ts">
  // The one bar (ADR 0035, #269): the glass capsule at the bottom that every mode lives in (tabs,
  // note tools, text bar, block bar, the sheets that grow out of it, toasts, questions). One
  // component, so all of them get the same glass, blur halo, spring and focus rules; what it shows
  // comes from the app's registered modes (modes.ts), chosen by pick.ts.
  //
  // Nothing in it ever takes focus from the text (#268): a press keeps focus where it is, so a
  // phone keyboard never drops and comes back. Only a real field (a sheet's search or link box)
  // takes focus.
  import type { Snippet } from "svelte";
  import Morph from "../components/Morph.svelte";
  import type { BarRole } from "./modes";

  let {
    key,
    role,
    label,
    phone = false,
    away = false,
    dragY = 0,
    dragging = false,
    outside,
    wrap = $bindable(),
    plain,
    content,
  }: {
    key: string;
    role: BarRole;
    label: string;
    /** The phone layout: fixed to the screen's bottom, bigger targets, no halo. */
    phone?: boolean;
    /** Hidden in place for a moment (the phone keyboard closing under it). */
    away?: boolean;
    /** A sheet being dragged down to close. */
    dragY?: number;
    /** A finger is dragging the sheet: it follows 1:1, with no spring behind it. */
    dragging?: boolean;
    /** On a phone, a tap anywhere outside the open sheet calls this, and does nothing else. Nothing
        dims behind a sheet (IN5, #660): the catcher is clear. */
    outside?: (() => void) | null;
    /** The bar's outer element, for the app to position (riding on the keyboard). */
    wrap?: HTMLDivElement;
    /** Layers that only fade, with no blur clearing (tall sheets), see Morph. */
    plain?: (key: string) => boolean;
    content: Snippet<[string]>;
  } = $props();

  // How: the press's mousedown is cancelled, which keeps focus where it is. For a mouse that's its
  // pointerdown too; never for a finger (#290): cancelling a touch's pointerdown makes iOS drop the
  // tap's click, and the button only answers a long press. A touch's mousedown comes after the
  // finger lifts, and cancelling that keeps focus without losing the click.
  const FIELD = "input, textarea, select, [contenteditable]";
  function keepFocus(node: HTMLElement) {
    const hold = (e: Event) => {
      if (e instanceof PointerEvent && e.pointerType !== "mouse") return;
      if (!(e.target as Element).closest?.(FIELD)) e.preventDefault();
    };
    node.addEventListener("pointerdown", hold);
    node.addEventListener("mousedown", hold);
    return {
      destroy() {
        node.removeEventListener("pointerdown", hold);
        node.removeEventListener("mousedown", hold);
      },
    };
  }
</script>

{#if phone && outside}
  <!-- A tap outside the sheet closes it and lands on nothing under it (no caret, no link). Clear, never a dim. -->
  <button class="bar-catch" aria-label="Close {label}" tabindex="-1" onclick={outside}></button>
{/if}

<div class="bar-wrap" class:phone class:sheet={role === "dialog"} data-key={key} bind:this={wrap} use:keepFocus>
  <Morph halo={!phone} {away} {key} {role} {label} {dragY} {dragging} {plain} {content} />
</div>
