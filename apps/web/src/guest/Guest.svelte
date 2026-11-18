<script lang="ts">
  // The page a link opens (`/s/<secret>`): only the item, read-only, no sidebar and no sign-in. A note shows its text; a collection lists its members, each opening as a page of its own with a way back. "Open in Calternal" shows only when the server sends `item`, which it does only for a signed-in account that can open the item without the link. A link that does not work (wrong, revoked, expired or trashed) says only that, and what to do: never which.
  import { Button, Card } from "@calternal/ui";
  import NoteEditor from "../apps/notes/NoteEditor.svelte";
  import { sharing, type Guest } from "../apps/notes/share/sharing";
  import { href, route } from "../lib/route.svelte";
  import { say } from "../server/say";
  let item = $state<Guest | null>(null);
  let error = $state("");
  // Follows the address: a member's page, or Back, is another address of the same kind.
  $effect(() => {
    const path = route.path;
    item = null; error = "";
    void sharing.guest(path).then((i) => (item = i), (e) => { error = e?.code === "not_found" ? "gone" : say(e, "open this link"); });
  });
  const back = $derived(item && "back" in item ? item.back : undefined);
</script>

<main class="wrap">
  <header class="top">
    <p class="mark">Calternal</p>
    {#if item?.item}<Button variant="primary" text="Open in Calternal" onclick={() => location.assign(href(`/${item!.item!.kind}/${item!.item!.id}`))} />{/if}
  </header>
  <Card label={item ? item.title : "Shared link"} variant="solid" class="card">
    {#if item}
      {#if back}<p class="by"><a href={href(back)}>Back to the collection</a></p>{/if}
      {#if "members" in item}
        <h1>{item.title}</h1>
        <ul class="members">{#each item.members as m (m.href)}<li><a href={href(m.href)}>{m.title}</a></li>{/each}</ul>
      {:else}
        <div class="doc"><NoteEditor id={item.item?.id ?? route.path} text={item.markdown} label={item.title} readOnly onedit={() => {}} /></div>
      {/if}
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
  .by { margin: 0 0 var(--s-5); font-size: var(--text-label); color: var(--ink-3); }
  .by a { color: var(--ink-2); text-decoration: underline; }
  h1 { margin: 0 0 var(--s-4); font-size: var(--text-title); font-weight: var(--strong-weight); }
  .members { list-style: none; margin: 0 calc(var(--s-5) * -1); padding: 0; display: grid; gap: var(--s-1); }
  .members a { display: flex; align-items: center; min-height: var(--row-h); padding: 0 var(--s-5); border-radius: var(--r-select); font-size: var(--text-ui); color: var(--ink); }
  @media (hover: hover) and (pointer: fine) { .members a:hover { background: var(--hover); } }
  .lede { color: var(--ink-3); font-size: var(--text-ui); }
</style>
