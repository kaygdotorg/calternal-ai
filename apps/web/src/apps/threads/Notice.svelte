<script lang="ts">
  // One notice above the composer: what is wrong or what happened, in plain words, and the next step as one or two buttons. No art, no dismiss: it goes away when the thing it names is
  // fixed. Built on the same glass panel as the request bar so the two stack as one family. A notice is a status for assistive technology (`role="status"`), never an alert.
  import { Button, Icon, Panel, type IconName } from "@calternal/ui";
  let { icon = "warning", title, children, actions = [] }: { icon?: IconName; title: string; children?: import("svelte").Snippet; actions?: { text: string; onclick: () => void; primary?: boolean }[] } = $props();
</script>

<Panel class="notice" radius="pane" dense role="status" aria-label={title}>
  <div class="in">
    <span class="ic"><Icon name={icon} /></span>
    <div class="tx">
      <h2>{title}</h2>
      {#if children}<p>{@render children()}</p>{/if}
    </div>
    {#if actions.length}<div class="act">{#each actions as a}<Button variant={a.primary ? "primary" : "quiet"} text={a.text} onclick={a.onclick} />{/each}</div>{/if}
  </div>
</Panel>

<style>
  :global(.notice) { color: var(--ink); }
  .in { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--s-4) var(--s-5); align-items: start; padding: var(--s-5) var(--s-6); }
  .ic { display: grid; place-items: center; height: calc(var(--text-heading) * 1.3); color: var(--ink-2); }
  h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.3; }
  p { margin-top: var(--s-2); color: var(--ink-2); font-size: var(--text-ui); }
  .act { grid-column: 2; display: flex; flex-wrap: wrap; gap: var(--s-3); }
  :global(.notice code) { font-family: var(--font-mono); font-size: var(--text-label); padding: 0 var(--s-2); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); user-select: text; }
</style>
