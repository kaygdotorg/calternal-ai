<script lang="ts">
  import { href } from "../../lib/route.svelte";
  // The open thread in the middle card: header, the virtualised message list, then the sticky request bar and the composer.
  // Messages are read a page at a time around what is on screen; the streaming message is one extra row at the end whose text is the
  // only thing that changes while it streams.
  import { Badge, IconButton } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import VirtualList from "../../lib/VirtualList.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { Conversation } from "./conversation.svelte";
  import Composer from "./Composer.svelte";
  import MessageRow from "./MessageRow.svelte";
  import type { ThreadState } from "./model";
  import RequestBar from "./RequestBar.svelte";
  import { client, store } from "./store.svelte";

  let { id }: { id: string } = $props();
  let conv = $state<Conversation>();
  let list = $state<ReturnType<typeof VirtualList>>();
  const summary = $derived(conv?.summary ?? store.get(id));
  const parent = $derived(summary?.parentId ? store.get(summary.parentId) : undefined);
  const reqs = $derived(store.requestsFor(id));
  const total = $derived((conv?.count ?? 0) + (conv?.stream ? 1 : 0));
  const TONE: Record<ThreadState, "neutral" | "good" | "warn" | "bad" | "info" | "accent"> = { idle: "neutral", running: "accent", waiting: "warn", failed: "bad", done: "good" };

  $effect(() => {
    const s = store.get(id);
    if (!s) return;
    const c = new Conversation(client, id, s);
    conv = c;
    return () => c.dispose();
  });
  // A message you send, or a first stream chunk, follows the end.
  let streamId = "";
  $effect(() => {
    const sid = conv?.stream?.id ?? "";
    if (sid && sid !== streamId) list?.scrollToBottom();
    streamId = sid;
  });
</script>

<div class="thread">
  <header>
    {#if shell.narrow || !shell.sidebarOpen}<IconButton icon="sidebar" label="Show threads" onclick={() => shell.toggleSidebar()} />{/if}
    <div class="title">
      {#if parent}<a class="crumb" href={href(`/threads/${parent.id}`)}>{parent.title}</a>{/if}
      <h1>{summary?.title ?? "Thread"}</h1>
    </div>
    {#if summary}<Badge text={summary.state} tone={TONE[summary.state]} dot />{/if}
    {#if !shell.narrow}<IconButton icon="sidebar-right" label={shell.rightOpen ? "Hide the side card" : "Show the side card"} pressed={shell.rightOpen} onclick={() => shell.toggleRight()} />{/if}
  </header>

  <div class="list">
    {#if conv}
      {#key conv.id}
        <VirtualList bind:this={list} count={total} estimate={150} label="Messages" onrange={(a, b) => conv?.ensure(a, b)}>
          {#snippet row(i)}
            {#if conv && i < conv.count}
              {@const m = conv.get(i)}
              {#if m}<MessageRow message={m} />{:else}<div class="skeleton" aria-hidden="true"></div>{/if}
            {:else if conv?.stream}
              <div class="msg"><Md text={conv.stream.text} caret /></div>
            {/if}
          {/snippet}
        </VirtualList>
      {/key}
    {/if}
  </div>

  {#if summary}
    <div class="bottom">
      <RequestBar requests={reqs} onanswer={(rid, a) => client.answer(rid, a)} />
      <Composer thread={summary} providers={store.providers} machines={store.machines} onpermission={(p) => client.setPermission(id, p)} onrunson={(r) => client.setRunsOn(id, r)} onsend={(t) => client.send(id, t, crypto.randomUUID())} onstop={() => client.stop(id)} onprovider={(p, m) => client.setProvider(id, p, m)} />
    </div>
  {/if}
</div>

<style>
  .thread { position: absolute; inset: 0; display: flex; flex-direction: column; }
  header { flex: none; display: flex; align-items: center; gap: var(--s-4); padding: var(--s-4) var(--s-5) var(--s-4) var(--pad-region); min-height: var(--bar-h); }
  .title { flex: 1; min-width: 0; display: grid; }
  h1 { font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .crumb { font-size: var(--text-label); color: var(--ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .list { position: relative; flex: 1; min-height: 0; }
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--pad-region); }
  .skeleton { height: 96px; margin: var(--s-4) var(--pad-region); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); }
  .bottom { flex: 0 1 auto; display: flex; flex-direction: column; gap: var(--s-4); min-height: 0; width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--pad-region) max(var(--pad-region), var(--safe-bottom), var(--bottom-inset, 0px)); }
</style>
