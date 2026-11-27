<script lang="ts">
  // A contact, as a page: the picture (initials until there is one), the name and what the person is to me, four actions that go to the other apps (Message, Mail, Event, Share), the fields in one grouped block (label above value, so a long address or note
  // never fights a label for width), and then what the rest of the product knows about them as three small collections: recent mail, chats and events, each drawn with its own kind's row and each with "Show all" that opens it as a collection. This is the "one app
  // for everything" idea on one page. A person with nothing but a name shows an invitation to add a field, not empty headings. Fixture only.
  import AppIcon from "../../shell/AppIcon.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import ItemRow from "../../shell/Item.svelte";
  import MailLook from "../mail/MailLook.svelte";
  import EventLook from "../calendar/EventLook.svelte";
  import { EV } from "../calendar/data";
  import { ITEMS, type Item } from "../../data/collections";
  import { wins } from "../../windows.svelte";
  import { PEOPLE } from "./people";
  import { SIDE } from "./data";
  import { startDraft } from "../mail/compose";
  let { item, onback, onall }: { item: Item; onback?: () => void; onall?: (cid: string) => void } = $props();
  const p = $derived(PEOPLE[item.id]);
  const bag = $derived(p ? SIDE.map((s) => ({ ...s, items: p[s.key].map((id) => ITEMS[id]).filter((i): i is Item => !!i) })).filter((s) => s.items.length) : []);
  const empty = $derived(!!p && !p.phones.length && !p.emails.length && !p.address && !p.birthday && !p.notes);
  const show = (cid: string) => (onall ? onall(cid) : wins.beside("collection", cid));
  const chat = $derived(p?.chats[0] ? ITEMS[p.chats[0]] : undefined);
  const mailTo = () => { startDraft("new", undefined, p ? [p.name] : []); };
</script>

<article class="cc" data-contact={item.id}>
  {#if onback}<button type="button" class="back" onclick={onback}>‹ People</button>{/if}
  {#if p}
    <header class="hero"><Av name={p.name} size="5.5rem" />
      <div class="id"><h1>{p.name}{#if p.me}<span class="me">Me</span>{/if}</h1>{#if p.role || p.org}<p class="m">{[p.role, p.org].filter(Boolean).join(" · ")}</p>{/if}</div></header>
    <div class="act" role="toolbar" aria-label="Contact">
      <div><button type="button" class="rb" aria-label="Message" disabled={!chat} onclick={() => chat && wins.openItem(chat)}><AppIcon name="chat" /></button><span>Message</span></div>
      <div><button type="button" class="rb" aria-label="Call" disabled={!p.phones.length}><Glyph name="phone" /></button><span>Call</span></div>
      <div><button type="button" class="rb" aria-label="Mail" disabled={!p.emails.length} onclick={mailTo}><AppIcon name="mail" /></button><span>Mail</span></div>
      <div><button type="button" class="rb" aria-label="Event"><AppIcon name="calendar" /></button><span>Event</span></div>
      <div><button type="button" class="rb" aria-label="Share"><AppIcon name="share" /></button><span>Share</span></div>
    </div>
    {#if empty}
      <section class="nothing"><p>Only a name so far.</p><p class="m">Add a phone number, an email or an address and it appears here.</p></section>
    {:else}
      <dl class="grp">
        {#each p.phones as [l, v] (l + v)}<div><dt>{l}</dt><dd class="act1">{v}</dd></div>{/each}
        {#each p.emails as [l, v] (l + v)}<div><dt>{l}</dt><dd class="act1">{v}</dd></div>{/each}
        {#if p.address}<div><dt>address</dt><dd>{#each p.address as l (l)}<span class="ln">{l}</span>{/each}</dd></div>{/if}
        {#if p.birthday}<div><dt class="bd"><Glyph name="gift" inline />birthday</dt><dd>{p.birthday}</dd></div>{/if}
      </dl>
      {#if p.notes}<div class="grp"><div><dt>notes</dt><dd class="notes">{p.notes}</dd></div></div>{/if}
    {/if}
    {#each bag as s (s.key)}
      <section class="coll"><header><h3>{s.title}</h3><button type="button" onclick={() => show(`ct-${item.id}-${s.key}`)}>Show all{s.items.length > 3 ? ` ${s.items.length}` : ""}</button></header>
        <div class="rows">{#each s.items.slice(0, 3) as it (it.id)}{#if s.key === "mail"}<MailLook item={it} mode="row" onopen={(i) => wins.openItem(i)} />{:else if s.key === "events" && EV[it.id]}<EventLook ev={EV[it.id]!} mode="row" onopen={() => wins.openItem(it)} />{:else}<ItemRow item={it} onopen={(i) => wins.openItem(i)} />{/if}{/each}</div></section>
    {/each}
  {/if}
</article>

<style>
  .cc { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; container-type: inline-size; max-width: 36rem; padding-bottom: var(--s-9); } .back { margin: 0 0 var(--s-3) calc(var(--s-3) * -1); padding: var(--s-2) var(--s-4); border-radius: var(--r-pill); color: var(--accent); font-size: var(--text-label); }
  .hero { display: flex; align-items: center; gap: var(--s-6); padding: var(--s-3) 0 var(--s-5); } .id { min-width: 0; } h1 { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; overflow-wrap: anywhere; } .m { margin: var(--s-1) 0 0; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.4; }
  .me { padding: 1px var(--s-4); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); font-weight: var(--ui-weight); }
  .act { display: grid; grid-template-columns: repeat(5, 1fr); gap: var(--s-3); padding: var(--s-3) 0 var(--s-6); } .act div { display: grid; justify-items: center; gap: var(--s-2); } .rb { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 50%; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .rb:active { scale: var(--press-scale); } .rb:disabled { opacity: 0.4; } @media (hover: hover) and (pointer: fine) { .rb:not(:disabled):hover { background: rgb(var(--ink-rgb) / 0.14); } }
  .act span { color: var(--ink-2); font-size: var(--text-label); }
  .grp { display: grid; margin: 0 0 var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); overflow: hidden; } .grp > div { display: grid; gap: 1px; padding: var(--s-4) var(--s-6); } .grp > div + div { border-top: 1px solid rgb(var(--ink-rgb) / 0.08); }
  .bd { display: flex; align-items: center; gap: var(--s-2); } dt { color: var(--ink-3); font-size: var(--text-label); text-transform: capitalize; } dd { margin: 0; font-size: var(--text-ui); line-height: 1.45; overflow-wrap: anywhere; } .act1 { color: var(--accent); } .ln { display: block; } .notes { color: var(--ink-2); white-space: pre-line; }
  .nothing { padding: var(--s-6) 0; } .nothing p { margin: 0; font-size: var(--text-ui); } .nothing .m { margin-top: var(--s-2); }
  .coll { margin-top: var(--gap-before-head); } .coll header { display: flex; align-items: baseline; justify-content: space-between; gap: var(--s-4); padding-inline: var(--s-5); margin-bottom: var(--gap-head-items); } h3 { margin: 0; color: var(--ink-2); font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.2; }
  .coll header button { color: var(--accent); font-size: var(--text-label); padding: var(--s-1) var(--s-3); border-radius: var(--r-select); } @media (hover: hover) and (pointer: fine) { .coll header button:hover { background: var(--hover); } } .rows { display: grid; grid-template-columns: minmax(0, 1fr); }
</style>
