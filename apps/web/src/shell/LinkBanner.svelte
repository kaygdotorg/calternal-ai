<script lang="ts">
  // One quiet pill at the top when the line to the real server is not fine: "Reconnecting…" while it cannot be reached (the app keeps what it shows and retries), or "Your session ended" just before the page
  // goes to the sign-in screen at the same address. Nothing when all is well, and never on fixtures.
  import { link } from "../server/link.svelte";
</script>

{#if link.state !== "ok"}
  <div class="banner" role="status" aria-live="polite">{link.state === "expired" ? "Your session ended. Signing in again…" : "Can't reach the server. Reconnecting…"}</div>
{/if}

<style>
  .banner { position: fixed; z-index: 60; top: calc(env(safe-area-inset-top) + var(--s-4)); left: 50%; translate: -50% 0; max-width: calc(100vw - var(--s-8)); padding: var(--s-3) var(--s-6); border-radius: var(--r-pill); background: var(--paper); color: var(--ink); box-shadow: var(--shadow-card); font-size: var(--text-label); }
</style>
