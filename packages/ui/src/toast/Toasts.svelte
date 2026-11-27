<script lang="ts">
  // Where toasts appear when there is no bottom bar to show them: a column above the bottom edge,
  // set up the Calternal way. Put it once near the root. It is a polite live region.
  // `bottom` keeps them clear of anything pinned to the bottom of the window (the app's bar): pixels,
  // or any CSS length (like one that adds the phone's home-indicator inset). Toasts stack apart,
  // never one over another (the glass is see-through, so a toast behind another read as a smear).
  // Swipe a toast down to dismiss it.
  import { visible, hold } from "./stack.svelte";
  import { dismiss } from "./notify.svelte";
  import ToastCapsule from "./ToastCapsule.svelte";
  import { prefersReducedMotion } from "../lib/spring-engine";

  let { bottom = 24 }: { bottom?: number | string } = $props();
  const len = (v: number | string) => (typeof v === "number" ? `${v}px` : v);

  // A toast that appears under a resting pointer gets no pointerenter: ask the browser.
  function mounted(el: HTMLElement, id: string) {
    if (el.matches(":hover")) hold(id, "mouse", true);
  }

  let y0: number | null = null;
  function down(e: PointerEvent, id: string) {
    if (e.pointerType !== "mouse") hold(id, "touch", true); // a finger resting on it holds it
    if ((e.target as Element).closest("button,a")) return;
    y0 = e.clientY;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (y0 === null) return;
    const el = e.currentTarget as HTMLElement;
    el.style.transition = "none";
    el.style.translate = `0 ${Math.max(0, e.clientY - y0)}px`;
  }
  function up(e: PointerEvent, id: string) {
    hold(id, "touch", false);
    if (y0 === null) return;
    const el = e.currentTarget as HTMLElement;
    const far = e.clientY - y0 > 40;
    y0 = null;
    if (!far) {
      el.style.transition = "";
      el.style.translate = "";
      return;
    }
    // Swipe away: slide on down and fade, then it is gone.
    if (prefersReducedMotion()) return dismiss(id);
    el.style.transition = "translate var(--dur-fast) var(--ease-out), opacity var(--dur-fast) var(--ease-out)";
    el.style.translate = "0 100%";
    el.style.opacity = "0";
    setTimeout(() => dismiss(id), 160);
  }
</script>

<section class="toasts" aria-label="Notifications" aria-live="polite" style:--b={len(bottom)}>
  {#each visible() as id (id)}
    <div
      class="item"
      role="group"
      data-toast
      use:mounted={id}
      onpointerdown={(e) => down(e, id)}
      onpointermove={move}
      onpointerup={(e) => up(e, id)}
      onpointercancel={(e) => up(e, id)}
      onpointerenter={(e) => e.pointerType === "mouse" && hold(id, "mouse", true)}
      onpointerleave={(e) => e.pointerType === "mouse" && hold(id, "mouse", false)}
      onfocusin={() => hold(id, "focus", true)}
      onfocusout={() => hold(id, "focus", false)}
    >
      <ToastCapsule {id} />
    </div>
  {/each}
</section>

<style>
  /* Our toasts size themselves; the column just needs to be wide enough for the longest. An app can
     centre them over something other than the window (say, the page above its bar) with --toast-x. */
  .toasts {
    position: fixed;
    z-index: 999999999;
    bottom: var(--b);
    left: var(--toast-x, 50%);
    translate: -50% 0;
    width: min(440px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  /* Phones: the column spans the window inside a 16 px gutter, a little higher (#434). */
  @media (max-width: 600px) {
    .toasts {
      left: 16px;
      translate: none;
      bottom: calc(var(--b) + 8px);
    }
  }
  .item {
    display: flex;
    justify-content: center;
    pointer-events: auto;
    touch-action: pan-x;
    transition: translate 160ms ease-out;
    animation: arrive 260ms cubic-bezier(0.21, 1.02, 0.73, 1) backwards;
  }
  /* A toast rises in; one that goes is gone at once (a quick cut), never drawn under the new one. */
  @keyframes arrive {
    from {
      opacity: 0;
      translate: 0 40%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .item {
      animation: none;
      transition: none;
    }
  }
</style>
