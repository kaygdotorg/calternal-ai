<script lang="ts">
  // The Chat conversation view, as built for the Chat interface and drawn here from fixtures only (nothing is sent). It shows: a date chip; entries grouped by author within five minutes (one name above a run; your own to the end in the accent tint, others to the
  // start); an agent entry with its provider's mark and "asked by"; a quote with a straight square-ended bar; "3 unread messages" where reading resumes (the view opens there); inline code and a code block; an attachment chip; the edited mark and the sending
  // state (dimmed, "Sending…"); the jump buttons (to the latest with the unread count, and to the next mention); and the composer (a text field, Attach, "Ask an agent" with the installed providers, Send). Scroll anchoring, read marking and the host's live
  // exchange are the real renderer's; here the unread line, the jump counters and the buttons are drawn and work on the fixture.
  import { onMount, tick } from "svelte";
  import { Icon, Menu, IconButton, type MenuItem } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import Mark from "../../shell/Mark.svelte";
  import type { Item } from "../../data/collections";
  let { item }: { item?: Item } = $props();
  type Prov = "claude" | "codex";
  interface Msg { id: string; kind: "msg"; who: string; own?: boolean; agent?: Prov; asked?: string; t: string; text?: string; quote?: { who: string; text: string }; code?: string; file?: string; edited?: boolean; sending?: boolean; mention?: boolean }
  type Row = { id: string; kind: "date"; text: string } | { id: string; kind: "unread"; n: number } | Msg;
  /** The agent that answers in this thread: the thread's own provider (a Claude thread is answered by Claude). */
  const A: Prov = $derived(item?.provider === "claude" ? "claude" : "codex"), AN = $derived(A === "claude" ? "Claude" : "Codex");
  const rows: Row[] = $derived([
    { id: "d1", kind: "date", text: "Thursday 19 November" },
    { id: "m1", kind: "msg", who: "Ivo Novak", t: "09:02", text: "Morning! Did the dependency audit finish overnight?" },
    { id: "m2", kind: "msg", who: "Ivo Novak", t: "09:03", text: "I would like to ship the update today if it is clean." },
    { id: "m3", kind: "msg", own: true, who: "You", t: "09:10", text: "It finished at 6. Three advisories, all in dev dependencies, none reachable from the app." },
    { id: "m4", kind: "msg", who: "Hana Sato", t: "09:12", text: "Can someone summarise them for the release notes?" },
    { id: "m5", kind: "msg", own: true, who: "You", t: "09:14", text: `On it: I will ask ${AN} to write them up and check \`npm audit\` again.`, quote: { who: "Hana Sato", text: "Can someone summarise them for the release notes?" } },
    { id: "u", kind: "unread", n: 3 },
    { id: "m6", kind: "msg", agent: A, who: AN, asked: "Ada", t: "09:16", text: "All three are in `eslint-plugin-x` and its test helpers. The fixed version is 4.1.2:", code: "npm install --save-dev eslint-plugin-x@4.1.2\nnpm audit --omit=dev   # found 0 vulnerabilities", file: "audit-2026-11-19.txt · 18 KB" },
    { id: "m7", kind: "msg", who: "Hana Sato", t: "09:20", text: "Great, thank you @Ada. That goes straight into the notes.", mention: true, edited: true },
    { id: "m8", kind: "msg", own: true, who: "You", t: "09:24", text: "Merging the fix now.", sending: true },
  ]);
  const title = $derived(item?.title ?? "Dependency audit");
  const group = (i: number) => { const r = rows[i] as Msg, p = rows[i - 1] as Msg | undefined; const first = !(p && p.kind === "msg" && p.who === r.who && !!p.own === !!r.own); const n = rows[i + 1] as Msg | undefined; const last = !(n && n.kind === "msg" && n.who === r.who && !!n.own === !!r.own); return { first, last }; };
  let scroller = $state<HTMLElement>(), atEnd = $state(false), draft = $state(""), askOpen = $state(false), askEl = $state<HTMLElement | null>(null), picked = $state<Prov | null>(null), mentionAt = $state(0);
  const unread = 3, mentions = $derived(rows.filter((r) => r.kind === "msg" && (r as Msg).mention));
  const near = () => { if (scroller) atEnd = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40; };
  onMount(async () => { await tick(); const u = scroller?.querySelector<HTMLElement>("[data-unread]"); if (u && scroller) scroller.scrollTop = Math.max(0, u.offsetTop - 96); near(); });
  const toEnd = () => scroller?.scrollTo({ top: scroller.scrollHeight, behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  const toMention = () => { const m = mentions[mentionAt % mentions.length]; mentionAt++; scroller?.querySelector<HTMLElement>(`[data-msg="${m!.id}"]`)?.scrollIntoView({ block: "center", behavior: "smooth" }); };
  /** Marks a code block that scrolls sideways with a fade on the side where there is more. */
  const cue = (n: HTMLElement) => { const f = () => { n.classList.toggle("mr", n.scrollLeft + n.clientWidth < n.scrollWidth - 2); n.classList.toggle("ml", n.scrollLeft > 2); }; f(); n.addEventListener("scroll", f); const ro = new ResizeObserver(f); ro.observe(n); return { destroy: () => { n.removeEventListener("scroll", f); ro.disconnect(); } }; };
  const providers: MenuItem[] = [{ id: "claude", label: "Claude" }, { id: "codex", label: "Codex" }];
  const send = () => { draft = ""; picked = null; };
</script>

<section class="cv" data-chat-page aria-label={title}>
  <div class="trw">
  <div class="tr" bind:this={scroller} onscroll={near} role="log" aria-label="Messages">
    {#each rows as r, i (r.id)}
      {#if r.kind === "date"}<div class="mk"><time>{r.text}</time></div>
      {:else if r.kind === "unread"}<div class="un" data-unread role="separator"><span>{r.n} unread messages</span></div>
      {:else}
        {@const g = group(i)}
        <article class="e" class:own={r.own} class:first={g.first} class:last={g.last} class:queued={r.sending} data-msg={r.id} aria-label={`${r.who}, ${r.t}${r.asked ? `, asked by ${r.asked}` : ""}`}>
          {#if g.first && !r.own}<header class="eh">{#if r.agent}<Mark provider={r.agent} size="18px" />{:else}<Av name={r.who} size="18px" />{/if}<span class="nm">{r.who}</span>{#if r.asked}<span class="ask">asked by {r.asked}</span>{/if}</header>{/if}
          <div class="bub">
            {#if r.quote}<div class="qt" data-quote><span class="qa">{r.quote.who}</span><span class="qx">{r.quote.text}</span></div>{/if}
            {#if r.text}<p class="t">{#each r.text.split(/(`[^`]+`|@\w+)/) as part, k (k)}{#if part.startsWith("`")}<code class="ic">{part.slice(1, -1)}</code>{:else if part.startsWith("@")}<span class="mn">{part}</span>{:else}{part}{/if}{/each}</p>{/if}
            {#if r.code}<pre class="c" use:cue><code>{r.code}</code></pre>{/if}
            {#if r.file}<div class="att" role="group" aria-label={r.file}><Icon name="paperclip" inline /><span>{r.file}</span></div>{/if}
            <span class="meta">{#if r.edited}<span class="ed">edited</span>{/if}{#if r.sending}<span class="sd">Sending…</span>{/if}<time>{r.t}</time></span>
          </div>
        </article>
      {/if}
    {/each}
  </div>
  <div class="jumps" aria-label="Jump">
    {#if mentions.length}<button type="button" class="jb" aria-label="Next mention" onclick={toMention}><span class="at">@</span><span class="bd">{mentions.length}</span></button>{/if}
    {#if !atEnd}<button type="button" class="jb" aria-label={`Jump to the latest, ${unread} unread`} onclick={toEnd}><Icon name="down" inline /><span class="bd">{unread}</span></button>{/if}
  </div>
  </div>
  <form class="cp" onsubmit={(e) => { e.preventDefault(); send(); }} aria-label="Composer">
    {#if picked}<div class="who"><Mark provider={picked} size="18px" /><span>Ask {picked === "claude" ? "Claude" : "Codex"}</span><button type="button" class="x" aria-label="Do not ask an agent" onclick={() => (picked = null)}><Icon name="close" inline /></button></div>{/if}
    <div class="row">
      <IconButton icon="paperclip" label="Attach" onclick={() => {}} />
      <textarea rows="1" bind:value={draft} placeholder="Message" aria-label="Message"></textarea>
      <button type="button" class="ask-b" bind:this={askEl} aria-haspopup="menu" onclick={() => (askOpen = true)}><Icon name="spark" inline /><span class="lbl">Ask an agent</span></button>
      <button type="submit" class="send" disabled={!draft.trim()} aria-label="Send"><Icon name="send" inline /></button>
    </div>
    <Menu open={askOpen} items={providers} label="Ask an agent" onpick={(id) => { picked = id as Prov; askOpen = false; }} onclose={() => (askOpen = false)} trigger={askEl} />
  </form>
</section>

<style>
  .cv { position: relative; display: flex; flex-direction: column; flex: 1; min-height: 0; margin: 0 calc(var(--s-6) * -1); }
  .tr { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; padding: 0 var(--s-6) calc(2 * 36px + 2 * var(--s-3) + 12px); /* room for both jump buttons (36 each), their gap and offset, and the count badge that overhangs the top */ scroll-padding-block: var(--s-6); }
  .mk { align-self: center; margin-block: var(--s-4) var(--s-2); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; } .mk time { display: inline-block; padding: var(--s-2) var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); }
  .un { display: flex; align-items: center; gap: var(--s-4); margin-block: var(--s-5) var(--s-2); color: var(--accent); font-size: var(--text-label); font-weight: var(--strong-weight); line-height: 1.25; } .un::before, .un::after { content: ""; flex: 1; height: 1px; background: color-mix(in oklab, var(--accent) 40%, transparent); }
  .e { display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-1); margin-top: var(--s-1); max-width: min(82%, 38rem); position: relative; } .e.first { margin-top: var(--s-5); } .e.own { align-self: flex-end; align-items: flex-end; }
  .eh { display: flex; align-items: center; gap: var(--s-3); padding-inline: var(--s-4); align-self: flex-start; background: rgb(var(--tint) / 0.7); border-radius: var(--r-pill); padding-block: 1px; margin-inline-start: var(--s-2); font-size: var(--text-label); line-height: 1.25; } .nm { font-weight: var(--strong-weight); color: var(--ink); } .ask { color: var(--ink-2); }
  .bub { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-3); padding: var(--s-4) var(--s-5); padding-bottom: calc(var(--s-6) + var(--s-2)); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); min-width: 7rem; max-width: 100%; }
  .e:not(.own):not(.first) .bub { border-top-left-radius: var(--r-select); } .e:not(.own):not(.last) .bub { border-bottom-left-radius: var(--r-select); } .e:not(.own).last .bub { border-bottom-left-radius: var(--r-select); }
  .e.own .bub { background: var(--accent-soft); } .e.own:not(.first) .bub { border-top-right-radius: var(--r-select); } .e.own:not(.last) .bub { border-bottom-right-radius: var(--r-select); } .e.own.last .bub { border-bottom-right-radius: var(--r-select); }
  .e.queued .bub { background: rgb(var(--ink-rgb) / 0.04); outline: 1px dashed rgb(var(--ink-rgb) / 0.3); outline-offset: -1px; } /* a message still being sent is drawn outlined, not faded: its text keeps its contrast */
  .t { margin: 0; font-family: var(--font-chat); font-size: var(--text-ui); line-height: 1.45; white-space: pre-wrap; overflow-wrap: anywhere; } .mn { color: var(--ink); font-weight: var(--strong-weight); background: color-mix(in oklab, var(--accent-base) 16%, transparent); border-radius: var(--r-select); padding: 0 var(--s-2); }
  .ic { font-family: var(--font-mono); font-size: var(--text-label); padding: 0 var(--s-1); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); }
  .c { margin: 0; max-width: 100%; box-sizing: border-box; padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.08); overflow: auto; font-size: var(--text-label); line-height: 1.45; } .c code { font-family: var(--font-mono); }
  .att { display: inline-flex; align-items: center; gap: var(--s-3); padding: var(--s-2) var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); width: fit-content; max-width: 100%; font-size: var(--text-label); line-height: 1.25; } .att span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* the quote's leading bar is a straight, square-ended strip, never a rounded border */
  .qt { position: relative; display: grid; gap: 2px; align-self: stretch; padding: var(--s-2) var(--s-4) var(--s-2) var(--s-5); border-radius: 0 var(--r-select) var(--r-select) 0; background: color-mix(in oklab, var(--accent) 12%, transparent); } .qt::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; width: 3px; background: var(--accent); }
  .qa { font-weight: var(--strong-weight); font-size: var(--text-label); color: var(--ink); line-height: 1.25; } .qx { color: var(--ink-2); font-size: var(--text-label); line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .meta { position: absolute; right: var(--s-5); bottom: var(--s-2); display: inline-flex; align-items: baseline; gap: var(--s-3); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; font-variant-numeric: tabular-nums; white-space: nowrap; } .ed, .sd { font-style: italic; }
  .trw { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .jumps { position: absolute; right: var(--s-6); bottom: var(--s-3); display: grid; gap: var(--s-3); justify-items: end; }
  .jb { position: relative; display: grid; place-items: center; width: 36px; height: 36px; border-radius: 50%; background: var(--paper, #fff); color: var(--ink); box-shadow: 0 1px 2px rgb(0 0 0 / 0.14), 0 4px 14px rgb(0 0 0 / 0.12); transition: scale var(--dur-fast) var(--ease-out); } .jb:active { scale: var(--press-scale); } :global(html[data-scheme="dark"]) .jb { --paper: #3a3a3d; }
  .at { font-weight: var(--strong-weight); } .bd { position: absolute; top: -6px; right: -6px; min-width: 18px; height: 18px; padding: 0 var(--s-3); box-sizing: border-box; border-radius: var(--r-pill); display: grid; place-items: center; background: var(--badge-bg); color: #fff; font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .cp { flex: none; display: grid; gap: var(--s-3); margin: 0 var(--s-5) var(--s-5); padding: var(--s-3); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); }
  .row { display: flex; align-items: flex-end; gap: var(--s-3); } textarea { flex: 1 1 0; width: 0; min-width: 0; min-height: 34px; max-height: 8rem; padding: var(--s-4) var(--s-3); border: 0; outline: 0; resize: none; background: none; color: var(--ink); font: inherit; font-size: var(--text-ui); line-height: 1.3; }
  .ask-b { display: inline-flex; align-items: center; gap: var(--s-3); min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--hover); font-size: var(--text-label); } .ask-b:active { scale: var(--press-scale); }
  @media (max-width: 719px) { .lbl { display: none; } .ask-b { padding: 0; width: 34px; justify-content: center; } .cp { margin: 0 var(--s-4) var(--s-4); } }
  .send { flex: none; display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: var(--badge-bg); color: #fff; } .send:disabled { opacity: 0.4; }
  .who { display: flex; align-items: center; gap: var(--s-3); padding: 0 var(--s-3); font-size: var(--text-label); color: var(--ink-2); } .x { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 50%; }
</style>
