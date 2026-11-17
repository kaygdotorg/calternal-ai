<script lang="ts">
  // The app switcher as a bounded capsule: at most three apps (the open one is always among them), and "More" opens the full list when there are more.
  // It sits at the bottom of the middle card (one of the two places being compared; the other is the sidebar's footer). Always visible by default.
  import { Capsule, Menu, type MenuItem } from "@calternal/ui";
  import { APPS, type AppDef } from "../apps/registry";
  import { navigate } from "../lib/route.svelte";
  let { app }: { app: AppDef } = $props();
  const MAX = 3;
  // svelte-ignore state_referenced_locally
  let sel = $state(app.kind);
  $effect(() => { sel = app.kind; });
  let menu = $state(false);
  let box = $state<HTMLElement>();
  const overflow = $derived(APPS.length > MAX);
  const shown = $derived.by(() => {
    if (!overflow) return APPS;
    const head = APPS.slice(0, MAX - 1);
    return head.some((a) => a.kind === app.kind) ? head : [...head.slice(0, MAX - 2), app];
  });
  // Labels only, no icons: the capsule is a row of words on the pill's own glass.
  const items = $derived([...shown.map((a) => ({ id: a.kind, label: a.name })), ...(overflow ? [{ id: "__more", label: "More" }] : [])]);
  const all = $derived<MenuItem[]>(APPS.map((a) => ({ id: a.kind, label: a.name, icon: a.icon, checked: a.kind === app.kind })));
  function change(id: string) {
    if (id === "__more") { sel = app.kind; menu = true; return; }
    navigate(`/${id}`);
  }
</script>

<div class="switcher" bind:this={box}>
  <Capsule label="Apps" items={items} bind:value={sel} onchange={change} />
  {#if overflow}<Menu open={menu} items={all} label="All apps" trigger={box} onclose={() => (menu = false)} onpick={(k) => navigate(`/${k}`)} />{/if}
</div>

<style>
  .switcher { display: flex; justify-content: center; width: 100%; }
  .switcher :global(.cap) { width: 100%; max-width: 20rem; }
</style>
