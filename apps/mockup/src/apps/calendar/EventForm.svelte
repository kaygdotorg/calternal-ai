<script lang="ts">
  // A new event, as a static form in a window beside: the title first and large, the calendar as four pills (each with its colour), All-day, when it starts and ends, where, guests, a reminder, how it repeats, notes; Add and Cancel at the foot. Nothing is saved:
  // Add closes the window. Fields are one line each with a hairline between, as in Mail's compose. MOTION: the form fades in and rises over `--dur-base` once the window has mostly grown (motion.ts `arrive`); the guest chips and the all-day times unfold. Fixture only.
  import { onMount, tick } from "svelte";
  import { Button, IconButton, Switch } from "@calternal/ui";
  import Av from "../../shell/Av.svelte";
  import type { Item } from "../../data/collections";
  import { wins, type Win } from "../../windows.svelte";
  import { arrive, unfold } from "../mail/motion";
  import { CONTACT_ITEMS, PEOPLE } from "../contacts/people";
  import { CALS } from "./data";
  let { item, win }: { item: Item; win?: Win } = $props();
  const wid = $derived(win?.id ?? wins.list.find((w) => w.kind === "item" && w.ref === item.id)?.id);
  let title = $state(""), cal = $state("personal"), all = $state(false), day = $state(item.date ?? "2026-11-20"), from = $state("10:00"), to = $state("11:00"), where = $state(""), guests = $state<string[]>([]), typed = $state(""), alert = $state("10 minutes before"), repeat = $state("Never"), notes = $state("");
  let titleEl = $state<HTMLInputElement>();
  onMount(() => { const w = setTimeout(async () => { await tick(); titleEl?.focus({ preventScroll: true }); }, 260); return () => clearTimeout(w); });
  const add = () => { const v = typed.trim().replace(/,$/, ""); if (v && !guests.includes(v)) guests = [...guests, v]; typed = ""; };
  const names = CONTACT_ITEMS.filter((c) => !PEOPLE[c.id]!.me).map((c) => c.title);
</script>

<div class="ef" data-event-form in:arrive={{ after: 0.55 }}>
  <div class="form">
    <input class="title" bind:this={titleEl} bind:value={title} placeholder="New event" aria-label="Title" />
    <div class="f"><span class="l" id="cal-l">Calendar</span><div class="v pills" role="radiogroup" aria-labelledby="cal-l">{#each CALS as c (c.id)}<button type="button" role="radio" aria-checked={cal === c.id} class="pill" class:on={cal === c.id} onclick={() => (cal = c.id)}><i style:--hue={c.hue}></i>{c.name}</button>{/each}</div></div>
    <div class="f sw"><Switch label="All day" checked={all} onchange={(v: boolean) => (all = v)} /></div>
    <div class="f"><label class="l" for="d">Date</label><input id="d" class="in" type="date" bind:value={day} /></div>
    {#if !all}<div class="f" transition:unfold><label class="l" for="s">Starts</label><input id="s" class="in" type="time" bind:value={from} /></div>
    <div class="f" transition:unfold><label class="l" for="e">Ends</label><input id="e" class="in" type="time" bind:value={to} /></div>{/if}
    <div class="f"><label class="l" for="w">Where</label><input id="w" class="in" bind:value={where} placeholder="A place or a video link" /></div>
    <div class="f top"><label class="l" for="g">Guests</label>
      <div class="v chips">{#each guests as g, i (g)}<span class="chip" transition:unfold><Av name={g} size="20px" /><span class="pn">{g}</span><IconButton small icon="close" label={`Remove ${g}`} onclick={() => (guests = guests.filter((_, k) => k !== i))} /></span>{/each}
        <input id="g" class="in" list="people" bind:value={typed} placeholder={guests.length ? "" : "Names or addresses"} onkeydown={(e) => { if (e.key === "Enter" || e.key === ",") { e.preventDefault(); add(); } }} onblur={add} /><datalist id="people">{#each names as n (n)}<option value={n}></option>{/each}</datalist></div></div>
    <div class="f"><label class="l" for="a">Reminder</label><select id="a" class="in" bind:value={alert}>{#each ["None", "At the time", "10 minutes before", "30 minutes before", "1 hour before", "1 day before"] as o (o)}<option>{o}</option>{/each}</select></div>
    <div class="f"><label class="l" for="r">Repeats</label><select id="r" class="in" bind:value={repeat}>{#each ["Never", "Every day", "Every weekday", "Every week", "Every month", "Every year"] as o (o)}<option>{o}</option>{/each}</select></div>
    <div class="f top"><label class="l" for="n">Notes</label><textarea id="n" class="in notes" bind:value={notes} placeholder="Anything to remember"></textarea></div>
  </div>
  <footer class="bar"><Button text="Add" variant="primary" disabled={!title.trim()} onclick={() => wid && wins.close(wid)} /><Button text="Cancel" variant="quiet" onclick={() => wid && wins.close(wid)} /></footer>
</div>

<style>
  .ef { flex: 1; min-height: 0; display: flex; flex-direction: column; container-type: inline-size; } .form { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; max-width: 40rem; width: 100%; }
  .title { display: block; width: 100%; box-sizing: border-box; padding: var(--s-2) 0 var(--s-4); border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; } .title::placeholder, .in::placeholder { color: var(--ink-3); }
  .f { display: flex; align-items: center; gap: var(--s-5); min-height: var(--row-h); border-top: 1px solid rgb(var(--ink-rgb) / 0.09); } .f.top { align-items: flex-start; } .f.sw { padding-inline-end: 0; } .f.sw :global(.sw) { flex: 1; }
  .l { flex: none; width: 4.5rem; color: var(--ink-3); font-size: var(--text-label); } .top .l { padding-top: var(--s-5); }
  .in { flex: 1; min-width: 0; min-height: 32px; padding: 0; border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: var(--text-ui); color-scheme: light dark; } select.in { cursor: pointer; } .notes { min-height: 6rem; padding: var(--s-4) 0; resize: none; field-sizing: content; line-height: 1.5; }
  input[list]::-webkit-calendar-picker-indicator { display: none !important; }
  .v { flex: 1; min-width: 0; } .pills { display: flex; flex-wrap: wrap; gap: var(--s-3); padding: var(--s-3) 0; } .pill { display: inline-flex; align-items: center; gap: var(--s-3); min-height: 30px; padding: 0 var(--s-5) 0 var(--s-4); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink); font-size: var(--text-label); transition: background-color var(--dur-fast) var(--ease-out), scale var(--dur-fast) var(--ease-out); } .pill:active { scale: var(--press-scale); } .pill.on { background: var(--accent-soft); color: var(--accent); }
  .pill i { width: 10px; height: 10px; border-radius: 50%; background: oklch(0.7 0.12 var(--hue)); } .chips { display: flex; flex-wrap: wrap; align-items: center; gap: var(--s-2) var(--s-3); padding: var(--s-3) 0; } .chip { display: inline-flex; align-items: center; gap: var(--s-3); min-height: 28px; padding: 0 var(--s-2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); } .pn { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .chips .in { flex: 1 1 6rem; min-width: 4rem; }
  .bar { flex: none; display: flex; gap: var(--s-3); padding-top: var(--s-4); }
</style>
