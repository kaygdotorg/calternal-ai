<script lang="ts">
  import { href } from "../../lib/route.svelte";
  // One message. The person's words sit in a tinted bubble on the right; the agent's are plain text on the card, with folded
  // steps, generated-UI cards and links to child threads between paragraphs. No inner card around a message.
  import type { Component } from "svelte";
  import { Button, Icon } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import MessageActions from "./MessageActions.svelte";
  import Steps from "./Steps.svelte";
  import type { Message } from "./model";
  import { store } from "./store.svelte";
  let { message }: { message: Message } = $props();
  // A very long message (a tool's whole output, say) is drawn in part: the first CAP characters, cut at a line end, and the person chooses to draw the rest. Drawing 5 MB of Markdown at once blocked the page
  // for 0.7 s (2.4 s at 4x CPU slowdown); the list virtualises messages, not the inside of one. The text is all there (copy takes all of it).
  const CAP = 64 * 1024;
  let all = $state(false);
  const clip = (t: string) => { if (all || t.length <= CAP) return t; const cut = t.lastIndexOf("\n", CAP); return t.slice(0, cut > CAP / 2 ? cut : CAP); };
  const size = (t: string) => (t.length >= 1048576 ? `${(t.length / 1048576).toFixed(1)} MB` : `${Math.round(t.length / 1024)} KB`);
  const long = $derived(message.blocks.find((b) => b.type === "text" && b.text.length > CAP && !all) as { type: "text"; text: string } | undefined);
  // The generated-UI renderer is its own chunk, fetched the first time a message holds a card; a thread of plain messages never downloads it.
  let GenUi = $state<Component<{ source: string }>>();
  $effect(() => { if (!GenUi && message.blocks.some((b) => b.type === "card")) void import("../../genui/GenUi.svelte").then((m) => (GenUi = m.default as never)); });
  // Message actions are created on first hover or focus (touch has no hover: they are there from the start).
  let armed = $state(typeof matchMedia === "function" && matchMedia("(hover: none)").matches);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="msg {message.role}" onpointerenter={() => (armed = true)} onfocusin={() => (armed = true)}>
  {#if message.role === "user"}
    <div class="col end"><div class="bubble">{#each message.blocks as b}{#if b.type === "text"}<Md text={clip(b.text)} />{/if}{/each}</div><MessageActions {message} {armed} /></div>
  {:else}
    <div class="agent" id={message.id}>
      {#each message.blocks as b}
        {#if b.type === "text"}<Md text={clip(b.text)} />
        {:else if b.type === "steps"}<Steps steps={b.steps} ms={b.ms} />
        {:else if b.type === "card"}{#if GenUi}<GenUi source={b.source} />{/if}
        {:else}
          {@const t = store.get(b.threadId)}
          {#if t}<a class="child" href={href(`/threads/${t.id}`)}><Icon name="branch" inline /><span>{t.title}</span><small>{t.state}</small></a>{/if}
        {/if}
      {/each}
      {#if long}<p class="clipped">This message is {size(long.text)}; the first 64 KB is shown. <Button variant="quiet" text="Show all" onclick={() => (all = true)} /></p>{/if}
      <MessageActions {message} {armed} />
    </div>
  {/if}
</div>

<style>
  .clipped { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-label); }
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) var(--s-4) var(--msg-pad-l, var(--pad-region)); display: flex; }
  .user { justify-content: flex-end; }
  .bubble { position: relative; max-width: 82%; padding: var(--s-4) var(--s-6); border-radius: var(--r-pane); background: rgb(var(--ink-rgb) / 0.07); }
  /* Your messages get an iMessage tail in the bubble's own fill (ported from the calternal-ui plugin): the bottom-right corner is square and a rounded point flares from that edge a little below the bubble. */
  .user .bubble { border-bottom-right-radius: 0; }
  .user .bubble::after { content: ""; position: absolute; right: -0.5rem; bottom: -0.125rem; width: 0.875rem; height: 1.25rem; background: rgb(var(--ink-rgb) / 0.07); pointer-events: none;
    -webkit-mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 14 20'%3E%3Cpath d='M6 0C6.2 8 8.4 14 12.2 17.6C13.3 18.3 13.5 19 12.6 19.4C10 20.1 7.5 19.9 5 19C3.4 18.5 1.8 18 0 18L6 18Z'/%3E%3C/svg%3E") center / 100% 100% no-repeat;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 14 20'%3E%3Cpath d='M6 0C6.2 8 8.4 14 12.2 17.6C13.3 18.3 13.5 19 12.6 19.4C10 20.1 7.5 19.9 5 19C3.4 18.5 1.8 18 0 18L6 18Z'/%3E%3C/svg%3E") center / 100% 100% no-repeat; }
  .agent > :global(.meta) { margin-top: calc(var(--s-4) * -1); }
  .col { display: grid; gap: var(--s-1); max-width: 82%; }
  .col.end { justify-items: end; }
  .col .bubble { max-width: none; }
  .agent { display: grid; gap: var(--s-5); min-width: 0; width: 100%; }
  .child { display: inline-flex; align-items: center; gap: var(--s-4); padding: var(--s-3) var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); justify-self: start; }
  .child small { color: var(--ink-3); font-weight: var(--ui-weight); }
</style>
