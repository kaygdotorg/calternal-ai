<script lang="ts">
  // One message. The person's words sit in a tinted bubble on the right; the agent's are plain text on the card, with folded
  // steps, generated-UI cards and links to child threads between paragraphs. No inner card around a message.
  import { href } from "../../lib/route.svelte";
  import type { Component } from "svelte";
  import { Button, Icon } from "@calternal/ui";
  import Md from "../../lib/Md.svelte";
  import MessageActions from "./MessageActions.svelte";
  import ProviderMark from "./ProviderMark.svelte";
  import "./lead-mark.css";
  import Steps from "./Steps.svelte";
  import Thought from "./Thought.svelte";
  import type { Message, ProviderId } from "./model";
  import { client, store } from "./store.svelte";
  import { InlineEditor } from "@calternal/ui";
  // `provider` is the thread's current provider, the mark of a message whose marker does not say which wrote it.
  let { message, provider, providers, threadId = "", hit = false }: { threadId?: string; hit?: boolean; message: Message; provider: ProviderId; providers: { id: ProviderId; name: string; models: { id: string; name: string }[] }[] } = $props();
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
  // Editing your own message is another version of the thread from here on (docs/threads-ui.md, "Editing a sent message"): the bubble becomes an editor in place; Enter sends, Escape cancels.
  const canEdit = $derived(message.role === "user" && !!threadId && typeof client.editMessage === "function");
  let editing = $state(false);
  let draft = $state("");
  const own = $derived(message.blocks.flatMap((b) => (b.type === "text" ? [b.text] : [])).join("\n\n"));
  function edit() { draft = own; editing = true; }
  function send() { const t = draft.trim(); editing = false; if (t && t !== own.trim()) void client.editMessage!(threadId, message.id, t, crypto.randomUUID()); }
  const version = (v: number) => { void client.setVersion?.(threadId, message.id, v); };
  // Alt+[ and Alt+] step through the versions from anywhere in the message.
  function vkey(e: KeyboardEvent) {
    const v = message.version;
    if (!v || !e.altKey || (e.code !== "BracketLeft" && e.code !== "BracketRight")) return;
    const next = v.n + (e.code === "BracketRight" ? 1 : -1);
    if (next >= 1 && next <= v.of) { e.preventDefault(); version(next); }
  }
  // The mark sits on the first text line's centre for a text message and on the row's centre when the message starts with a block row (a thought, tool steps).
  const rowFirst = $derived(message.role === "agent" && (message.blocks[0]?.type === "thought" || message.blocks[0]?.type === "steps"));
  // What the message starts with decides where the mark goes (lead-mark.css): inline in the first paragraph, inside a first row, or on a line of its own before a list, code, picture or card.
  const lead = $derived(rowFirst ? "lead-row" : message.blocks[0]?.type === "text" && !/^\s*([-*+]\s|\d+[.)]\s|```|~~~|\||>|!\[)/.test(message.blocks[0].text) ? "lead-text" : "lead-own");
  // Message actions are created on first hover or focus (touch has no hover: they are there from the start).
  // An agent message far from the screen is not drawn yet: the row is a box of about its height until it is near the view (one observer per row, released once drawn), so a thread opens by drawing what is on screen, not the Markdown, code and steps of every row the list keeps around it. Without IntersectionObserver (tests) it is drawn at once.
  let drawn = $state(true); // (a message far from the screen was drawn lazily here once; it moved heights under a list pinned to its end, so every message is drawn when its row is)
  const pendingH = $derived(Math.max(48, Math.min(900, message.blocks.reduce((h, b) => h + (b.type === "text" ? 24 * Math.ceil(b.text.length / 85) + 12 : b.type === "steps" ? 40 : b.type === "card" ? 220 : 40), 16))));
  // The heavy parts (code blocks, pictures, the steps) are boxes of about their height for the first frame the message is drawn and are filled in a frame later: the first frame of a thread is its text.
  let rich = $state(true); // (the heavy parts were light boxes for a frame once; their height change moved a list pinned to its end, so they are drawn with the message)
  $effect(() => { if (!drawn || rich) return; let b = 0; const a = requestAnimationFrame(() => { b = requestAnimationFrame(() => (rich = true)); }); return () => { cancelAnimationFrame(a); cancelAnimationFrame(b); }; });
  function watch(node: HTMLElement) {
    if (drawn) return;
    const io = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) { drawn = true; io.disconnect(); } }, { rootMargin: "120px" });
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
  let armed = $state(typeof matchMedia === "function" && matchMedia("(hover: none)").matches);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="msg {message.role}" class:agent-row={message.role === "agent"} data-hit={hit ? "" : undefined} onpointerenter={() => (armed = true)} onfocusin={() => (armed = true)} onkeydown={vkey} use:watch>
  {#if message.role === "user"}
    <div class="col end">
      {#if editing}
        <div class="editor" data-ring-host><InlineEditor multiline autoselect label="Edit your message" bind:value={draft} onsubmit={send} oncancel={() => (editing = false)} /><span class="eb"><Button variant="quiet" text="Cancel" onclick={() => (editing = false)} /><Button variant="primary" text="Send" onclick={send} /></span></div>
      {:else}
        <div class="bubble">{#each message.blocks as b}{#if b.type === "text"}<Md text={clip(b.text)} />{:else if b.type === "file"}<span class="file"><Icon name={b.kind === "image" ? "image" : "file"} inline />{b.name}</span>{/if}{/each}</div>
      {/if}
      <MessageActions {message} {armed} onedit={canEdit && !editing ? edit : undefined} onversion={version} />
    </div>
  {:else if !drawn}
    <div class="pending" style:height="{pendingH}px" aria-hidden="true"></div>
  {:else}
    <div class="agent lead-host {lead}" id={message.id}>
      <div class="lead" class:row={rowFirst}><ProviderMark by={message.by} {provider} {providers} row={rowFirst} /></div>
      {#each message.blocks as b}
        {#if b.type === "text"}<Md text={clip(b.text)} light={!rich} />
        {:else if b.type === "thought"}<Thought text={b.text} ms={b.ms} />
        {:else if b.type === "file"}<span class="file"><Icon name={b.kind === "image" ? "image" : "file"} inline />{b.name}</span>
        {:else if b.type === "steps"}{#if rich}<Steps steps={b.steps} ms={b.ms} />{:else}<div class="ph" style:height="2.5rem" aria-hidden="true"></div>{/if}
        {:else if b.type === "card"}{#if GenUi && rich}<GenUi source={b.source} />{:else}<div class="ph" style:height="9rem" aria-hidden="true"></div>{/if}
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
  .ph { border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); }
  .editor { display: grid; gap: var(--s-3); width: min(100%, 36rem); padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); }
  .eb { display: flex; justify-content: flex-end; gap: var(--s-2); }
  .clipped { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-3); color: var(--ink-3); font-size: var(--text-label); }
  .msg[data-hit] { animation: hit 2s ease-out; }
  @keyframes hit { from { background: color-mix(in oklab, var(--accent) 16%, transparent); } to { background: transparent; } }
  .msg { width: 100%; max-width: var(--msg-w); margin: 0 auto; padding: var(--s-4) var(--msg-pad-r, var(--pad-region)) var(--s-4) var(--msg-pad-l, var(--pad-region)); display: flex; }
  .msg.agent-row { display: block; }
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
  /* One column that can shrink below its content (`minmax(0, 1fr)`): an `auto` track grew to the widest child (a code line, a long word) and pushed the steps and text past the right edge on a narrow screen. */
  .agent { display: grid; grid-template-columns: minmax(0, 1fr); gap: var(--s-5); min-width: 0; width: 100%; }
  .file { display: inline-flex; align-items: center; gap: var(--s-3); margin-bottom: var(--s-2); padding: var(--s-1) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .child { display: inline-flex; align-items: center; gap: var(--s-4); padding: var(--s-3) var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); justify-self: start; }
  .child small { color: var(--ink-3); font-weight: var(--ui-weight); }
</style>
