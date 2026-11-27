<script lang="ts">
  // A new contact, as a static form: the picture (initials until there is one) with "Add a photo", the name and company, then the same grouped blocks the contact page shows (phone, email, address, birthday, notes), each empty with its "add" row so the shape of a finished card is visible.
  // Nothing is saved: Save closes the window. MOTION: the form fades in and rises over `--dur-base` once the window has mostly grown (motion.ts `arrive`); a field added unfolds. Fixture only.
  import { onMount, tick } from "svelte";
  import { Button } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import Av from "../../shell/Av.svelte";
  import type { Item } from "../../data/collections";
  import { wins, type Win } from "../../windows.svelte";
  import { arrive, unfold } from "../mail/motion";
  let { item, win }: { item: Item; win?: Win } = $props();
  const wid = $derived(win?.id ?? wins.list.find((w) => w.kind === "item" && w.ref === item.id)?.id);
  let first = $state(""), last = $state(""), org = $state(""), phones = $state<string[]>([""]), emails = $state<string[]>([""]), street = $state(""), city = $state(""), birthday = $state(""), notes = $state(""), firstEl = $state<HTMLInputElement>();
  onMount(() => { const w = setTimeout(async () => { await tick(); firstEl?.focus({ preventScroll: true }); }, 260); return () => clearTimeout(w); });
  const name = $derived(`${first} ${last}`.trim());
</script>

<div class="cf" data-contact-form in:arrive={{ after: 0.55 }}>
  <div class="form">
    <header class="hero"><Av name={name} size="5.5rem" /><div class="id"><input bind:this={firstEl} bind:value={first} class="nm" placeholder="First name" aria-label="First name" autocomplete="off" /><input bind:value={last} class="nm" placeholder="Last name" aria-label="Last name" autocomplete="off" /><input bind:value={org} class="co" placeholder="Company" aria-label="Company" autocomplete="off" /></div></header>
    <button type="button" class="photo">Add a photo</button>
    <div class="grp">
      {#each phones as _, i (i)}<div class="r" transition:unfold><span class="k">{i ? "other" : "mobile"}</span><input type="tel" bind:value={phones[i]} placeholder="Phone number" aria-label="Phone number" /></div>{/each}
      <button type="button" class="add" onclick={() => (phones = [...phones, ""])}><AppIcon name="plus" inline />Add a phone number</button>
    </div>
    <div class="grp">
      {#each emails as _, i (i)}<div class="r" transition:unfold><span class="k">{i ? "other" : "home"}</span><input type="email" bind:value={emails[i]} placeholder="Email address" aria-label="Email address" /></div>{/each}
      <button type="button" class="add" onclick={() => (emails = [...emails, ""])}><AppIcon name="plus" inline />Add an email address</button>
    </div>
    <div class="grp"><div class="r"><span class="k">address</span><input bind:value={street} placeholder="Street and number" aria-label="Street and number" /></div><div class="r"><span class="k"></span><input bind:value={city} placeholder="Postcode and city" aria-label="Postcode and city" /></div></div>
    <div class="grp"><div class="r"><label class="k" for="bd">birthday</label><input id="bd" type="date" bind:value={birthday} /></div></div>
    <div class="grp"><div class="r top"><label class="k" for="nt">notes</label><textarea id="nt" bind:value={notes} placeholder="Anything to remember"></textarea></div></div>
  </div>
  <footer class="bar"><Button text="Save" variant="primary" disabled={!name} onclick={() => wid && wins.close(wid)} /><Button text="Cancel" variant="quiet" onclick={() => wid && wins.close(wid)} /></footer>
</div>

<style>
  .cf { flex: 1; min-height: 0; display: flex; flex-direction: column; container-type: inline-size; } .form { flex: 1; min-height: 0; overflow: auto; scrollbar-width: thin; max-width: 36rem; width: 100%; }
  .hero { display: flex; align-items: center; gap: var(--s-6); padding: var(--s-3) 0 var(--s-2); } .id { flex: 1; min-width: 0; display: grid; gap: var(--s-1); } input, textarea { min-width: 0; border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; color-scheme: light dark; } input::placeholder, textarea::placeholder { color: var(--ink-3); }
  .nm { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.25; padding: 0; } .co { font-size: var(--text-ui); padding: var(--s-1) 0; }
  .photo { margin: 0 0 var(--s-6); padding: var(--s-2) 0; color: var(--accent); font-size: var(--text-label); }
  .grp { display: grid; margin: 0 0 var(--s-5); border-radius: var(--r-menu); background: rgb(var(--ink-rgb) / 0.05); overflow: hidden; } .grp > * + * { border-top: 1px solid rgb(var(--ink-rgb) / 0.08); }
  .r { display: grid; gap: var(--s-1); padding: var(--s-4) var(--s-6); } .k { color: var(--ink-3); font-size: var(--text-label); text-transform: capitalize; } .r input, .r textarea { font-size: var(--text-ui); line-height: 1.45; padding: 0; } textarea { min-height: 4.5rem; resize: none; field-sizing: content; }
  .add { display: flex; align-items: center; gap: var(--s-4); min-height: var(--row-h); padding: 0 var(--s-6); color: var(--accent); font-size: var(--text-ui); text-align: start; } @media (hover: hover) and (pointer: fine) { .add:hover { background: rgb(var(--ink-rgb) / 0.04); } }
  .bar { flex: none; display: flex; gap: var(--s-3); padding-top: var(--s-4); }
</style>
