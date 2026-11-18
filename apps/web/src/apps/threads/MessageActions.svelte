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
    menu = false;
    if (id === "quote") bridge.comment(text);
    else if (id === "link") void navigator.clipboard?.writeText(`${location.origin}${location.pathname}#${message.id}`).then(() => toast("Link copied"));
  }
</script>

<!-- The time and the actions on one line, as in the calternal-ui plugin's hover logic (frame/msg-times.ts): an agent's actions come before its time, your own after it. With a mouse the actions' box takes no room until the
     message is hovered or focused (or its menu is open), then opens to its width over --dur-base on --ease-out with a margin beside the time, so the time slides aside as the icons fade in. The box is always there (a cheap
     element) so it can open by transition; its buttons are only created once the message is armed. -->
{#snippet acts()}
  <span class="acts" data-open={menu ? "" : undefined}>
    {#if armed}
      <IconButton small icon="copy" label="Copy message" onclick={copy} />
      <IconButton small icon="threads" label="Reply in side chat" onclick={() => bridge.ask(text)} />
      <IconButton small icon="more" label="Message actions" aria-haspopup="menu" aria-expanded={menu} bind:el={more} onclick={() => (menu = !menu)} />
    {/if}
  </span>
{/snippet}
<div class="meta {message.role}">
  {#if message.role === "agent"}{@render acts()}<span class="t">{time}</span>{:else}<span class="t">{time}</span>{@render acts()}{/if}
  {#if armed}<Menu open={menu} {items} label="Message actions" trigger={more} onclose={() => (menu = false)} onpick={pick} />{/if}
</div>

<style>
  .meta { display: flex; align-items: center; min-height: var(--ib-small); color: var(--ink-3); font-size: var(--text-label); }
  .meta.user { justify-content: flex-end; }
  .t { font-variant-numeric: tabular-nums; }
  .acts { display: inline-flex; align-items: center; }
  /* A touch screen has no hover: the actions are always there, beside the time. */
  @media (hover: none) { .acts { margin-inline: var(--s-4) 0; } .meta.agent .acts { margin-inline: 0 var(--s-4); } }
  /* A mouse: closed they take no room and are invisible; open they have their width (12rem at most) and margin beside the time. */
  @media (hover: hover) and (pointer: fine) {
    .acts { max-width: 0; overflow: hidden; opacity: 0; transition: max-width var(--dur-base) var(--ease-out), margin var(--dur-base) var(--ease-out), opacity var(--dur-base) var(--ease-out); }
    :global(.msg:hover) .acts, .acts:focus-within, .acts[data-open] { max-width: 12rem; opacity: 1; }
    :global(.msg:hover) .meta.agent .acts, .meta.agent .acts:focus-within, .meta.agent .acts[data-open] { margin-right: var(--s-4); }
    :global(.msg:hover) .meta.user .acts, .meta.user .acts:focus-within, .meta.user .acts[data-open] { margin-left: var(--s-4); }
  }
</style>
