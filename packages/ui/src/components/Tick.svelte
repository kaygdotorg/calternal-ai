<script lang="ts">
  // The one check mark of the product: a filled disc that swells in on the spring (it overshoots a little and settles) with the tick drawn on over it, 200 ms. Everything that is done or copied or chosen shows this
  // (a finished onboarding step, a copied command), so there is one motion to learn. `animate` is for a pointer or a step finishing by itself; a key-driven change passes `animate={false}` and the mark is there at
  // once (a keyboard user is not made to watch). Reduced motion: no swell and no draw-on, only a fade. The accessible name is the caller's: the mark itself is decorative.
  let { checked = false, animate = true, size = "var(--s-8)" }: { checked?: boolean; animate?: boolean; size?: string } = $props();
</script>

<span class="tick" class:on={checked} class:still={!animate} style:--tick-size={size} aria-hidden="true">
  <svg viewBox="0 0 24 24" width="100%" height="100%" fill="none" stroke-linecap="round" stroke-linejoin="round">
    <circle class="disc" cx="12" cy="12" r="11" />
    <path class="mark" d="M7 12.5l3.4 3.4L17 9" pathLength="1" />
  </svg>
</span>

<style>
  .tick { display: inline-block; width: var(--tick-size); height: var(--tick-size); flex: none; }
  .disc { fill: var(--accent-base); transform-origin: 12px 12px; transform: scale(0); opacity: 0; transition: transform var(--dur-base) var(--spring), opacity var(--dur-fast) var(--ease-out); }
  .mark { stroke: #fff; stroke-width: 2.4; stroke-dasharray: 1; stroke-dashoffset: 1; transition: stroke-dashoffset 200ms var(--ease-out) 70ms; }
  .on .disc { transform: scale(1); opacity: 1; }
  .on .mark { stroke-dashoffset: 0; }
  .still .disc, .still .mark { transition: none; }
  @media (prefers-reduced-motion: reduce) { .disc { transform: none; transition: opacity var(--dur-fast) linear; } .mark { stroke-dashoffset: 0; opacity: 0; transition: opacity var(--dur-fast) linear; } .on .mark { opacity: 1; } }
</style>
