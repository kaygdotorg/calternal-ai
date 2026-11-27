<script lang="ts">
  // The Mail window: the open message (or draft) if there is one, else a quiet "nothing open" page that says what is waiting and offers Write. The lists are the sidebar's collections; nothing is repeated here.
  import { Button } from "@calternal/ui";
  import AppIcon from "../../shell/AppIcon.svelte";
  import { ITEMS } from "../../data/collections";
  import type { PageProps } from "../registry";
  import { startDraft } from "./compose";
  import { COLLS } from "../../data/collections";
  import MailItem from "./MailItem.svelte";
  import MailLook from "./MailLook.svelte";
  import { wins } from "../../windows.svelte";
  let { win }: PageProps = $props();
  const item = $derived(win.sel ? ITEMS[win.sel] : undefined);
  const unread = (id: string) => COLLS[id]?.items.filter((i) => i.unread).length ?? 0;
</script>

{#if item}
  {#key item.id}<MailItem {item} {win} />{/key}
{:else}
  <div class="empty" data-mail-empty>
    <span class="g"><AppIcon name="mail" /></span>
    <h2>No message open</h2>
    <p>{unread("mail-inbox")} unread in Inbox: {unread("mail-personal")} in Personal and {unread("mail-studio")} in Studio. Choose a message in the sidebar to read it here.</p>
    <Button text="Write" icon="edit" variant="primary" onclick={() => startDraft("new")} />
    <section class="pick"><h3>Pick up where you left off</h3><div class="cards">{#each (COLLS["mail-flagged"]?.items ?? []).slice(0, 3) as it (it.id)}<MailLook item={it} mode="card" onopen={(i) => wins.openItem(i)} />{/each}</div></section>
  </div>
{/if}

<style>
  .empty { flex: 1; min-height: 0; display: grid; place-content: center; justify-items: center; gap: var(--s-4); max-width: 44rem; margin: 0 auto; text-align: center; }
  .g { display: grid; place-items: center; width: 56px; height: 56px; border-radius: 50%; background: rgb(var(--ink-rgb) / 0.06); color: var(--ink-3); } h2 { margin: var(--s-3) 0 0; font-size: var(--text-heading); font-weight: var(--strong-weight); } p { margin: 0 0 var(--s-3); max-width: 26rem; color: var(--ink-3); font-size: var(--text-ui); line-height: 1.5; text-wrap: balance; }
  .pick { justify-self: stretch; margin-top: var(--gap-before-head); text-align: start; } .pick h3 { margin: 0 0 var(--gap-head-items); color: var(--ink-2); font-size: var(--text-heading); font-weight: var(--strong-weight); } .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr)); gap: var(--s-4); }
</style>
