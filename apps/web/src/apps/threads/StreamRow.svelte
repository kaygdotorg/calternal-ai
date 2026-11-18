<script lang="ts">
  // The running turn's row at the end of the list, from the moment of the send: its provider's mark, the reasoning so far (folded, "Thinking" while it goes), the tool steps so far (folded the way they will be when
  // the turn is done), and the Working row, which pulses and says what is happening now (sending, waiting to start, starting the machine, thinking, running a tool, writing). When the person turned streaming on the
  // message text appears under it as it is written. When the turn ends the finished message replaces this row: the same blocks, collapsed to "Thought for 4 s" and "Worked for 12 s · 3 tools".
  import Md from "../../lib/Md.svelte";
  import ProviderMark from "./ProviderMark.svelte";
  import Steps from "./Steps.svelte";
  import Thought from "./Thought.svelte";
  import type { Turn } from "./conversation.svelte";
  import type { ProviderId } from "./model";
  let { turn, provider, providers }: { turn: Turn; provider: ProviderId; providers: { id: ProviderId; name: string; models: { id: string; name: string }[] }[] } = $props();
</script>

<div class="msg">
  <div class="lead"><ProviderMark {provider} {providers} /></div>
  <div class="agent">
    {#if turn.thought}<Thought text={turn.thought.text} ms={turn.thought.ms ?? 0} running={turn.thought.ms === undefined} />{/if}
    {#if turn.steps.length}<Steps steps={turn.steps} ms={0} running />{/if}
    <div class="working" role="status"><span class="word pulse">Working</span><span class="phase">{turn.phase}</span></div>
    {#if turn.text}<div class="selectable"><Md text={turn.text} caret /></div>{/if}
  </div>
</div>

<style>
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) var(--s-4) var(--msg-pad-l, var(--pad-region)); display: grid; grid-template-columns: var(--s-8) minmax(0, 1fr); column-gap: var(--s-4); }
  /* The mark sits in a line of the message's own font and line height, so its centre is the first line's x-height centre at every text size and zoom (ProviderMark.svelte). */
  .lead { font-size: var(--text-ui); line-height: 1.55; font-weight: var(--body-weight); }
  .agent { display: grid; gap: var(--s-5); min-width: 0; }
  .working { display: flex; align-items: baseline; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-ui); min-height: var(--s-9); }
  .word { font-weight: var(--strong-weight); color: var(--ink-2); }
  .phase { font-size: var(--text-label); }
</style>
