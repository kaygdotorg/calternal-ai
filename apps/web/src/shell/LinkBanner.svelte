<script lang="ts">
  // One quiet pill at the top when the line to the real server is not fine: "Reconnecting…" while it cannot be reached (the app keeps what it shows and retries), or "Your session ended" just before the page
  // goes to the sign-in screen at the same address. Nothing when all is well, and never on fixtures.
  import { Panel } from "@calternal/ui";
  import { link } from "../server/link.svelte";
</script>

{#if link.state !== "ok"}
  <Panel class="banner" radius="pill" dense role="status" aria-live="polite">{link.state === "expired" ? "Your session ended. Signing in again…" : "Can't reach the server. Reconnecting…"}</Panel>
{/if}

<style>
  /* The shared glass (Panel), placed at the top; it sets no background of its own. */
  :global(.banner) { position: fixed; z-index: 60; top: calc(var(--safe-top) + var(--s-4)); left: 50%; translate: -50% 0; max-width: calc(100vw - var(--s-8)); padding: var(--s-3) var(--s-6); color: var(--ink); font-size: var(--text-label); }
</style>
