<script lang="ts">
  // An empty page: a glyph on a soft disc, one line saying what is empty and why, one line saying what to do, and (optionally) the action. Centred in the window, never a blank screen. Shared by Notes and Tasks.
  import { Button } from "@calternal/ui";
  import Glyph from "../../shell/AppIcon.svelte";
  let { icon, title, text, action }: { icon: string; title: string; text: string; action?: string } = $props();
</script>

<div class="em" data-empty>
  <span class="disc"><Glyph name={icon} /></span>
  <h3>{title}</h3>
  <p>{text}</p>
  {#if action}<Button text={action} variant="primary" icon="plus" />{/if}
</div>

<style>
  .em { display: grid; justify-items: center; align-content: center; gap: var(--s-4); min-height: 16rem; padding: var(--s-9) var(--s-6); text-align: center; animation: in var(--dur-base) var(--ease-out); }
  .disc { display: grid; place-items: center; width: var(--s-10); height: var(--s-10); border-radius: 50%; background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-3); margin-bottom: var(--s-2); }
  h3 { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.25; } p { margin: 0 0 var(--s-3); max-width: 22rem; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.45; }
  @keyframes in { from { opacity: 0; translate: 0 6px; } }
</style>
