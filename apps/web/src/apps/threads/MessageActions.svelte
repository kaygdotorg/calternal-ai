<script lang="ts">
  // The line under a message: its time (always visible) and, beside it, three bare small icons (copy, reply in side chat, more) that fade in when the
  // message is hovered or one of them has keyboard focus. On a touch screen, where there is no hover, they stay visible. Each has a tooltip.
  import { IconButton, Menu, toast, type MenuItem } from "@calternal/ui";
  import { bridge } from "./compose.svelte";
  import type { Message } from "./model";
  // `armed`: the message has been pointed at or focused (or there is no hover). The three controls and the menu are only created then, so a long list does not
  // pay for hundreds of controls nobody has reached for.
  let { message, armed = false }: { message: Message; armed?: boolean } = $props();
  const text = $derived(message.blocks.flatMap((b) => (b.type === "text" ? [b.text] : [])).join("\n\n"));
  const time = $derived(new Date(message.at).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }));
  let menu = $state(false);
  let more = $state<HTMLButtonElement>();
  const items: MenuItem[] = [{ id: "quote", label: "Quote in the message box" }, { id: "link", label: "Copy link to this message" }];
  const copy = async () => { try { await navigator.clipboard.writeText(text); } catch { /* no clipboard: nothing to confirm */ } };
  function pick(id: string) {
    if (id === "quote") bridge.comment(text);
    else if (id === "link") void navigator.clipboard?.writeText(`${location.origin}${location.pathname}#${message.id}`).then(() => toast("Link copied"));
  }
</script>

<div class="meta {message.role}">
  <span class="t">{time}</span>
  {#if armed}<span class="acts">
    <IconButton small icon="copy" label="Copy message" onclick={copy} />
    <IconButton small icon="threads" label="Reply in side chat" onclick={() => bridge.ask(text)} />
    <IconButton small icon="more" label="Message actions" aria-haspopup="menu" aria-expanded={menu} bind:el={more} onclick={() => (menu = !menu)} />
  </span>
  <Menu open={menu} {items} label="Message actions" trigger={more} onclose={() => (menu = false)} onpick={pick} />{/if}
</div>

<style>
  .meta { display: flex; align-items: center; gap: var(--s-2); min-height: var(--ib-small); color: var(--ink-3); font-size: var(--text-label); }
  .meta.user { flex-direction: row-reverse; }
  .t { font-variant-numeric: tabular-nums; }
  .acts { display: inline-flex; align-items: center; opacity: 0; transition: opacity var(--dur-base) var(--ease-out); }
  :global(.msg:hover) .acts, .acts:focus-within { opacity: 1; }
  @media (hover: none) { .acts { opacity: 1; } }
</style>
