<script lang="ts">
  // The page a link opens (`/s/<secret>`): only the item, read-only, no sidebar and no sign-in. Someone who is signed in gets "Open in Calternal" to see it in their own app. A secret that does not work
  // (wrong, revoked or expired) says only that, and what to do: it never says which.
  import { Avatar, Button, Card } from "@calternal/ui";
  import NoteEditor from "../apps/notes/NoteEditor.svelte";
  import { sharing, type GuestItem } from "../apps/notes/share/sharing";
  import { href } from "../lib/route.svelte";
  import { serverMode } from "../server/client";
  import { say } from "../server/say";
  let { secret }: { secret: string } = $props();
  let item = $state<GuestItem | null>(null);
  let error = $state("");
  let signedIn = $state(false);
  $effect(() => {
    void sharing.guest_read(secret).then((i) => (item = i), (e) => { error = e?.code === "not_found" ? "gone" : say(e, "open this link"); });
    if (serverMode()) void import("../server/auth").then((a) => a.status()).then((s) => (signedIn = s.signed_in), () => {});
    else signedIn = new URLSearchParams(location.search).has("signedin");
  });
</script>

<main class="wrap">
  <header class="top">
    <p class="mark">Calternal</p>
    {#if item && signedIn}<Button variant="primary" text="Open in Calternal" onclick={() => location.assign(href(`/${item!.item.kind}/${item!.item.id}`))} />{/if}
  </header>
  <Card label={item ? item.title : "Shared link"} variant="solid" class="card">
    {#if item}
      <p class="by"><Avatar name={item.shared_by} /><span>Shared by {item.shared_by}. You can read this; you can't change it.</span></p>
      <div class="doc"><NoteEditor id={item.item.id} text={item.markdown} label={item.title} readOnly onedit={() => {}} /></div>
    {:else if error}
      <h1>{error === "gone" ? "This link doesn't work" : "Couldn't open this link"}</h1>
      <p class="lede">{error === "gone" ? "It may have been turned off, or it ran out. Ask whoever sent it for a new one." : error}</p>
    {:else}<p class="lede">Opening…</p>{/if}
  </Card>
</main>

<style>
  .wrap { min-height: 100dvh; box-sizing: border-box; display: grid; grid-template-rows: auto 1fr; gap: var(--s-4); padding: calc(var(--safe-top, 0px) + var(--inset)) var(--inset) calc(var(--safe-bottom, 0px) + var(--inset)); }
  .top { display: flex; align-items: center; justify-content: space-between; gap: var(--s-4); padding: 0 var(--s-3); min-height: var(--pill-h); }
  .mark { margin: 0; font-size: var(--text-heading); font-weight: var(--strong-weight); font-family: var(--font-rounded, inherit); font-variation-settings: "ROND" 100; }
  .wrap :global(.card) { width: min(46rem, 100%); justify-self: center; box-sizing: border-box; padding: var(--s-6) var(--s-7); }
  .by { display: flex; align-items: center; gap: var(--s-3); margin: 0 0 var(--s-5); font-size: var(--text-label); color: var(--ink-3); }
  h1 { margin: 0; font-size: var(--text-title); font-weight: var(--strong-weight); }
  .lede { color: var(--ink-3); font-size: var(--text-ui); }
</style>
