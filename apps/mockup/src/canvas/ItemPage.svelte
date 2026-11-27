<script lang="ts">
  // One item drawn big, in the look of its kind: a note's text, a mail's message, a chat's thread, a task's checklist, a file's preview, a photo, an event's date block, a contact's card, a transaction's amount, a workout's figures. Fixture only.
  import AppIcon from "../shell/AppIcon.svelte";
  import Av from "../shell/Av.svelte";
  import type { Item } from "../data/collections";
  let { item }: { item: Item } = $props();
  const tint = $derived(`oklch(0.78 0.08 ${item.hue ?? 205} / 0.55)`);
  const pos = $derived(item.amount?.startsWith("+"));
</script>

<article class="ip" data-kind={item.kind} data-item-page={item.id}>
  {#if item.kind === "note"}
    <h2>{item.title}</h2><p class="m">Edited {item.when ?? "Today"}</p><p>{item.sub}. Everything here is fixture text, standing in for the note's body.</p><p>A second paragraph, to show a line length and a paragraph gap in a window.</p><ul><li>Book the tickets</li><li>Check the weather</li><li>Ask Hana about the key</li></ul>
  {:else if item.kind === "mail"}
    <header class="mh"><Av name={item.title} size="2.5rem" /><div><strong>{item.title}</strong><span class="m">to me · {item.when}</span></div></header><h2>{item.sub}</h2><p>Hello Ada, this is a fixture message. It has a few lines so the window has something to show.</p><p>Best wishes,<br />{item.title}</p>
  {:else if item.kind === "chat"}
    <h2>{item.title}</h2>
    <div class="th"><div class="b o">Can we agree the plan before Friday?<time>9:16</time></div><div class="b">{item.sub}<time>{item.when ?? ""}</time></div><div class="b ag"><em>Codex</em> The river place has a free table at 19:00.<time>9:21</time></div><div class="b o">River it is. Booking now.<time>9:24</time></div></div>
  {:else if item.kind === "task"}
    <h2>{item.title}</h2><p class="m">{item.sub} · due {item.due}</p><div class="steps">{#each ["Find the page", "Fill the form", "Pay the fee"] as s, i (s)}<label><input type="checkbox" checked={item.status === "Done" || i === 0} />{s}</label>{/each}</div>
  {:else if item.kind === "file"}
    <h2>{item.title}</h2><p class="m">{item.sub}</p><div class="pv"><AppIcon name="file" /><span>Preview of {item.title}</span></div>
  {:else if item.kind === "photo"}
    <h2>{item.title}</h2><div class="ph" style:background={tint}></div><p class="m">{item.sub} · {item.when ?? "12 Nov"}</p>
  {:else if item.kind === "event"}
    <div class="ev"><span class="dc">{(item.due ?? "").slice(0, 3).toUpperCase()}</span><div><h2>{item.title}</h2><p class="m">{item.sub}</p></div></div><p>Location, guests and notes would be here.</p>
  {:else if item.kind === "contact"}
    <header class="ct"><Av name={item.title} size="4.5rem" /><div><h2>{item.title}</h2><p class="m">{item.sub}</p></div></header><dl><dt>Mobile</dt><dd>+351 912 000 000</dd><dt>Email</dt><dd>{item.title.split(" ")[0]?.toLowerCase()}@example.org</dd></dl>
  {:else if item.kind === "transaction"}
    <p class="m">{item.sub} · {item.when}</p><p class="amt" class:pos>{item.amount}</p><h2>{item.title}</h2><dl><dt>Account</dt><dd>Household</dd><dt>Category</dt><dd>Travel</dd></dl>
  {:else if item.kind === "workout"}
    <h2>{item.title}</h2><div class="stats"><div><b>{item.sub}</b><span>Distance</span></div><div><b>{item.when ?? "31:04"}</b><span>Time</span></div><div><b>5:58</b><span>Pace /km</span></div><div><b>142</b><span>Heart rate</span></div></div>
  {:else}
    <h2>{item.title}</h2><p class="m">{item.sub}</p>
  {/if}
</article>

<style>
  .ip { display: grid; gap: var(--s-3); align-content: start; max-width: 40rem; padding-top: var(--s-2); } h2 { margin: 0; font-size: var(--text-title); line-height: 1.2; font-weight: var(--strong-weight); } p { margin: 0; line-height: 1.45; font-size: var(--text-ui); } .m { color: var(--ink-3); font-size: var(--text-label); line-height: 1.25; }
  ul { margin: 0; padding-inline-start: var(--s-8); list-style: disc; } .mh, .ct { display: flex; align-items: center; gap: var(--s-4); } .mh div, .ct div { display: grid; }
  .th { display: flex; flex-direction: column; gap: var(--s-2); } .b { align-self: flex-start; padding: var(--s-3) var(--s-4); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); } .b.o { align-self: flex-end; background: var(--accent-soft); } .b time { margin-inline-start: var(--s-3); color: var(--ink-3); font-size: var(--text-label); } .ag em { font-style: normal; font-weight: var(--strong-weight); }
  .steps { display: grid; gap: var(--s-3); } .pv { display: grid; place-items: center; gap: var(--s-3); aspect-ratio: 4 / 3; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-3); } .ph { aspect-ratio: 4 / 3; border-radius: var(--r-card); }
  .ev { display: flex; align-items: center; gap: var(--s-5); } .dc { min-width: 72px; padding: var(--s-5) 0; text-align: center; border-radius: var(--r-menu); background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); letter-spacing: 0.06em; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-2) var(--s-5); margin: 0; } dt { color: var(--ink-3); } dd { margin: 0; }
  .amt { font-size: var(--text-title); font-weight: var(--strong-weight); font-variant-numeric: tabular-nums; } .amt.pos { color: var(--good, oklch(0.62 0.14 150)); }
  .stats { display: grid; grid-template-columns: repeat(2, 1fr); gap: var(--s-4); } .stats div { display: grid; padding: var(--s-4); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.06); } .stats b { font-size: var(--text-title); } .stats span { color: var(--ink-3); font-size: var(--text-label); }
</style>
