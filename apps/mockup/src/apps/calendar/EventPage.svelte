<script lang="ts">
  // One event, as a page: the calendar it is on, the title, the day and time in words, then what a person needs in the order they need it: where (with a map stand-in), who is coming and what each said, how it repeats, when it reminds, the notes, and what it is
  // about elsewhere (a mail, a chat), drawn with those kinds' own rows. "Going, Maybe, Can't" answers an invitation. A guest opens their contact page beside. Past events are quiet. Fixture only.
  import { Badge, IconButton, Selector } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import AppIcon from "../../shell/AppIcon.svelte";
  import Glyph from "../../shell/AppIcon.svelte";
  import ItemRow from "../../shell/Item.svelte";
  import { ITEMS, type Item } from "../../data/collections";
  import { wins } from "../../windows.svelte";
  import { calOf, EV, rsvp, type Rsvp } from "./data";
  import { longDate, mins, NOW, relative, TODAY } from "./dates";
  import { personByName } from "../contacts/people";
  let { item, onback, backLabel = "Calendar" }: { item: Item; onback?: () => void; backLabel?: string } = $props();
  const ev = $derived(EV[item.id]), cal = $derived(ev ? calOf(ev.cal) : undefined);
  const first = $derived(ev?.days[0] ?? TODAY), past = $derived(!!ev && (ev.days.at(-1)! < TODAY || (ev.days.at(-1) === TODAY && !!ev.end && mins(ev.end) < mins(NOW))));
  let mine = $state<"going" | "maybe" | "declined" | undefined>(), seen = $state("");
  $effect(() => { if (seen !== item.id) { seen = item.id; mine = undefined; } });
  const TONE: Record<Rsvp, "good" | "warn" | "bad" | "neutral"> = { going: "good", maybe: "warn", declined: "bad", waiting: "neutral" }, WORD: Record<Rsvp, string> = { going: "Going", maybe: "Maybe", declined: "Can't go", waiting: "No answer" };
  const counts = $derived(ev ? ev.guests.reduce<Record<string, number>>((c, _, i) => { const r = rsvp(ev, i); c[r] = (c[r] ?? 0) + 1; return c; }, {}) : {});
  const hasMap = $derived(!!ev?.where && /[0-9]|,/.test(ev.where));
  const related = $derived((ev?.related ?? []).map((id) => ITEMS[id]).filter((x): x is Item => !!x));
  const range = $derived(!ev ? "" : ev.start ? `${ev.start} – ${ev.end}` : ev.days.length > 1 ? `All day, ${ev.days.length} days` : "All day");
  const open = (n: string) => { const p = personByName(n); if (p) wins.beside("item", p.id); };
</script>

<article class="ep" class:past data-event-page={item.id}>
  {#if onback}<button type="button" class="back" onclick={onback}><AppIcon name="back" inline /><span>{backLabel}</span></button>{/if}
  {#if !ev || !cal}
    <h1>{item.title}</h1><p class="m">{item.sub}</p>
  {:else}
    <p class="cl"><i style:--hue={cal.hue}></i>{cal.name}{#if ev.cal === "birthdays"}<span class="gf"><Glyph name="gift" inline /></span>{/if}{#if ev.repeat}<span class="rp">· {ev.repeat}</span>{/if}</p>
    <div class="tt"><h1>{ev.title}</h1>
      <div class="acts" role="toolbar" aria-label="Event"><IconButton icon="edit" label="Edit" /><IconButton icon="share" label="Share" /><IconButton icon="trash" label="Delete" /></div></div>
    <p class="when"><strong>{longDate(first)}{ev.days.length > 1 ? ` to ${longDate(ev.days.at(-1)!)}` : ""}</strong><span>{range}{["Today", "Tomorrow", "Yesterday"].includes(relative(first)) ? ` · ${relative(first)}` : ""}</span></p>
    {#if ev.guests.length && ev.cal !== "birthdays" && !past}
      <div class="rs"><Selector options={[{ value: "going", label: "Going" }, { value: "maybe", label: "Maybe" }, { value: "declined", label: "Can't go" }]} value={mine ?? "going"} label="Your answer" size="sm" fill={false} onchange={(v: string) => (mine = v as typeof mine)} /></div>
    {/if}
    <dl class="f">
      {#if ev.where}<div><dt><Glyph name="mappin" inline /><span class="sr">Where</span></dt><dd><span>{ev.where}</span>{#if hasMap}<span class="map" style:--hue={cal.hue} role="img" aria-label="Map of the place (fixture)"><Glyph name="pin" /></span>{/if}</dd></div>{/if}
      {#if ev.alert}<div><dt><AppIcon name="bell" inline /><span class="sr">Reminder</span></dt><dd>{ev.alert}</dd></div>{/if}
      {#if ev.guests.length}
        <div><dt><AppIcon name="user" inline /><span class="sr">Guests</span></dt>
          <dd><span class="sum">{ev.guests.length} {ev.guests.length === 1 ? "guest" : "guests"}{ev.cal !== "birthdays" ? `: ${(["going", "maybe", "declined", "waiting"] as const).filter((k) => counts[k]).map((k) => [k, counts[k]!] as const).map(([k, v]) => `${v} ${WORD[k as Rsvp].toLowerCase()}`).join(", ")}` : ""}</span>
            <ul class="gl">{#each ev.guests as g, i (g)}<li><button type="button" class="g" disabled={!personByName(g)} onclick={() => open(g)}><Av name={g} size="28px" /><span class="gn">{g}</span></button>{#if ev.cal !== "birthdays"}<Badge text={WORD[rsvp(ev, i)]} tone={TONE[rsvp(ev, i)]} />{/if}</li>{/each}</ul></dd></div>
      {/if}
      {#if ev.notes}<div><dt><AppIcon name="notes" inline /><span class="sr">Notes</span></dt><dd class="notes">{ev.notes}</dd></div>{/if}
    </dl>
    {#if related.length}<section class="rel"><h3>About this</h3>{#each related as r (r.id)}<ItemRow item={r} big onopen={(i) => wins.openItem(i)} />{/each}</section>{/if}
  {/if}
</article>

<style>
  .ep { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; container-type: inline-size; display: block; max-width: 40rem; padding-bottom: var(--s-9); } .ep.past h1, .ep.past .when { color: var(--ink-2); }
  .gf { display: inline-grid; color: var(--ink-3); }
  .back { display: inline-flex; align-items: center; gap: var(--s-2); margin: 0 0 var(--s-4) calc(var(--s-3) * -1); padding: var(--s-2) var(--s-4) var(--s-2) var(--s-3); border-radius: var(--r-pill); color: var(--accent); font-size: var(--text-label); } @media (hover: hover) and (pointer: fine) { .back:hover { background: var(--hover); } }
  .cl { display: flex; align-items: center; gap: var(--s-3); margin: 0; color: var(--ink-3); font-size: var(--text-label); } .cl i { width: 10px; height: 10px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); } .rp { color: var(--ink-3); }
  .tt { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--s-5); margin-top: var(--s-3); } h1 { flex: 1; min-width: 0; margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; text-wrap: balance; overflow-wrap: anywhere; } .acts { flex: none; display: flex; gap: var(--s-1); }
  .when { display: grid; gap: var(--s-1); margin: var(--s-3) 0 0; font-size: var(--text-ui); line-height: 1.4; } .when strong { font-weight: var(--strong-weight); } .when span { color: var(--ink-3); font-variant-numeric: tabular-nums; } .rs { margin-top: var(--s-6); } .m { color: var(--ink-3); }
  .f { display: grid; gap: var(--s-6); margin: var(--s-7) 0 0; } .f > div { display: grid; grid-template-columns: 1.75rem minmax(0, 1fr); gap: var(--s-4); align-items: start; } dt { display: grid; justify-items: start; padding-top: 2px; color: var(--ink-3); } dd { margin: 0; display: grid; gap: var(--s-4); font-size: var(--text-ui); line-height: 1.45; min-width: 0; overflow-wrap: anywhere; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); } .map { display: grid; place-items: center; aspect-ratio: 16 / 5; max-width: 26rem; border-radius: var(--r-menu); background: oklch(0.8 0.06 var(--hue) / 0.35); color: var(--ink-2); }
  .sum { color: var(--ink-3); font-size: var(--text-label); } .gl { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-2); } .gl li { display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); min-height: 36px; } .g { display: flex; align-items: center; gap: var(--s-4); min-width: 0; margin-inline-start: calc(var(--s-3) * -1); padding: var(--s-1) var(--s-3); border-radius: var(--r-select); color: var(--ink); text-align: start; } .g:not(:disabled) { cursor: pointer; } @media (hover: hover) and (pointer: fine) { .g:not(:disabled):hover { background: var(--hover); } } .gn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .notes { white-space: pre-line; color: var(--ink-2); } .rel { margin-top: var(--gap-before-head); } h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.2; }
  @container (max-width: 28rem) { .tt { flex-direction: column; gap: var(--s-2); } .acts { margin-inline-start: calc(var(--s-3) * -1); } }
</style>
