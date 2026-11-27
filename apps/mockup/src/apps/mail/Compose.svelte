<script lang="ts">
  // The compose window: a second canvas window, so the message being answered stays open beside it. Fields are one line each (From, To, Cc, Subject) with a hairline between, then the body, the quoted message folded under the cursor, attachments, and one
  // action row: Send (the one primary), attach, link, and Discard at the far end; the quiet "Saved" line says the draft is already kept. Typing in To suggests people from Contacts. Send does not close anything: the window turns into a short "Sent"
  // note with Undo for six seconds (the message is held that long), then offers to close. MOTION (see motion.ts), in order: the shell grows the window over `--dur-morph` on `--ease-morph`; when it is about 55 % grown the form fades in and rises 6 px over `--dur-base` on `--ease-out`; when the window has settled the cursor goes to To (new message) or to the text (a reply, where To is filled). The suggestions and the sent note fade and rise over `--dur-base`; the Cc row and the folded quote unfold (height, then text). Each reverses from where it is when interrupted; reduced motion makes all of it instant, and the cursor still moves.
  import { onDestroy } from "svelte";
    import { Button, IconButton } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import { tick as settle } from "svelte";
  import { arrive, unfold } from "./motion";
  import { onMount } from "svelte";
  import { wins } from "../../windows.svelte";
  import type { Item } from "../../data/collections";
  import AttChip from "./AttChip.svelte";
  import { DRAFTS, THREADS, type Att } from "./data";
  import { CONTACT_ITEMS, PEOPLE } from "../contacts/people";
  let { item, win }: { item: Item; win?: { id: string } } = $props();
  /** The window this compose is in: the one given, or (an item window gets no props but the item) the item window of this draft. */
  const wid = $derived(win?.id ?? wins.list.find((w) => w.kind === "item" && w.ref === item.id)?.id);
  const d = $derived(DRAFTS[item.id] ?? DRAFTS["draft-new"]!);
  let to = $state<string[]>([]), cc = $state<string[]>([]), subject = $state(""), body = $state(""), atts = $state<Att[]>([]), from = $state("Personal"), typed = $state(""), ccTyped = $state(""), showCc = $state(false), saved = $state(""), quote = $state(false), sent = $state<number | null>(null), field = $state<"to" | "cc" | null>(null);
  $effect(() => { const x = d; to = [...x.to]; cc = [...x.cc]; subject = x.subject; body = x.body; atts = [...x.atts]; from = x.from; saved = x.saved; showCc = x.cc.length > 0; sent = null; quote = false; });
  let timer: ReturnType<typeof setTimeout> | undefined, tick: ReturnType<typeof setInterval> | undefined;
  const touch = () => { saved = "Saving…"; clearTimeout(timer); timer = setTimeout(() => (saved = "Saved just now"), 700); };
  const q = $derived((field === "cc" ? ccTyped : typed).trim().toLowerCase());
  const sugg = $derived(!field || !q ? [] : CONTACT_ITEMS.filter((c) => !PEOPLE[c.id]?.me && PEOPLE[c.id]!.emails.length && !to.includes(c.title) && !cc.includes(c.title) && (c.title.toLowerCase().includes(q) || PEOPLE[c.id]!.emails.some((e) => e[1].includes(q)))).slice(0, 4));
  const add = (name: string) => { if (field === "cc") { cc = [...cc, name]; ccTyped = ""; } else { to = [...to, name]; typed = ""; } touch(); };
  const commit = (which: "to" | "cc") => { const v = (which === "cc" ? ccTyped : typed).trim().replace(/,$/, ""); if (!v) return; if (which === "cc") { cc = [...cc, v]; ccTyped = ""; } else { to = [...to, v]; typed = ""; } touch(); };
  function key(e: KeyboardEvent, which: "to" | "cc") {
    const v = which === "cc" ? ccTyped : typed;
    if (e.key === "Enter" || e.key === ",") { e.preventDefault(); if (sugg[0] && v.trim()) add(sugg[0].title); else commit(which); }
    else if (e.key === "Backspace" && !v) { if (which === "cc") cc = cc.slice(0, -1); else to = to.slice(0, -1); }
  }
  const quoted = $derived(d.replyTo ? THREADS[d.replyTo]?.msgs.at(-1) : undefined);
  const size = $derived(atts.reduce((s, a) => s + (parseFloat(a.size) * (a.size.includes("MB") ? 1024 : 1)), 0));
  function send() { sent = 6; clearInterval(tick); tick = setInterval(() => { if (sent !== null && sent > 0) sent -= 1; else clearInterval(tick); }, 1000); }
  const undo = () => { clearInterval(tick); sent = null; };
  onDestroy(() => { clearTimeout(timer); clearInterval(tick); });
  const fly = (n: Element) => arrive(n);
  let toEl = $state<HTMLInputElement>(), bodyEl = $state<HTMLTextAreaElement>();
  /** Once the window has grown: the cursor goes to To for a new message, to the message for a reply (To is already filled). */
  onMount(() => { const w = setTimeout(async () => { await settle(); (to.length ? bodyEl : toEl)?.focus({ preventScroll: true }); }, 260); return () => clearTimeout(w); });
</script>

<div class="cp" data-compose={item.id}>
  {#if sent !== null}
    <div class="done" in:fly>
      <h2>{sent > 0 ? "Sending" : "Sent"}</h2>
      <p>{sent > 0 ? `Your message to ${to.join(", ") || "no one"} goes in ${sent} ${sent === 1 ? "second" : "seconds"}.` : `Your message to ${to.join(", ") || "no one"} is on its way.`}</p>
      <div class="row">{#if sent > 0}<Button text="Undo" variant="secondary" onclick={undo} />{/if}<Button text="Close this window" variant="quiet" onclick={() => wid && wins.close(wid)} /></div>
    </div>
  {:else}
    <div class="form" in:arrive={{ after: 0.55 }}>
      <div class="f"><span class="l">From</span><span class="v"><span class="acct"><b>{from === "Studio" ? "Ada Lindqvist" : "Ada Lindqvist"}</b><span class="ad">{from === "Studio" ? "ada@studio-lindqvist.example" : "ada@lindqvist.example"}</span></span></span></div>
      <div class="f"><label class="l" for="to-{item.id}">To</label>
        <div class="v chips">
          {#each to as n, i (n + i)}<span class="pill"><Av name={n} size="20px" /><span class="pn">{n}</span><IconButton small icon="close" label={`Remove ${n}`} onclick={() => { to = to.filter((_, k) => k !== i); touch(); }} /></span>{/each}
          <input id="to-{item.id}" bind:this={toEl} class="in" bind:value={typed} placeholder={to.length ? "" : "Name or address"} autocomplete="off" onfocus={() => (field = "to")} onblur={() => { commit("to"); field = field === "to" ? null : field; }} onkeydown={(e) => key(e, "to")} />
        </div>
        {#if !showCc}<button type="button" class="more" onclick={() => (showCc = true)}>Cc</button>{/if}
        {#if field === "to" && sugg.length}<ul class="sg" in:fly role="listbox" aria-label="People">{#each sugg as c (c.id)}<li><button type="button" role="option" aria-selected="false" onmousedown={(e) => { e.preventDefault(); add(c.title); }}><Av name={c.title} size="24px" /><span class="pn">{c.title}</span><span class="ad">{PEOPLE[c.id]!.emails[0]![1]}</span></button></li>{/each}</ul>{/if}
      </div>
      {#if showCc}
        <div class="f" transition:unfold><label class="l" for="cc-{item.id}">Cc</label>
          <div class="v chips">
            {#each cc as n, i (n + i)}<span class="pill"><Av name={n} size="20px" /><span class="pn">{n}</span><IconButton small icon="close" label={`Remove ${n}`} onclick={() => { cc = cc.filter((_, k) => k !== i); touch(); }} /></span>{/each}
            <input id="cc-{item.id}" class="in" bind:value={ccTyped} autocomplete="off" onfocus={() => (field = "cc")} onblur={() => { commit("cc"); field = field === "cc" ? null : field; }} onkeydown={(e) => key(e, "cc")} />
          </div>
          {#if field === "cc" && sugg.length}<ul class="sg" in:fly role="listbox" aria-label="People">{#each sugg as c (c.id)}<li><button type="button" role="option" aria-selected="false" onmousedown={(e) => { e.preventDefault(); add(c.title); }}><Av name={c.title} size="24px" /><span class="pn">{c.title}</span><span class="ad">{PEOPLE[c.id]!.emails[0]![1]}</span></button></li>{/each}</ul>{/if}
        </div>
      {/if}
      <div class="f"><label class="l" for="sb-{item.id}">Subject</label><input id="sb-{item.id}" class="in sub" bind:value={subject} oninput={touch} placeholder="What is it about?" /></div>
      <textarea class="body" bind:this={bodyEl} aria-label="Message" bind:value={body} oninput={touch} placeholder="Write your message"></textarea>
      {#if quoted}
        <button type="button" class="fold" aria-expanded={quote} aria-label={quote ? "Hide the quoted message" : "Show the quoted message"} onclick={() => (quote = !quote)}>···</button>
        {#if quote}<blockquote transition:unfold><p class="qh">On {quoted.at}, {quoted.from} wrote:</p>{#each quoted.paras as p, k (k)}<p>{p}</p>{/each}</blockquote>{/if}
      {/if}
      {#if atts.length}<ul class="atts" aria-label="Attachments">{#each atts as a, i (a.name)}<li><AttChip att={a} onremove={() => { atts = atts.filter((_, k) => k !== i); touch(); }} /></li>{/each}</ul>{/if}
    </div>
    <footer class="bar">
      <Button text="Send" variant="primary" icon="send" disabled={!to.length} onclick={send} />
      <IconButton icon="paperclip" label="Attach a file" onclick={() => { atts = [...atts, { name: "Scan 0412.png", size: "2.1 MB", type: "image" }]; touch(); }} />
      <IconButton icon="link" label="Insert a link" /><IconButton icon="image" label="Insert a picture" />
      <span class="st" aria-live="polite">{saved}{#if atts.length && size > 0}{saved ? " · " : ""}{(size / 1024).toFixed(1)} MB of 25{/if}</span>
      <IconButton icon="trash" label="Discard the draft" onclick={() => wid && wins.close(wid)} />
    </footer>
  {/if}
</div>

<style>
  .cp { flex: 1; min-height: 0; display: flex; flex-direction: column; container-type: inline-size; } .form { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; display: flex; flex-direction: column; max-width: 44rem; width: 100%; }
  .f { position: relative; display: flex; align-items: flex-start; gap: var(--s-5); min-height: var(--row-h); border-bottom: 1px solid rgb(var(--ink-rgb) / 0.09); } .l, .more { display: flex; align-items: center; min-height: var(--row-h); } .l { flex: none; width: 3.5rem; color: var(--ink-3); font-size: var(--text-label); } .v { flex: 1; min-width: 0; min-height: var(--row-h); display: flex; align-items: center; font-size: var(--text-ui); }
  .acct { display: flex; align-items: baseline; gap: var(--s-4); min-width: 0; flex-wrap: wrap; } .ad { color: var(--ink-3); font-size: var(--text-label); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chips { flex-wrap: wrap; gap: var(--s-2) var(--s-3); padding: var(--s-2) 0; box-sizing: border-box; } .pill { display: inline-flex; align-items: center; gap: var(--s-3); max-width: 100%; min-height: 28px; padding: 0 var(--s-2) 0 var(--s-2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); } .pn { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .in { flex: 1 1 3rem; min-width: 3rem; min-height: 32px; padding: 0; border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: var(--text-ui); } .in::placeholder, .body::placeholder { color: var(--ink-3); } .f > .sub { align-self: center; } .sub { font-weight: var(--strong-weight); } .sub::placeholder { font-weight: var(--ui-weight); }
  .more { flex: none; color: var(--ink-3); font-size: var(--text-label); padding: var(--s-2) var(--s-3); border-radius: var(--r-select); } @media (hover: hover) and (pointer: fine) { .more:hover { background: var(--hover); color: var(--ink); } }
  .sg { position: absolute; z-index: 2; top: 100%; left: 3.5rem; right: 0; max-width: 24rem; list-style: none; margin: var(--s-2) 0 0; padding: var(--s-2); border-radius: var(--r-menu); background: rgb(var(--tint) / 0.96); box-shadow: var(--shadow-float); } .sg button { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: 36px; padding: 0 var(--s-4); border-radius: var(--r-select); text-align: start; color: var(--ink); font-size: var(--text-ui); } .sg .ad { margin-inline-start: auto; } @media (hover: hover) and (pointer: fine) { .sg button:hover { background: var(--hover); } }
  .body { flex: none; display: block; width: 100%; min-height: 12rem; box-sizing: border-box; padding: var(--s-5) 0; border: 0; outline: 0; resize: none; field-sizing: content; background: transparent; color: var(--ink); font: inherit; font-size: var(--text-ui); line-height: 1.55; }
  .fold { align-self: flex-start; width: 34px; height: 20px; line-height: 12px; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-3); letter-spacing: 0.1em; text-align: center; } blockquote { margin: var(--s-4) 0 0; padding-inline-start: var(--s-5); border-inline-start: 2px solid rgb(var(--ink-rgb) / 0.16); color: var(--ink-3); font-size: var(--text-label); line-height: 1.5; } blockquote p { margin: 0 0 var(--s-3); }
  .atts { list-style: none; display: flex; flex-wrap: wrap; gap: var(--s-4); margin: var(--s-5) 0 0; padding: 0; }
  .bar { flex: none; display: flex; align-items: center; gap: var(--s-2); padding-top: var(--s-4); max-width: 44rem; } .bar :global(.btn) { margin-inline-end: var(--s-3); } .st { flex: 1; min-width: 0; padding-inline: var(--s-4); color: var(--ink-3); font-size: var(--text-label); text-align: end; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .done { display: grid; gap: var(--s-4); align-content: start; max-width: 28rem; padding-top: var(--s-6); } h2 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); } .done p { margin: 0; color: var(--ink-2); font-size: var(--text-ui); line-height: 1.5; } .row { display: flex; gap: var(--s-3); }
  @container (max-width: 30rem) { .l { width: 2.75rem; } .sg { left: 0; } }
</style>
