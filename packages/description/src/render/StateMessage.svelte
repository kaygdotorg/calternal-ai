<script lang="ts">
  // A bind state's message (loading, empty, error, refine_query): the description's own title, detail and icon, with its optional action. The text is the author's or the host's; this component adds none.
  import { Icon } from "@calternal/ui";
  import type { State } from "../model";
  import { knownIcon } from "./icons";
  let { message, tone = "neutral", live = "status", onaction }: { message: State; tone?: "neutral" | "bad"; live?: "status" | "alert"; onaction?: () => void } = $props();
  const icon = $derived(typeof message.icon === "string" ? knownIcon(message.icon) : undefined);
</script>

<div class="msg" class:bad={tone === "bad"} role={live} data-state-message>
  {#if icon}<span class="ic"><Icon name={icon} /></span>{/if}
  <p class="t">{message.title}</p>
  {#if message.detail}<p class="d">{message.detail}</p>{/if}
  {#if message.action && onaction}<button type="button" class="act" onclick={onaction}>{(message.action as { title?: string }).title ?? ""}</button>{/if}
</div>

<style>
  .msg { display: grid; justify-items: center; gap: var(--s-3); padding: var(--s-8) var(--s-6); text-align: center; color: var(--ink-2); }
  .ic { color: var(--ink-3); }
  .t { margin: 0; color: var(--ink); font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .d { margin: 0; font-size: var(--text-label); color: var(--ink-3); }
  .bad .t { color: var(--danger); }
  .act { min-height: 44px; padding: 0 var(--s-5); color: var(--accent); }
</style>
