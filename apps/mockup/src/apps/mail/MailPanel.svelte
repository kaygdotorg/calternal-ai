<script lang="ts">
  import PanelFrame from "../../shell/PanelFrame.svelte";
  // Mail's right panel. For an open conversation: the sender (a small card that opens their contact page), who is on it, the attachments, the labels, and what it is about elsewhere (an event, a task, a chat: Related, drawn with each kind's own row). For a draft: where
  // it goes from and how big it is. With nothing open: the two accounts. Assistant is the shared tab, with suggestions fitted to mail. Fixture only.
  import Av from "../../shell/Av.svelte";
  import ItemRow from "../../shell/Item.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import { wins, type Win } from "../../windows.svelte";
  import AttChip from "./AttChip.svelte";
  import { DRAFTS, THREADS } from "./data";
  import { addrOf, personByName } from "../contacts/people";
  let { item }: { win?: Win | undefined; item?: Item } = $props();
  const t = $derived(item ? THREADS[item.id] : undefined), draft = $derived(item ? DRAFTS[item.id] : undefined);
  const first = $derived(t?.msgs.find((m) => !m.mine)?.from ?? item?.title);
  const sender = $derived(first ? personByName(first) : undefined);
  const people = $derived([...new Map((t?.msgs ?? []).map((m) => [m.from, m])).values()].map((m) => m.from));
  const atts = $derived((t?.msgs ?? []).flatMap((m) => m.atts ?? []));
  const related = $derived((t?.related ?? []).map((id) => ITEMS[id]).filter((x): x is Item => !!x));
  const title = $derived(draft ? "Draft" : (item?.sub ?? "Mail"));
</script>

<PanelFrame title={title}>
      {#if t && first}
        <section><h3>From</h3>
          <button type="button" class="card" onclick={() => sender && wins.beside("item", sender.id)} disabled={!sender}><Av name={first} size="40px" /><span class="ct"><strong>{first}</strong><span class="s">{addrOf(first)}</span></span></button>
        </section>
        <section><h3>In this conversation</h3><ul>{#each people as p (p)}<li><Av name={p} size="24px" /><span class="pn">{p}</span><span class="rl">{t.msgs.filter((m) => m.from === p).length}</span></li>{/each}</ul></section>
        {#if atts.length}<section><h3>Attachments</h3><ul class="al">{#each atts as a (a.name)}<li><AttChip att={a} flat /></li>{/each}</ul></section>{/if}
        <section><h3>Filed</h3><dl><dt>Account</dt><dd>{t.account}</dd><dt>Labels</dt><dd>{t.labels.join(", ") || "None"}</dd><dt>Messages</dt><dd>{t.msgs.length}</dd></dl></section>
        {#if related.length}<section><h3>Related</h3><div class="rel">{#each related as r (r.id)}<ItemRow item={r} onopen={(i) => wins.openItem(i)} />{/each}</div></section>{/if}
      {:else if draft}
        <section><h3>Sending</h3><dl><dt>From</dt><dd>{draft.from === "Studio" ? "ada@studio-lindqvist.example" : "ada@lindqvist.example"}</dd><dt>Account</dt><dd>{draft.from}</dd><dt>Saved</dt><dd>{draft.saved || "Not yet"}</dd></dl></section>
        {#if draft.replyTo && THREADS[draft.replyTo]}<section><h3>Answering</h3><div class="rel"><ItemRow item={ITEMS[draft.replyTo]!} big onopen={(i) => wins.openItem(i)} /></div></section>{/if}
      {:else}
        <section><h3>Accounts</h3><dl><dt>Personal</dt><dd>ada@lindqvist.example</dd><dt>Studio</dt><dd>ada@studio-lindqvist.example</dd></dl></section>
        <section><h3>Storage</h3><dl><dt>Used</dt><dd>4.2 GB of 15 GB</dd><dt>Kept</dt><dd>6 years of mail</dd></dl></section>
      {/if}
</PanelFrame>

<style>
  .rp { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 0 var(--s-3); box-sizing: border-box; gap: var(--s-3); } header { flex: none; display: flex; align-items: center; padding: var(--s-5) var(--s-3) 0; } h2 { margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  .bd { flex: 1; min-height: 0; overflow-y: auto; scrollbar-width: none; display: grid; align-content: start; gap: var(--gap-before-head); padding: var(--s-4) var(--s-3); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); line-height: 1.2; font-weight: var(--strong-weight); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--s-3) var(--s-6); margin: 0; font-size: var(--text-ui); line-height: 1.3; } dt { color: var(--ink-3); } dd { margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-3); font-size: var(--text-ui); } li { display: flex; align-items: center; gap: var(--s-4); min-height: 28px; } .pn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .rl { color: var(--ink-3); font-size: var(--text-label); font-variant-numeric: tabular-nums; } .al { gap: var(--s-3); } .al li { min-height: 0; }
  .card { display: flex; align-items: center; gap: var(--s-5); width: 100%; padding: var(--s-4); margin: calc(var(--s-4) * -1); box-sizing: content-box; border-radius: var(--r-menu); text-align: start; color: var(--ink); transition: background-color var(--dur-fast) var(--ease-out); } @media (hover: hover) and (pointer: fine) { .card:not(:disabled):hover { background: var(--hover); } } .ct { display: grid; min-width: 0; } .ct strong { font-weight: var(--strong-weight); font-size: var(--text-ui); } .s { color: var(--ink-3); font-size: var(--text-label); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rel { display: grid; margin-inline-start: calc(var(--s-3) * -1); }
  .ai { display: grid; gap: var(--s-4); } .b { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-size: var(--text-ui); line-height: 1.45; } .sg { display: flex; flex-wrap: wrap; gap: var(--s-3); } .sg button { min-height: 30px; padding: 0 var(--s-5); border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent); font-size: var(--text-label); }
  .in input { width: 100%; box-sizing: border-box; min-height: 34px; padding: 0 var(--s-5); border-radius: var(--r-pill); border: 0; outline: 0; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font: inherit; font-size: var(--text-ui); } footer { flex: none; padding-bottom: var(--s-3); }
</style>
