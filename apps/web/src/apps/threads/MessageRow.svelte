<script lang="ts">
  import { href } from "../../lib/route.svelte";
  // One message. The person's words sit in a tinted bubble on the right; the agent's are plain text on the card, with folded
  // steps, generated-UI cards and links to child threads between paragraphs. No inner card around a message.
  import { Icon } from "@calternal/ui";
  import GenUi from "../../genui/GenUi.svelte";
  import Md from "../../lib/Md.svelte";
  import MessageActions from "./MessageActions.svelte";
  import Steps from "./Steps.svelte";
  import type { Message } from "./model";
  import { store } from "./store.svelte";
  let { message }: { message: Message } = $props();
  // Message actions are created on first hover or focus (touch has no hover: they are there from the start).
  let armed = $state(typeof matchMedia === "function" && matchMedia("(hover: none)").matches);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="msg {message.role}" onpointerenter={() => (armed = true)} onfocusin={() => (armed = true)}>
  {#if message.role === "user"}
    <div class="col end"><div class="bubble">{#each message.blocks as b}{#if b.type === "text"}<Md text={b.text} />{/if}{/each}</div><MessageActions {message} {armed} /></div>
  {:else}
    <div class="agent" id={message.id}>
      {#each message.blocks as b}
        {#if b.type === "text"}<Md text={b.text} />
        {:else if b.type === "steps"}<Steps steps={b.steps} ms={b.ms} />
        {:else if b.type === "card"}<GenUi source={b.source} />
        {:else}
          {@const t = store.get(b.threadId)}
          {#if t}<a class="child" href={href(`/threads/${t.id}`)}><Icon name="branch" inline /><span>{t.title}</span><small>{t.state}</small></a>{/if}
        {/if}
      {/each}
      <MessageActions {message} {armed} />
    </div>
  {/if}
</div>

<style>
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--pad-region); display: flex; }
  .user { justify-content: flex-end; }
  .bubble { max-width: 82%; padding: var(--s-4) var(--s-6); border-radius: var(--r-pane); background: rgb(var(--ink-rgb) / 0.07); }
  .agent > :global(.meta) { margin-top: calc(var(--s-4) * -1); }
  .col { display: grid; gap: var(--s-1); max-width: 82%; }
  .col.end { justify-items: end; }
  .col .bubble { max-width: none; }
  .agent { display: grid; gap: var(--s-5); min-width: 0; width: 100%; }
  .child { display: inline-flex; align-items: center; gap: var(--s-4); padding: var(--s-3) var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); justify-self: start; }
  .child small { color: var(--ink-3); font-weight: var(--ui-weight); }
</style>
