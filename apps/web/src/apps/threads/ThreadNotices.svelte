<script lang="ts">
  // What needs attention on this thread, in plain words with the next step: no machine, a machine that is offline, a provider that is not signed in on the machine, a turn that failed
  // (and what Calternal does about it by itself), a turn waiting to retry, and a turn the machine dropped. One at a time, the most blocking first; nothing when nothing does.
  import { toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import Notice from "./Notice.svelte";
  import type { ThreadSummary } from "./model";
  let { thread, onresume }: { thread: ThreadSummary; onresume: () => void } = $props();
  const n = $derived(thread.notice);
  const machines = () => navigate("/settings/machines");
  const SIGNIN = "claude auth login";
  // The retry wait counts down while a notice shows it (one timer, only while shown).
  let now = $state(Date.now());
  $effect(() => {
    if (n?.kind !== "retrying") return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const left = $derived(n?.kind === "retrying" ? Math.max(0, Math.round((n.nextAt - now) / 1000)) : 0);
  const clock = $derived(`${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`);
</script>

{#if n?.kind === "no-machine"}
  <Notice icon="terminal" title="No machine to run this on yet" actions={[{ text: "Open Machines", onclick: machines, primary: true }]}>
    Agents work on a computer you add: yours, or one from the pool if your plan has it. Add one in Settings, then send your message again.
  </Notice>
{:else if n?.kind === "machine-offline"}
  <Notice icon="terminal" title="{n.machine} is offline" actions={[{ text: "Open Machines", onclick: machines, primary: true }]}>
    Your message is safe in this thread and will not run until the machine is back. Start it, or choose another machine under the message box. Nothing moves to another machine by itself.
  </Notice>
{:else if n?.kind === "provider-signin"}
  <Notice icon="key" title="{n.provider} is not signed in on {n.machine}" actions={[{ text: "Copy the command", onclick: () => void navigator.clipboard?.writeText(SIGNIN).then(() => toast("Command copied")), primary: true }]}>
    Sign in once on that machine, as the account that runs agents: run <code>{SIGNIN}</code> there. Calternal never sees the login. Then send your message again.
  </Notice>
{:else if n?.kind === "retrying"}
  <Notice icon="clock" title="Trying again in {clock}" actions={[{ text: "Try now", onclick: onresume, primary: true }]}>
    {n.reason}. Attempt {n.attempt} of {n.of}: Calternal retries by itself after 30 seconds, then 1, 2, 4 and 8 minutes. If all fail it keeps checking, or waits for your plan’s limit to reset.
  </Notice>
{:else if n?.kind === "failed"}
  <Notice icon="warning" title="The last turn failed" actions={[{ text: "Try again", onclick: onresume, primary: true }]}>
    {n.reason}. Calternal already retried by itself (five times, from 30 seconds up to 8 minutes). Try again now, or change the provider under the message box.
  </Notice>
{:else if n?.kind === "interrupted"}
  <Notice icon="warning" title="{n.machine} stopped answering" actions={[{ text: "Resume", onclick: onresume, primary: true }]}>
    The turn stopped at its last saved step and nothing moved to another machine. Resume continues it on {n.machine} if it is back, or on another one you choose.
  </Notice>
{/if}
