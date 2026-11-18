<script lang="ts">
  // The surface of a row's press (lib/press.ts mounts it once, on the first hold): the page dims and blurs behind (one backdrop layer that only fades), the preview card of the
  // held item grows out of the row by a clip (the row's own look-alike fades into the card, which opens to its size), and the row's menu (the shared Menu) sits under it. A tap
  // outside, Escape, picking an item or a drag of the card downwards puts it away: the opening played backwards, from wherever it is (it can be interrupted either way). A right
  // click or the menu key shows the menu alone, at the point, with no dimming. Motion is transform, opacity and the clip, on the motion tokens; reduced motion just shows it.
  // The preview is drawn from what is already loaded (`registerPreview`); nothing is fetched here, and nothing of this is in the page until the first hold.
  import { tick, untrack } from "svelte";
  import Menu from "./Menu.svelte";
  import { dimPage } from "../lib/dim-page";
  import { tokenEase, tokenMs } from "../lib/motion";
  import { pressState } from "../lib/press-state.svelte";
  import { previewOf } from "../lib/press";
  import type { Anchor } from "../lib/place";

  const cur = $derived(pressState.cur);
  const GAP = 10, M = 12, R = 16;
  let card = $state<HTMLElement>(), faceEl = $state<HTMLElement>(), copyHost = $state<HTMLElement>();
  let at = $state({ left: 0, top: 0, width: 0, height: 0 });
  let scrim = $state<HTMLElement>();
  let menuOpen = $state(false);
  let shown = $state.raw<typeof cur>(null);
  let anims: Animation[] = [];
  let closing = false;
  const reduce = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  const items = $derived(shown ? shown.bag.items() : []);
  const Face = $derived(shown?.preview && shown.bag.preview ? previewOf(shown.bag.preview.kind) : undefined);
  const anchor = $derived<Anchor | null>(shown ? (shown.preview ? { left: at.left, right: at.left + at.width, top: at.top, bottom: at.top + at.height } : { left: shown.x, right: shown.x, top: shown.y, bottom: shown.y }) : null);

  $effect(() => {
    const c = cur;
    untrack(() => { if (c) void open(c); else if (shown) void close(); });
  });

  async function open(c: NonNullable<typeof cur>) {
    closing = false; for (const a of anims) a.cancel(); anims = [];
    shown = c; menuOpen = false;
    if (!c.preview) { await tick(); menuOpen = true; return; }
    const r = c.row.getBoundingClientRect();
    const vw = innerWidth, vh = innerHeight;
    const pw = Math.min(vw - 2 * M, 360, Math.max(r.width, 300));
    at = { left: Math.max(M, Math.min(r.left + r.width / 2 - pw / 2, vw - M - pw)), top: r.top, width: pw, height: 0 };
    await tick();
    if (!card || shown !== c) return;
    // The card's height: what its content needs, up to a limit; measured in place, unseen.
    const ph = Math.max(r.height, Math.min(faceEl?.scrollHeight ?? r.height, 300, vh * 0.38));
    const menuH = Math.min(8, Math.max(1, items.length)) * 44 + 16;
    const top = r.top + ph + GAP + menuH > vh - M ? Math.max(M, vh - M - ph - GAP - menuH) : r.top;
    at = { left: at.left, top, width: pw, height: ph };
    await tick();
    if (!card || shown !== c) return;
    const copy = c.copy ?? undefined;
    if (copy && copyHost) {
      copy.style.cssText += `;position:absolute;width:${r.width}px;height:${r.height}px;left:${r.left - at.left}px;top:${r.top - top}px;background:var(--paper)`;
      copyHost.replaceChildren(copy);
    }
    dimPage(true);
    if (!reduce()) {
      const ms = tokenMs("--dur-base", 220), ease = tokenEase("--ease-out", "cubic-bezier(0.23, 1, 0.32, 1)");
      const inset = `inset(${r.top - top}px ${at.left + pw - r.right}px ${top + ph - r.bottom}px ${r.left - at.left}px round ${R - 4}px)`;
      anims.push(
        scrim!.animate([{ opacity: 0 }, { opacity: 1 }], { duration: ms, easing: ease, fill: "both" }),
        card.animate([{ clipPath: inset }, { clipPath: `inset(0 round ${R}px)` }], { duration: ms, easing: ease, fill: "both" }),
        faceEl!.animate([{ opacity: 0 }, { opacity: 0, offset: 0.4 }, { opacity: 1 }], { duration: ms, easing: ease, fill: "both" }),
        ...(copyHost ? [copyHost.animate([{ opacity: 1 }, { opacity: 1, offset: 0.2 }, { opacity: 0 }], { duration: ms, easing: ease, fill: "both" })] : []),
      );
    }
    menuOpen = true;
  }
  /** The way back: every part of the opening played backwards at once, from wherever it is. */
  async function close() {
    if (closing) return;
    closing = true; menuOpen = false;
    const live = anims.filter((a) => a.playState !== "idle");
    if (live.length) { for (const a of live) { a.playbackRate = -1.2; a.play(); } await Promise.allSettled(live.map((a) => a.finished)); }
    if (closing) { dimPage(false); anims = []; shown = null; }
  }
  const dismiss = () => { pressState.cur = null; };
  let drag: { y: number } | null = null;
  const dmove = (e: PointerEvent) => { if (drag && e.clientY - drag.y > 56) { drag = null; dismiss(); } };
</script>

<svelte:window onkeydown={(e) => { if (shown && e.key === "Escape") dismiss(); }} />

{#if shown}
  {#if shown.preview}
    <div class="scrim" bind:this={scrim} onpointerdown={dismiss} role="presentation"></div>
    <div class="card" bind:this={card} style:left="{at.left}px" style:top="{at.top}px" style:width="{at.width}px" style:height={at.height ? `${at.height}px` : "auto"} style:visibility={at.height ? "visible" : "hidden"}
      onpointerdown={(e) => { drag = { y: e.clientY }; }} onpointermove={dmove} onpointerup={() => (drag = null)} role="presentation">
      <div class="face" bind:this={faceEl}>{#if Face && shown.bag.preview}<Face item={shown.bag.preview.item as never} />{/if}</div>
      <div class="copy" bind:this={copyHost}></div>
    </div>
  {/if}
  <Menu open={menuOpen} {items} label={shown.bag.label} {anchor} placement="bottom" onpick={(id) => { dismiss(); shown?.bag.onpick(id); }} onclose={dismiss} />
{/if}

<style>
  /* One blur layer that only fades; the page behind is dimmed by it (the scrim ink at its amount). */
  .scrim { position: fixed; inset: 0; z-index: 40; background: color-mix(in oklab, var(--paper) 40%, transparent); backdrop-filter: blur(18px); -webkit-backdrop-filter: blur(18px); }
  .card { position: fixed; z-index: 41; overflow: hidden; border-radius: 16px; background: var(--paper); box-shadow: 0 12px 40px rgb(0 0 0 / 0.28); touch-action: none; }
  .face { height: 100%; overflow: hidden; -webkit-mask-image: linear-gradient(#000 calc(100% - 28px), transparent); mask-image: linear-gradient(#000 calc(100% - 28px), transparent); }
  .copy { position: absolute; inset: 0; pointer-events: none; }
</style>
