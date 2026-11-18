<script lang="ts">
  // Why a thread is in the Inbox, said by marks at the row's end (where All Threads shows the thread's state): one icon per reason, each with a tooltip and an accessible name, so an Inbox row is one line like every other row. The reasons are told apart by SHAPE
  // first (a question mark, a shield for an approval, a key for a secret, a lock for a sign-in, a warning triangle for a failure, a square for an interrupted turn) and by tone second (the failure is red, the rest are the ink), so they read in a glance and without colour.
  import { Icon, tip } from "@calternal/ui";
  import type { IconName } from "@calternal/ui";
  let { reasons }: { reasons: readonly string[] } = $props();
  const ICON: Record<string, IconName> = { Question: "question", Approval: "shield", Secret: "key", "Sign in": "lock", Failed: "warning", Interrupted: "stop" };
  const SAY: Record<string, string> = { Question: "Asks you a question", Approval: "Needs your approval", Secret: "Needs a secret", "Sign in": "Needs you to sign in", Failed: "Failed", Interrupted: "Interrupted: needs a decision" };
</script>

{#each reasons as r (r)}
  <span class="rm" class:bad={r === "Failed"} role="img" aria-label={SAY[r] ?? r} data-reason={r} use:tip={{ label: SAY[r] ?? r, side: "bottom" }}><Icon inline name={ICON[r] ?? "bell"} /></span>
{/each}

<style>
  .rm { display: inline-flex; align-items: center; color: var(--ink); }
  .rm.bad { color: var(--danger); }
  .rm[data-reason="Interrupted"] { color: var(--ink-2); }
</style>
