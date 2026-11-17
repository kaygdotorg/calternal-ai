<script lang="ts">
  // The running turn's row at the end of the list: a working state with the tool steps so far (folded as they will be when done), and, only when the person
  // turned streaming on, the message text as it is written. Otherwise the finished message replaces this row at once.
  import Md from "../../lib/Md.svelte";
  import Steps from "./Steps.svelte";
  import type { Step } from "./model";
  let { text, steps }: { text: string; steps: Step[] } = $props();
</script>

<div class="msg">
  <div class="agent">
    <Steps steps={steps.length ? steps : [{ id: "w", kind: "read", title: "Starting", status: "running", ms: 0 }]} ms={0} running />
    {#if text}<div class="selectable"><Md {text} caret /></div>{/if}
  </div>
</div>

<style>
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) var(--s-4) var(--msg-pad-l, var(--pad-region)); }
  .agent { display: grid; gap: var(--s-5); }
</style>
