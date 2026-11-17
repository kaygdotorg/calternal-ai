<script lang="ts">
  // A number whose digits roll: the ones that change slide up and out as the new ones come up from
  // below. Tabular, so 9 → 10 only adds a column (the capsule grows to fit it smoothly).
  import { prefersReducedMotion } from "../lib/spring-engine";

  let { value }: { value: number } = $props();

  // Digits keyed by place (ones, tens, …) so only the ones that change move.
  const digits = $derived(
    String(value)
      .split("")
      .reverse()
      .map((d, place) => ({ d, place }))
      .reverse(),
  );
  const reduce = prefersReducedMotion();
  function rollIn(_: Element) {
    return reduce
      ? { duration: 0 }
      : { duration: 260, easing: (t: number) => 1 - Math.pow(1 - t, 3), css: (t: number) => `translate: 0 ${(1 - t) * 70}%; opacity: ${t}` };
  }
  function rollOut(_: Element) {
    return reduce
      ? { duration: 0 }
      : { duration: 200, easing: (t: number) => t * t, css: (t: number) => `translate: 0 ${(t - 1) * 70}%; opacity: ${t}` };
  }
</script>

<span class="roll">
  {#each digits as { d, place } (place)}
    <span class="col">
      {#key d}<span class="digit" in:rollIn out:rollOut>{d}</span>{/key}
    </span>
  {/each}
</span>

<style>
  .roll {
    display: inline-flex;
    font-variant-numeric: tabular-nums;
  }
  .col {
    display: inline-grid;
    overflow: hidden;
  }
  .digit {
    grid-area: 1 / 1;
  }
</style>
