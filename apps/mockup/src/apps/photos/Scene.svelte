<script lang="ts">
  // A generated picture (scene.ts): an SVG that fills its box and crops like a photograph (`slice`). It is decoration: hidden from screen readers; the caption or the page carries the name.
  import { buildScene, type SceneSpec } from "./scene";
  let { spec, ratio = "" }: { spec: SceneSpec; ratio?: string } = $props();
  const sc = $derived(buildScene(spec));
</script>

<svg class="sc" style:aspect-ratio={ratio || undefined} viewBox="0 0 160 120" preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
  <defs><linearGradient id={sc.id} x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color={sc.sky[0]} /><stop offset="1" stop-color={sc.sky[1]} /></linearGradient></defs>
  <rect width="160" height="120" fill={`url(#${sc.id})`} />
  {#each sc.shapes as s, i (i)}
    {#if s.d}<path d={s.d} fill={s.fill} opacity={s.opacity} />
    {:else if s.r}<circle cx={s.cx} cy={s.cy} r={s.r} fill={s.fill} opacity={s.opacity} />
    {:else}<rect x={s.x} y={s.y} width={s.w} height={s.h} fill={s.fill} opacity={s.opacity} />{/if}
  {/each}
</svg>

<style>
  .sc { display: block; width: 100%; height: 100%; }
</style>
