<script lang="ts">
  // The open thread in the middle card: header, the virtualised message list, then the sticky request bar and the composer.
  // Messages are read a page at a time around what is on screen; the streaming message is one extra row at the end whose text is the
  // only thing that changes while it streams.
  import { untrack } from "svelte";
  import { Button, cssPx, InlineEditor, Panel, Pill, toast } from "@calternal/ui";
  import { appearance } from "../../appearance/prefs.svelte";
  import { hazards } from "../../lib/hazard";
  import { VirtualList } from "@calternal/ui";
  import { href, navigate } from "../../lib/route.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { bridge } from "./compose.svelte";
  import { Conversation } from "./conversation.svelte";
  import SelectionActions from "./SelectionActions.svelte";
  import StreamRow from "./StreamRow.svelte";
  import StateMark from "./StateMark.svelte";
  import Composer from "./Composer.svelte";
  import MessageRow from "./MessageRow.svelte";
  import ThreadNotices from "./ThreadNotices.svelte";
  import RequestBar from "./RequestBar.svelte";
  import { client, store, startThread } from "./store.svelte";

  let { id }: { id: string } = $props();
  let conv = $state<Conversation>();
  let list = $state<ReturnType<typeof VirtualList>>();
  let listEl = $state<HTMLElement>();
  let bottomEl = $state<HTMLElement>();
  let bottomH = $state(160);
  let barH = $state(52);
  // The list's insets follow the floating bars: the header's height token and the composer's measured height (it grows as the field does).
  $effect(() => {
    if (!bottomEl) return;
    const ro = new ResizeObserver(() => (bottomH = Math.ceil(bottomEl!.getBoundingClientRect().height)));
    ro.observe(bottomEl);
    return () => ro.disconnect();
  });
  $effect(() => {
    const read = () => (barH = Math.round(cssPx("--bar-h", 52) + cssPx("--safe-top", 0)));
    void appearance.prefs.uiSize;
    read();
    matchMedia("(min-width: 760px)").addEventListener("change", read);
    return () => matchMedia("(min-width: 760px)").removeEventListener("change", read);
  });
  // Pictures scrolling under the floating pills raise those pills' fill (pill-contrast hazards).
  let headerEl = $state<HTMLElement>();
  $effect(() => {
    if (!listEl || !headerEl) return;
    const h = headerEl;
    return hazards(listEl, () => [...h.querySelectorAll<HTMLElement>(".pill, .title")]);
  });
  const summary = $derived(conv?.summary ?? store.get(id));
  const parent = $derived(summary?.parentId ? store.get(summary.parentId) : undefined);
  const reqs = $derived(store.requestsFor(id));
  const total = $derived((conv?.count ?? 0) + (conv?.stream ? 1 : 0));

  // One conversation per open thread. It depends on the thread being listed (a boolean), not on the list itself: the list is replaced whenever the server says something changed (every checkpoint of a
  // running turn), and a conversation rebuilt each time lost its cache and its place and left the open thread showing placeholders for messages the adapter had already read.
  const listed = $derived(!!store.get(id));
  $effect(() => {
    if (!listed) return;
    const s = untrack(() => store.get(id))!;
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
  let renaming = $state(false);
  let draftTitle = $state("");
  function startRename() { if (!summary) return; draftTitle = summary.title; renaming = true; }
  function commitTitle() {
    if (!renaming || !summary) return;
    renaming = false;
    const t = draftTitle.trim();
    if (t && t !== summary.title) void client.rename(summary.id, t).catch((e: Error) => toast(`Not renamed: ${e.message}`));
  }
</script>

<div class="thread">
  <header bind:this={headerEl}>
    {#if shell.narrow}<Pill icon="sidebar" label="Show threads" onclick={() => shell.toggleSidebar()} />{/if}
    <Panel class="title" radius="pill">
      <div class="tt">
        {#if parent}<a class="crumb" href={href(`/threads/${parent.id}`)}>{parent.title}</a>{/if}
        {#if renaming}
          <InlineEditor class="title-edit" label="Rename thread" bind:value={draftTitle} autoselect onsubmit={commitTitle} oncancel={() => (renaming = false)} onblur={commitTitle} />
        {:else}
          <h1 ondblclick={startRename}>{summary?.title ?? "Thread"}</h1>
        {/if}
      </div>
      {#if summary && summary.state !== "idle" && summary.state !== "done"}<StateMark state={summary.state} form="word" />{/if}
    </Panel>
    <span class="grow"></span>
  </header>

  <div class="list" bind:this={listEl}>
    {#if conv && conv.count === 0 && !conv.stream}
      <div class="blank">
        <h2>Nothing sent yet</h2>
        <p>Say what you want done in the box below. The agent shows its steps while it works, and asks you before it does anything you have not allowed.</p>
        <Button variant="primary" text="Write a message" onclick={() => document.getElementById("composer-field")?.focus()} />
      </div>
    {:else if conv}
      {#key conv.id}
        <VirtualList bind:this={list} count={total} estimate={150} label="Messages" topInset={barH} bottomInset={bottomH} onrange={(a, b) => conv?.ensure(a, b)}>
          {#snippet row(i)}
            {#if conv && i < conv.count}
              {@const m = conv.get(i)}
              {#if m}<MessageRow message={m} />{:else}<div class="skeleton" aria-hidden="true"></div>{/if}
            {:else if conv?.stream}
              <StreamRow text={conv.stream.text} steps={conv.stream.steps} />
            {/if}
          {/snippet}
        </VirtualList>
      {/key}
    {/if}
  </div>

  {#if summary}
    <div class="bottom" bind:this={bottomEl}>
      <div class="inner">
        <ThreadNotices thread={summary} onnew={() => void startThread().then((t) => navigate(`/threads/${t.id}`), (e: Error) => toast(`Could not start a thread: ${e.message}`))} onresume={() => client.resume(id).catch((e: Error) => toast(`Could not resume: ${e.message}`))} oncheck={() => client.check(id).then(() => toast("Checked"), (e: Error) => toast(`Could not check: ${e.message}`))} />
        <RequestBar requests={reqs} onanswer={(rid, a) => client.answer(rid, a).catch((e: Error) => toast(`Not sent: ${e.message}`))} />
        <Composer thread={summary} providers={store.providers} machines={store.machines} onpermission={(p) => client.setPermission(id, p)} onrunson={(r) => client.setRunsOn(id, r)} onsend={(t) => client.send(id, t, crypto.randomUUID()).catch((e: Error) => { toast(`Not sent: ${e.message}`); throw e; })} onstop={() => client.stop(id).catch((e: Error) => toast(`Could not stop: ${e.message}`))} onprovider={(p, m) => client.setProvider(id, p, m)} />
      </div>
    </div>
  {/if}
  <SelectionActions root={listEl} oncomment={(t) => bridge.comment(t)} onask={(t) => { bridge.ask(t); shell.showPanel("chat"); }} />
</div>

<style>
  .thread { position: absolute; inset: 0; }
  /* The header and the composer float over the list; the list fades out under them (VirtualList), so there is no opaque band at the top of the card. */
  header { position: absolute; z-index: 3; top: var(--safe-top); left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--pad-region) 0 calc(var(--pad-region) + var(--edge-l, 0px)); pointer-events: none; } /* --edge-l: room for the shell's button that shows the sidebar again (shell/Shell.svelte) */
  header > :global(*) { pointer-events: auto; }
  .grow { flex: 1; pointer-events: none; }
  .blank { position: absolute; inset: 0; display: grid; align-content: center; justify-items: start; gap: var(--s-4); max-width: 30rem; margin: 0 auto; padding: 0 var(--pad-region); }
  .blank h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .blank p { color: var(--ink-2); font-size: var(--text-ui); }
  :global(.title) { display: flex; align-items: center; gap: var(--s-3); min-width: 0; max-width: 100%; height: var(--pill-h); padding: 0 var(--s-5); }
  .tt { min-width: 0; display: grid; }
  :global(.title-edit) { font-size: var(--text-ui); font-weight: var(--strong-weight); width: 100%; min-width: 12ch; }
  h1 { font-size: var(--text-ui); font-weight: var(--strong-weight); line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .crumb { font-size: var(--text-section); color: var(--ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; line-height: 1; }
  .list { position: absolute; inset: 0; }
  .skeleton { height: 96px; margin: var(--s-4) var(--pad-region); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); }
  .bottom { position: absolute; z-index: 3; left: 0; right: 0; bottom: 0; display: flex; justify-content: center; pointer-events: none; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) max(var(--pad-region), var(--safe-bottom), var(--bottom-inset, 0px)) var(--msg-pad-l, var(--pad-region)); }
  .inner { pointer-events: auto; display: flex; flex-direction: column; gap: var(--s-4); width: 100%; max-width: var(--msg-inner, calc(var(--msg-w) - 2 * var(--pad-region))); min-width: 0; }
</style>
