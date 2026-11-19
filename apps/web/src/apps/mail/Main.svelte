<script lang="ts">
  // The Mail middle card: the open message, the account form, or a quiet empty state. The page owns its heading (the subject, marked for the shared header line); the body is a GenUI description.
  import { untrack } from "svelte";
  import type { Value } from "@calternal/genui";
  import { chromeTrack, IconButton } from "@calternal/ui";
  import GenUi from "../../genui/GenUi.svelte";
  import { shell } from "../../shell/shell.svelte";
  import { effects } from "./effects";
  import { mail } from "./store.svelte";
  import "../notes/column.css";

  let { id }: { id?: string } = $props();
  const setup = $derived(id === "setup");
  const uid = $derived(id && /^\d+$/.test(id) ? Number(id) : null);
  $effect(() => { void mail.start(); });
  // The address is the truth: opening `/mail/<uid>` (a click, Back, a reload) shows that message once the folder it is in has loaded.
  $effect(() => {
    const u = uid;
    if (u === null || !mail.ready || !mail.folder || mail.loading) return;
    if (untrack(() => mail.selected) !== u || (untrack(() => !mail.open) && !untrack(() => mail.messageError))) void mail.view(u);
  });
  let values = $state<Record<string, Value>>({});
  let epoch = $state(0);
  const source = $derived(setup ? mail.setupSource(mail.accounts.length > 0) : mail.messageSource);
  $effect(() => {
    const v = values;
    untrack(() => {
      if (typeof v.go === "string" && v.go) epoch++;
      mail.apply(v, effects);
    });
  });
  const title = $derived(setup ? "Add a mail account" : mail.open ? mail.open.header.subject?.trim() || "(no subject)" : "Mail");
</script>

<div class="scroll" use:chromeTrack>
  <header class="nc-head">
    {#if shell.narrow}<IconButton icon="sidebar" label="Show mail" onclick={() => shell.toggleSidebar()} />{/if}
  </header>
  <article class="nc-article">
    <h1 data-chrome-title>{title}</h1>
    {#if setup || uid !== null}
      {#if !setup && mail.messageError}<p class="err" role="alert">{mail.messageError}</p>{/if}
      {#if source}{#key `${epoch}\0${source}`}<GenUi {source} bind:values />{/key}
      {:else if !mail.messageError}<p class="lede" role="status">Opening the message…</p>{/if}
    {:else if mail.ready && !mail.accounts.length}
      <p class="lede">Add an account to read your mail.</p>
      <GenUi source={mail.setupSource(false)} bind:values />
    {:else}
      <p class="lede">{mail.error ?? "Choose a message from the list."}</p>
    {/if}
  </article>
</div>

<style>
  .scroll { position: absolute; inset: 0; overflow-y: auto; }
  h1 { font-size: var(--text-title); font-weight: var(--strong-weight); line-height: 1.2; letter-spacing: -0.01em; overflow-wrap: anywhere; }
  .lede { color: var(--ink-2); font-size: var(--text-ui); }
  .err { color: var(--danger); font-size: var(--text-label); }
</style>
