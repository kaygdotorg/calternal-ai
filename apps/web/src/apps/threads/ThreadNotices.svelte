<script lang="ts">
  // What needs attention on this thread, in plain words with the next step: no machine, all machines busy, a machine that is offline, a provider that is not signed in on the machine, a turn that failed
  // (and which kind of failure), a turn waiting to retry (and by which rule), and a turn the machine dropped. One at a time; nothing when nothing does. The wording follows the server's `turn_reason`.
  import { toast } from "@calternal/ui";
  import { navigate } from "../../lib/route.svelte";
  import Notice from "./Notice.svelte";
  import type { ThreadSummary } from "./model";
  let { thread, onresume, oncheck }: { thread: ThreadSummary; onresume: () => void; oncheck?: () => void } = $props();
  const n = $derived(thread.notice);
  const machines = () => navigate("/settings/machines");
  // The retry wait counts down while a notice shows it (one timer, only while shown).
  let now = $state(Date.now());
  $effect(() => {
    if (n?.kind !== "retrying" || !n.nextAt) return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
  const left = $derived(n?.kind === "retrying" && n.nextAt ? Math.max(0, Math.round((n.nextAt - now) / 1000)) : 0);
  /** A wait as people say it: 2:05 under an hour, "3 hours" above. */
  const wait = (s: number) => (s >= 3600 ? `${Math.round(s / 3600)} hour${Math.round(s / 3600) === 1 ? "" : "s"}` : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`);
  const ago = (ms?: number) => { if (!ms) return ""; const m = Math.round((Date.now() - ms) / 60000); return m < 1 ? " Last heard from a moment ago." : m < 60 ? ` Last heard from ${m} minute${m === 1 ? "" : "s"} ago.` : ` Last heard from ${Math.round(m / 60)} hour${Math.round(m / 60) === 1 ? "" : "s"} ago.`; };
  const at = (ms?: number) => (ms ? new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "");
  const sentence = (s?: string) => (s ? `${s.replace(/\.$/, "")}. ` : "");
</script>

{#if n?.kind === "no-machine"}
  <Notice icon="terminal" title="No machine to run this on yet" actions={[{ text: "Open Machines", onclick: machines, primary: true }]}>
    Agents work on a computer you add: yours, or one from the pool if your plan has it. Add one in Settings, then send your message again.
  </Notice>
{:else if n?.kind === "waiting-capacity"}
  <Notice icon="clock" title="Waiting for a free machine">
    Every machine is busy{n.queuePosition ? `; you are number ${n.queuePosition} in the queue` : ""}. It starts by itself when one is free.
  </Notice>
{:else if n?.kind === "machine-offline"}
  <Notice icon="terminal" title={n.machine ? `${n.machine} is offline` : "Your machine is offline"} actions={[{ text: "Open Machines", onclick: machines, primary: true }]}>
    Your message is safe in this thread and will not run until the machine is back.{ago(n.sinceMs)} Start it, or choose another machine under the message box. Nothing moves to another machine by itself.
  </Notice>
{:else if n?.kind === "provider-signin"}
  <Notice icon="key" title={n.machine ? `${n.provider} is not signed in on ${n.machine}` : `${n.provider} is not signed in on the machine`} actions={[{ text: "Copy the command", onclick: () => void navigator.clipboard?.writeText(n.command ?? "claude auth login").then(() => toast("Command copied")), primary: true }, ...(oncheck ? [{ text: "Check again", onclick: oncheck }] : [])]}>
    Sign in once on that machine, as the account that runs agents: run <code>{n.command ?? "claude auth login"}</code> there. Calternal never sees the login. Then send your message again.
  </Notice>
{:else if n?.kind === "retrying"}
  {#if n.rule === "provider_backoff"}
    <Notice icon="clock" title={`Trying again in ${wait(left)}`} actions={[{ text: "Try now", onclick: onresume, primary: true }]}>
      {sentence(n.reason)}{n.attempt ? `Attempt ${n.attempt} of ${n.of ?? 5}: ` : ""}Calternal retries by itself after 30 seconds, then 1, 2, 4 and 8 minutes. If all fail it keeps checking, or waits for your plan’s limit to reset.
    </Notice>
  {:else if n.rule === "api_availability"}
    <Notice icon="clock" title={n.nextAt ? `Checking again in ${wait(left)}` : "Checking again later"} actions={[{ text: "Try now", onclick: onresume, primary: true }]}>
      {sentence(n.reason)}The retries are used up, so Calternal now checks every 3 hours whether the provider is back and continues when it is.
    </Notice>
  {:else}
    <Notice icon="clock" title={n.nextAt ? `Continues when your limit resets, about ${at(n.nextAt)}` : "Waiting for your plan’s limit to reset"} actions={[{ text: "Try now", onclick: onresume, primary: true }]}>
      {sentence(n.reason)}Nothing is lost: the turn resumes by itself when the limit resets.
    </Notice>
  {/if}
{:else if n?.kind === "failed"}
  {#if n.failure === "usage_limit"}
    <Notice icon="warning" title="Your plan’s limit is reached" actions={[{ text: "Try again", onclick: onresume, primary: true }]}>
      The provider will not take more work{n.resetAtMs ? ` until about ${at(n.resetAtMs)}` : " until the limit resets"}. Try again then, or change the provider under the message box.
    </Notice>
  {:else if n.failure === "permanent"}
    <Notice icon="warning" title="The last turn failed" actions={[{ text: "Try again", onclick: onresume, primary: true }]}>
      {sentence(n.reason)}Retrying by itself would not help, so Calternal stopped. Check the machine and the provider’s sign-in, then try again, or change the provider under the message box.
    </Notice>
  {:else}
    <Notice icon="warning" title="The last turn failed" actions={[{ text: "Try again", onclick: onresume, primary: true }]}>
      {sentence(n.reason)}Calternal already retried by itself (five times, from 30 seconds up to 8 minutes). Try again now, or change the provider under the message box.
    </Notice>
  {/if}
{:else if n?.kind === "interrupted"}
  {#if n.resumable}
    <Notice icon="warning" title={n.machine ? `${n.machine} stopped answering` : "The machine stopped answering"} actions={[{ text: "Resume", onclick: onresume, primary: true }]}>
      The turn stopped at its last saved step and nothing moved to another machine.{ago(n.sinceMs)} Resume continues it on {n.machine ?? "that machine"} if it is back, or on another one you choose.
    </Notice>
  {:else}
    <Notice icon="warning" title="The turn was interrupted">
      {sentence(n.why ?? "It is not known whether the last step finished")}It cannot be resumed safely, so Calternal has not tried. Check the thread’s files, then send your message again.
    </Notice>
  {/if}
{/if}
