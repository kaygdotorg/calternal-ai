<script lang="ts">
  // The reading page: one conversation. The subject is the page's title, with one quiet line under it (who, how many messages, where it is filed); everything about headers stays out of the way until asked for ("to me" opens From, To, Cc, Date and the account).
  // The latest message and every unread one are open; older ones are a single line that opens on a tap. Quoted text is folded behind a small pill. Attachments sit under the message they came with, pictures as tiles. Reply, Reply all and Forward open a
  // compose window beside this one. MOTION (see motion.ts): a message opening rises and fades in over `--dur-base` on `--ease-out`; a header's details and a folded quote unfold (height first, then the text fades in) over the same duration and curve; the chevron turns over `--dur-fast`. Each reverses from where it is if tapped again; reduced motion makes all of them instant (the tokens read 1 ms).
    import { Button, IconButton } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import AppIcon from "../../shell/AppIcon.svelte";
  import { rise } from "../../motion";
  import { unfold } from "./motion";
  import GlyphButton from "./GlyphButton.svelte";
  import type { Item } from "../../data/collections";
  import AttChip from "./AttChip.svelte";
  import { startDraft } from "./compose";
  import { THREADS, type Thread } from "./data";
  import { addrOf } from "../contacts/people";
  let { item }: { item: Item } = $props();
  /** An id with no conversation of its own draws as one message made from its row. */
  const t = $derived<Thread>(THREADS[item.id] ?? { subject: item.sub ?? "(no subject)", account: "Personal", labels: [], msgs: [{ from: item.title, to: "me", at: item.when ?? "Today", short: item.when ?? "", paras: ["This is a fixture message. It has a few lines so the page has something to show.", "Best wishes,\n" + item.title], unread: item.unread }] });
  let open = $state<Record<number, boolean | undefined>>({}), heads = $state<Record<number, boolean>>({}), quotes = $state<Record<number, boolean>>({}), flagged = $state<boolean | undefined>();
  $effect(() => { item.id; open = {}; heads = {}; quotes = {}; flagged = undefined; });
  const isOpen = (i: number) => open[i] ?? (i === t.msgs.length - 1 || !!t.msgs[i]?.unread);
  const last = $derived(t.msgs.at(-1)!);
  const nopeople = $derived(new Set(t.msgs.map((m) => m.from)).size);
  const snippet = (i: number) => t.msgs[i]!.paras.join(" ");
</script>

<div class="mp" data-mail-thread={item.id}>
  <article class="page">
    <header class="top">
      <div class="tt"><h1 title={t.subject}>{t.subject}</h1>
        <div class="acts" role="toolbar" aria-label="Conversation">
          <IconButton icon="archive" label="Archive" /><IconButton icon="trash" label="Delete" />
          <GlyphButton icon="flag" label={(flagged ?? t.flagged) ? "Remove the flag" : "Flag"} pressed={flagged ?? t.flagged} onclick={() => (flagged = !(flagged ?? t.flagged))} />
          <IconButton icon="more" label="More" />
        </div>
      </div>
      <p class="meta"><span>{nopeople === 1 ? t.msgs[0]!.from : `${nopeople} people`}</span><i aria-hidden="true">·</i><span>{t.msgs.length} {t.msgs.length === 1 ? "message" : "messages"}</span><i aria-hidden="true">·</i><span class="lb">{t.account}</span>
        {#each t.labels as l (l)}<span class="lb">{l}</span>{/each}</p>
    </header>

    <ol class="msgs">
      {#each t.msgs as m, i (i)}
        {@const shown = isOpen(i)}
        <li class="msg" class:shut={!shown} class:unread={m.unread} data-open={shown}>
          {#if !shown}
            <button type="button" class="row" aria-expanded="false" onclick={() => (open[i] = true)}>
              <Av name={m.from} size="24px" /><span class="nm">{m.mine ? "Me" : m.from}</span><span class="sn">{snippet(i)}</span><time>{m.short}</time>
            </button>
          {:else}
            <div class="op" in:rise>
            <div class="mh">
              <Av name={m.from} size="36px" />
              <div class="who">
                <button type="button" class="nmb" aria-expanded={!!heads[i]} onclick={() => (heads[i] = !heads[i])}>
                  <strong>{m.mine ? "Me" : m.from}</strong>
                </button>
                <button type="button" class="to" aria-expanded={!!heads[i]} onclick={() => (heads[i] = !heads[i])}><span class="tl">to {m.to}</span><span class="cv" class:up={heads[i]}><AppIcon name="chevron-down" inline /></span></button>
              </div>
              <time>{m.at.replace(/^(.*?), (\d\d:\d\d)$/, "$1 · $2")}</time>
              {#if t.msgs.length > 1}<span class="hide"><IconButton small icon="chevron-down" label="Collapse this message" onclick={() => (open[i] = false)} /></span>{/if}
            </div>
            {#if heads[i]}
              <dl class="hd" transition:unfold>
                <dt>From</dt><dd>{m.from} <span class="ad">&lt;{m.mine ? "ada@lindqvist.example" : addrOf(m.from)}&gt;</span></dd>
                <dt>To</dt><dd>{m.to}</dd>{#if m.cc}<dt>Cc</dt><dd>{m.cc}</dd>{/if}
                <dt>Date</dt><dd>{m.at}</dd><dt>Filed in</dt><dd>{t.account}, {t.labels.length ? t.labels.join(", ") : "Inbox"}</dd>
              </dl>
            {/if}
            <div class="body">{#each m.paras as p, k (k)}<p>{p}</p>{/each}</div>
            {#if m.quoted}
              <button type="button" class="fold" aria-expanded={!!quotes[i]} aria-label={quotes[i] ? "Hide the quoted text" : "Show the quoted text"} onclick={() => (quotes[i] = !quotes[i])}><AppIcon name="more" inline /></button>
              {#if quotes[i]}<blockquote transition:unfold><p class="qh">{m.quoted.head}</p>{#each m.quoted.lines as l, k (k)}{#if l}<p>{l}</p>{:else}<br />{/if}{/each}</blockquote>{/if}
            {/if}
            {#if m.atts?.length}<ul class="atts" aria-label="Attachments">{#each m.atts as a (a.name)}<li><AttChip att={a} /></li>{/each}</ul>{/if}
            </div>
          {/if}
        </li>
      {/each}
    </ol>

    <footer class="reply">
      <Button text="Reply" icon="reply" onclick={() => startDraft("reply", item.id)} />
      {#if !last.bulk && nopeople > 1}<Button text="Reply all" icon="replyall" variant="quiet" onclick={() => startDraft("all", item.id)} />{/if}
      <Button text="Forward" icon="forward" variant="quiet" onclick={() => startDraft("forward", item.id)} />
    </footer>
  </article>
</div>

<style>
  .mp { flex: 1; min-height: 0; overflow: auto; container-type: inline-size; scrollbar-width: thin; } .page { max-width: 44rem; padding-bottom: var(--s-9); }
  /* the top: the subject is the title; the line under it is quiet */
  .top { display: grid; gap: var(--s-3); padding: var(--s-2) 0 var(--s-5); } .tt { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--s-5); }
  h1 { flex: 1; min-width: 0; margin: 0; padding-top: var(--s-2); font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; text-wrap: balance; overflow-wrap: anywhere; }
  .acts { flex: none; display: flex; align-items: center; gap: var(--s-1); }
  .meta { margin: 0; display: flex; align-items: center; flex-wrap: wrap; gap: var(--s-2) var(--s-3); color: var(--ink-3); font-size: var(--text-label); line-height: 1.3; } .meta i { font-style: normal; }
  .lb { padding: 0 var(--s-4); min-height: var(--s-7); display: inline-flex; align-items: center; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); font-size: var(--text-label); line-height: 1.25; }
  /* the messages: one hairline between them, no boxes */
  .msgs { list-style: none; margin: 0; padding: 0; } .msg { border-top: 1px solid rgb(var(--ink-rgb) / 0.09); padding: var(--s-5) 0; } .msg[data-open="true"] { padding: var(--s-6) 0 var(--s-5); }
  .row { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: 32px; padding: 0 var(--s-3); margin-inline: calc(var(--s-3) * -1); box-sizing: content-box; border-radius: var(--r-select); text-align: start; color: var(--ink); font-size: var(--text-ui); transition: background-color var(--dur-fast) var(--ease-out); }
  @media (hover: hover) and (pointer: fine) { .row:hover { background: var(--hover); } }
  .row .nm { flex: none; max-width: 40%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: var(--strong-weight); } .row .sn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ink-3); } .row time { flex: none; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; }
  .mh { display: flex; align-items: center; gap: var(--s-5); } .who { flex: 1; min-width: 0; display: grid; justify-items: start; } .nmb { max-width: 100%; text-align: start; color: var(--ink); font-size: var(--text-ui); line-height: 1.3; } .nmb strong { font-weight: var(--strong-weight); display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .unread .nmb strong::after { content: ""; display: inline-block; width: 8px; height: 8px; margin-inline-start: var(--s-4); border-radius: 50%; background: var(--accent-base); vertical-align: 0.05em; }
  .to { display: inline-flex; align-items: center; gap: var(--s-1); max-width: 100%; color: var(--ink-3); font-size: var(--text-label); line-height: 1.3; text-align: start; } .tl { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .cv { flex: none; display: grid; transition: rotate var(--dur-fast) var(--ease-out); } .cv.up { rotate: 180deg; }
  .mh time { flex: none; color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; text-align: end; } .hide { flex: none; }
  .hd { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-2) var(--s-5); margin: var(--s-5) 0 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); font-size: var(--text-label); line-height: 1.35; } .hd dt { color: var(--ink-3); } .hd dd { margin: 0; overflow-wrap: anywhere; } .ad { color: var(--ink-3); }
  .body { margin-top: var(--s-5); display: grid; gap: var(--s-4); font-size: var(--text-ui); line-height: 1.55; overflow-wrap: anywhere; } .body p, blockquote p { margin: 0; white-space: pre-line; }
  .fold { display: inline-grid; place-items: center; width: 34px; height: 20px; margin-top: var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-3); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .fold:active { scale: var(--press-scale); } @media (hover: hover) and (pointer: fine) { .fold:hover { background: rgb(var(--ink-rgb) / 0.14); } }
  blockquote { display: grid; gap: var(--s-3); margin: var(--s-4) 0 0; padding-inline-start: var(--s-5); border-inline-start: 2px solid rgb(var(--ink-rgb) / 0.16); color: var(--ink-3); font-size: var(--text-label); line-height: 1.5; overflow-wrap: anywhere; } .qh { color: var(--ink-3); }
  .atts { list-style: none; display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--s-4); margin: var(--s-5) 0 0; padding: 0; } .atts li { min-width: 0; max-width: 100%; }
  .reply { display: flex; flex-wrap: wrap; gap: var(--s-3); padding-top: var(--s-6); border-top: 1px solid rgb(var(--ink-rgb) / 0.09); }
  @container (max-width: 30rem) { h1 { font-size: var(--text-heading); } .reply :global(svg) { display: none; } .tt { flex-direction: column; gap: var(--s-2); } .mh time { display: none; } .acts { margin-inline-start: calc(var(--s-3) * -1); } }
</style>
