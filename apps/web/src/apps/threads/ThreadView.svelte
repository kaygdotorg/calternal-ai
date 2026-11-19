<script lang="ts">
  // The open thread in the middle card: header, the virtualised message list, then the sticky request bar and the composer.
  // Messages are read a page at a time around what is on screen; the streaming message is one extra row at the end whose text is the
  // only thing that changes while it streams.
  import { getContext, untrack } from "svelte";
  import { Button, chromeHeader, cssPx, InlineEditor, Menu, Panel, Pill, toast } from "@calternal/ui";
  import { runThreadItem, threadItems } from "./thread-menu";
  import { appearance } from "../../appearance/prefs.svelte";
  import { hazards } from "../../lib/hazard";
  import { VirtualList } from "@calternal/ui";
  import { href, navigate, route } from "../../lib/route.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { bridge } from "./compose.svelte";
  import { Conversation } from "./conversation.svelte";
  import SelectionActions from "./SelectionActions.svelte";
  import StreamRow from "./StreamRow.svelte";
  import Composer from "./Composer.svelte";
  import MessageRow from "./MessageRow.svelte";
  import ThreadNotices from "./ThreadNotices.svelte";
  import RequestBar from "./RequestBar.svelte";
  import SignIn from "./SignIn.svelte";
  import type { SignInRequest } from "./model";
  import { client, store, startThread } from "./store.svelte";

  let { id }: { id: string } = $props();
  let conv = $state<Conversation>();
  // The header's menu, the contrast hazards and the selection actions are made after the first frame of a thread; everything that sets the height under the messages (notices, request bar, composer) is there from the first, so the list is never moved by it.
  let late = $state(false);
  $effect(() => { void id; late = false; let r2 = 0; const r1 = requestAnimationFrame(() => { r2 = requestAnimationFrame(() => (late = true)); }); return () => { cancelAnimationFrame(r1); cancelAnimationFrame(r2); }; });
  // The thread's open is stamped for the native shell and the timing suite: `calternal:thread:open` is a performance measure from the view being made to the first screenful being drawn (read it with `performance.getEntriesByName`), the same moment is the `calternal:thread-rendered` event on `window` and `data-thread-rendered` on the root (the thread's id).
  $effect(() => {
    const t = id;
    performance.mark("calternal:thread:start", { detail: { id: t } });
    delete document.documentElement.dataset.threadRendered;
    return () => { delete document.documentElement.dataset.threadRendered; };
  });
  $effect(() => {
    const c = conv; void c?.rev;
    if (!c || c.count === 0 || c.id !== id || document.documentElement.dataset.threadRendered === id) return;
    if (!c.get(c.count - 1)) return; // the screenful at the end has not been read yet
    const t = id;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (id !== t || document.documentElement.dataset.threadRendered === t) return;
      document.documentElement.dataset.threadRendered = t;
      try { performance.measure("calternal:thread:open", { start: "calternal:thread:start", detail: { id: t } }); } catch { /* no start mark */ }
      window.dispatchEvent(new CustomEvent("calternal:thread-rendered", { detail: { id: t, ms: performance.getEntriesByName("calternal:thread:open").at(-1)?.duration ?? null } }));
    }));
  });
  let list = $state<ReturnType<typeof VirtualList>>();
  let listEl = $state<HTMLElement>();
  let bottomEl = $state<HTMLElement>();
  /** An element's height in its own layout pixels: a canvas window may sit under a scaling transform, which `getBoundingClientRect` would apply (the list's inset would then be short by the scale). */
  const layoutH = (el: HTMLElement) => { const r = el.getBoundingClientRect(); const k = el.offsetWidth ? r.width / el.offsetWidth : 1; return Math.ceil(r.height / (k || 1) - 0.01); };
  let bottomH = $state(160);
  let barH = $state(52);
  // The list's bottom inset follows the height of what floats under it (the composer, with its notices and the request bar), as measured: that is where the last message ends. A list that was at the bottom
  // stays at the bottom when that height changes (the composer growing a line, a bar appearing, the composer expanding): the content moves up with it, as in ChatGPT's and Claude's apps (the owner's
  // recordings), not a gap left behind. When the composer expands or collapses (`cmp-fold`, composer's morph) the content also slides by the difference over the same time and curve, as a transform of the
  // list's content: the scroll position changes once, the transform brings the content from where it was to where it is, and nothing is laid out per frame.
  $effect(() => {
    if (!bottomEl) return;
    const el = bottomEl;
    const feed = () => listEl?.querySelector<HTMLElement>("[role=feed]") ?? null;
    // Plays `el` from `d` px away to its place. A slide that is still running is taken over from where it is (its offset is added to `d` and it is cancelled), so reversing mid-morph is continuous.
    const slide = (el: Element, d: number, ms: number, easing: string) => {
      let cur = 0;
      for (const a of el.getAnimations()) { const t = getComputedStyle(el).transform; if (t && t !== "none") cur = new DOMMatrixReadOnly(t).m42; a.cancel(); }
      el.animate([{ transform: `translateY(${cur + d}px)` }, { transform: "none" }], { duration: ms, easing });
    };
    const settle = (h: number, morph?: { ms: number; easing: string }) => {
      const f = feed(); const d = h - bottomH;
      if (d === 0) return;
      const pinned = !!f && f.scrollHeight - f.scrollTop - f.clientHeight < 24;
      bottomH = h;
      if (!f || !pinned) return;
      // After the list has taken the new inset (its state flush is a microtask scheduled by the line above).
      queueMicrotask(() => {
        f.scrollTop = f.scrollHeight;
        const sz = f.querySelector(".sizer"); if (morph && sz && Math.abs(d) > 1) slide(sz, d, morph.ms, morph.easing);
      });
    };
    const ro = new ResizeObserver(() => settle(layoutH(el)));
    const onfold = (e: Event) => {
      const d = (e as CustomEvent<{ animate: boolean; ms: number; easing: string }>).detail;
      const h = layoutH(el);
      const moved = h - bottomH;
      // What sits above the composer (the request bar, the notices) moves by the same amount in the one layout; it is played from where it was, so a press that began on it still lands on it.
      if (d.animate && Math.abs(moved) > 1) for (const n of el.querySelectorAll<HTMLElement>(".inner > :not(.composer)")) slide(n, moved, d.ms, d.easing);
      settle(h, d.animate ? d : undefined);
    };
    ro.observe(el);
    el.addEventListener("cmp-fold", onfold);
    return () => { ro.disconnect(); el.removeEventListener("cmp-fold", onfold); };
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
    if (!listEl || !headerEl || !late) return; // after the first frame: the pills' contrast follows the messages scrolling under them, nothing to follow before it
    const h = headerEl;
    return hazards(listEl, () => [...h.querySelectorAll<HTMLElement>(".pill, .title"), ...document.querySelectorAll<HTMLElement>(".hdr .hp")]);
  });
  const summary = $derived(conv?.summary ?? store.get(id));
  const reqs = $derived(store.requestsFor(id));
  /** Sign-ins for this thread, open or just ended: items at the end of the conversation (not part of it: the thread file carries one line). */
  const signIns = $derived<{ req: SignInRequest; state: "pending" | "done" | "failed" }[]>([
    ...reqs.flatMap((r) => (r.kind === "signin" ? [{ req: r, state: "pending" as const }] : [])),
    ...store.signedIn.filter((f) => f.req.threadId === id),
  ]);
  const total = $derived((conv?.count ?? 0) + (conv?.stream ? 1 : 0) + signIns.length);

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
  // `?m=<message id>` on the thread's address (a search result, a link to a message) scrolls to that message at once, with no animation, and marks it for a moment; the id is looked up, the messages between are not loaded.
  let hit = $state("");
  $effect(() => {
    const m = route.query.get("m");
    const ready = !!list && !!conv;
    if (!m || !ready) return;
    void untrack(async () => {
      const i = await client.messageIndex?.(id, m);
      if (i === undefined || !list) return;
      list.scrollToIndex(i, "centre");
      hit = m;
      setTimeout(() => { if (hit === m) hit = ""; }, 2000);
    });
  });
  // A message you send, or a first stream chunk, follows the end.
  let streamId = "";
  $effect(() => {
    const sid = conv?.stream?.id ?? "";
    if (sid && sid !== streamId && list?.nearEnd()) list?.scrollToBottom(); // a turn that starts follows the end only for someone reading the end: never yank a person who scrolled up to read
    streamId = sid;
  });
  // In a canvas window (canvas/WindowFrame.svelte) the window supplies the top right pill (full screen, share, the menu) and the sidebar's way back is the shell's: this view keeps only its title pill.
  const windowed = getContext<boolean | undefined>("canvas-window") === true;
  // The thread's own menu, at the card's top right: the very menu the sidebar row opens (thread-menu.ts, one definition).
  let menuOpen = $state(false);
  let moreEl = $state<HTMLButtonElement>();
  let renaming = $state(false);
  // The header line the shell draws beside the sidebar button is this thread's title while the view is open (a canvas window draws its own title pill).
  $effect(() => { if (windowed) return; chromeHeader.pinned = summary?.title ?? ""; return () => { chromeHeader.pinned = ""; }; });
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
  <header bind:this={headerEl} class:windowed data-chrome-header>
    {#if shell.narrow && !windowed}<Pill icon="sidebar" label="Show threads" onclick={() => shell.toggleSidebar()} />{/if}
    <!-- The thread's title is the page's one heading (assistive technology reads this) and, drawn, the shell's shared header line (HeaderPill, pinned below) beside the sidebar button; in a canvas window the window's own title pill. A pill is drawn here only while renaming. -->
    <h1 class="visually-hidden">{summary?.title ?? "Thread"}</h1>
    {#if renaming}
      <Panel class="title" radius="pill"><InlineEditor class="title-edit" label="Rename thread" bind:value={draftTitle} autoselect onsubmit={commitTitle} oncancel={() => (renaming = false)} onblur={commitTitle} /></Panel>
    {/if}
    <span class="grow"></span>
    {#if summary && !windowed && late}
      <Pill icon="more" label="Thread actions" aria-haspopup="menu" aria-expanded={menuOpen} bind:el={moreEl} onclick={() => (menuOpen = !menuOpen)} />
      <Menu open={menuOpen} items={[...threadItems(summary), { id: "panel", label: shell.rightOpen ? "Hide side panel" : "Show side panel", icon: "sidebar-right" as const }]} label="Thread actions" trigger={moreEl} morph onclose={() => (menuOpen = false)} onpick={(item) => { menuOpen = false; if (item === "panel") shell.toggleRight(); else if (summary) runThreadItem(item, summary, startRename); }} />
    {/if}
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
        <VirtualList bind:this={list} count={total} estimate={150} overscan={300} label="Messages" topInset={barH} bottomInset={bottomH} onrange={(a, b) => conv?.ensure(a, b)}>
          {#snippet row(i)}
            {#if conv && i < conv.count}
              {@const m = conv.get(i)}
              {#if m}<MessageRow message={m} threadId={id} hit={hit === m.id} provider={summary?.provider ?? "claude"} providers={store.providers} />{:else}<div class="skeleton" aria-hidden="true"></div>{/if}
            {:else if conv?.stream && i === conv.count}
              <StreamRow turn={conv.stream} provider={summary?.provider ?? "claude"} providers={store.providers} />
            {:else if conv}
              {@const s = signIns[i - conv.count - (conv.stream ? 1 : 0)]}
              {#if s}<div class="signin">{#key s.req.id}<SignIn request={s.req} result={s.state} onanswer={(code) => client.answer(s.req.id, { kind: "signin", code }).catch((e: Error) => toast(`Not sent: ${e.message}`))} onpoll={s.state === "pending" ? async () => { const st = (await client.signInStatus?.(s.req.id)) ?? "gone"; if (st === "done" || st === "failed") void store.refresh(); return st; } : undefined} />{/key}</div>{/if}
            {/if}
          {/snippet}
        </VirtualList>
      {/key}
    {/if}
  </div>

  {#if summary}
    <div class="bottom" bind:this={bottomEl}>
      <div class="inner" >
        <ThreadNotices thread={summary} onnew={() => void startThread().then((t) => navigate(`/threads/${t.id}`), (e: Error) => toast(`Could not start a thread: ${e.message}`))} onresume={() => client.resume(id).catch((e: Error) => toast(`Could not resume: ${e.message}`))} oncheck={() => client.check(id).then(() => toast("Checked"), (e: Error) => toast(`Could not check: ${e.message}`))} />
        <RequestBar requests={reqs} onanswer={(rid, a) => client.answer(rid, a).catch((e: Error) => toast(`Not sent: ${e.message}`))} />
        <Composer thread={summary} providers={store.providers} machines={store.machines} onpermission={(p) => client.setPermission(id, p)} onrunson={(r) => client.setRunsOn(id, r)} onreasoning={(l) => client.setReasoning(id, l)} ontype={() => void client.signInTyping?.(id)} onattach={(f, progress) => client.upload(id, f, progress)} onsend={(t, files) => { const undo = conv?.begin(); return client.send(id, t, crypto.randomUUID(), files).catch((e: Error) => { undo?.(); toast(`Not sent: ${e.message}`); throw e; }); }} onstop={() => client.stop(id).catch((e: Error) => toast(`Could not stop: ${e.message}`))} onprovider={(p, m) => client.setProvider(id, p, m)} />
      </div>
    </div>
  {/if}
  {#if late}<SelectionActions root={listEl} oncomment={(t) => bridge.comment(t)} onask={(t) => bridge.ask(t)} />{/if}
</div>

<style>
  .thread { position: absolute; inset: 0; }
  /* The header and the composer float over the list; the list fades out under them (VirtualList), so there is no opaque band at the top of the card. */
  header { position: absolute; z-index: 3; top: var(--safe-top); left: 0; right: 0; height: var(--bar-h); display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--pad-region) 0 calc(var(--pad-region) + var(--edge-l, 0px)); pointer-events: none; } /* --edge-l: room for the shell's button that shows the sidebar again (shell/Shell.svelte) */
  header > :global(*) { pointer-events: auto; }
  /* In a canvas window the right pill (three buttons) floats over the header: the title pill leaves room for it and shortens with an ellipsis. */
  header.windowed { padding-right: calc(var(--pill-h) * 3 + var(--s-8)); }
  .grow { flex: 1; pointer-events: none; }
  .blank { position: absolute; inset: 0; display: grid; align-content: center; justify-items: start; gap: var(--s-4); max-width: 30rem; margin: 0 auto; padding: 0 var(--pad-region); }
  .blank h2 { font-size: var(--text-heading); font-weight: var(--strong-weight); }
  .blank p { color: var(--ink-2); font-size: var(--text-ui); }
  :global(.title) { display: flex; align-items: center; min-width: 0; max-width: 100%; height: var(--pill-h); padding: 0 var(--s-5); }
  :global(.title-edit) { font-size: var(--text-ui); font-weight: var(--strong-weight); width: 100%; min-width: 12ch; }
  .list { position: absolute; inset: 0; }
  .signin { margin: var(--s-4) var(--pad-region); max-width: var(--msg-w, 100%); }
  .skeleton { height: 96px; margin: var(--s-4) var(--pad-region); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); }
  .bottom { position: absolute; z-index: 3; left: 0; right: 0; bottom: 0; display: flex; justify-content: center; pointer-events: none; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) max(var(--s-4), calc(var(--safe-bottom) - var(--kb, 0px))) var(--msg-pad-l, var(--pad-region)); }
  /* The composer floats at the bottom of the card above the safe area (the home indicator; none while the keyboard is up: `--kb`), over the scrolling messages, with nothing behind it but its own material. On a phone its box is the full content width. */
  :global(.shell.narrow) .inner { max-width: 100%; }
  .inner { pointer-events: auto; display: flex; flex-direction: column; gap: var(--s-4); width: 100%; max-width: var(--msg-inner, calc(var(--msg-w) - 2 * var(--pad-region))); min-width: 0; }
</style>
