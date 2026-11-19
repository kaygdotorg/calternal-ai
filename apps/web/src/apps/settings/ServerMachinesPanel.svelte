<script lang="ts">
  // The Machines page on the real server: your machines with their state, where threads run on each (sandboxed or direct: `machines.set`), stop sending work (drain) and revoke, and "Add a machine", which
  // makes a one-use enrollment code to give to that machine (`calternal machine enroll --server <this server> --code <code>`). Adding by SSH has no server operation, so that flow is not offered.
  import { Button, Selector } from "@calternal/ui";
  import { server } from "../../server/client";
  import { drainMachine, enrollmentCode, revokeMachine, setDirect } from "../../server/mutations";
  import { say } from "../../server/say";
  import { checkProvider, providerName, readiness, watchReadiness, type MachineState, type ProviderReadiness } from "../../server/readiness";
  import { spaces } from "../../shell/spaces.svelte";
  import { clockTime, copyText } from "../../lib/format";
  import "./panel.css";
  interface Machine { id: string; name: string; os: string; arch: string; direct: boolean; state: "online" | "draining" | "offline" | "revoked" }
  let loaded = $state(false), machines = $state<Machine[]>([]), error = $state(""), code = $state<{ code: string; expires_ms: number } | null>(null), busy = $state(false);
  const STATE: Record<string, string> = { online: "Online", draining: "Not taking new work", offline: "Offline", revoked: "Revoked" };
  const call = <T,>(path: string, body?: unknown): Promise<T> => server.json<T>(`/api/v1/machines${path}`, { body });
  async function load() { try { machines = (await call<{ machines: Machine[] }>("")).machines.filter((m) => m.state !== "revoked"); error = ""; loaded = true; } catch (e) { error = say(e); } }
  async function act(f: () => Promise<unknown>) { busy = true; try { await f(); error = ""; } catch (e) { error = say(e); } await load(); busy = false; }
  // Which providers are signed in on each machine (the readiness read, kept current by the account feed), with the provider's own sign-in command to copy and a Check again.
  let ready = $state<Record<string, MachineState>>({}), checking = $state<string | null>(null), copied = $state(""), silent = $state<Record<string, boolean>>({});
  async function loadReady() { try { ready = Object.fromEntries((await readiness(await spaces.whenCurrent())).machines.map((m) => [m.machine_id, m])); } catch { /* an older server has no readiness: no provider rows */ } }
  async function check(m: Machine, provider: string) { checking = `${m.id}/${provider}`; try { await checkProvider(spaces.current.id, m.id, provider); await loadReady(); silent[`${m.id}/${provider}`] = !ready[m.id]?.providers[provider]?.check; } catch (e) { error = say(e, "check the provider"); } checking = null; }
  async function copy(cmd: string) { if (await copyText(cmd)) { copied = cmd; setTimeout(() => (copied = ""), 2000); } else error = "Could not copy. Select the command and copy it."; }
  const standing = (p: ProviderReadiness) => !p.enabled ? "Off in this space" : p.ready ? "Signed in" : !p.check ? "Not checked yet" : !p.check.installed ? "Not installed on this machine" : !p.check.signed_in ? "Not signed in" : "Not ready";
  $effect(() => { void spaces.current.id; return watchReadiness(() => void loadReady()); });
  void load();
</script>

<div class="sp">
  {#each machines as m (m.id)}
    <div class="r" data-setting="machines.{m.id}">
      <span class="n"><span class="t">{m.name}</span><span class="h">{STATE[m.state] ?? m.state} · {m.os} {m.arch}{#if m.state === "offline"} · Not connected. Start the machine, or revoke it if it is gone for good.{/if}</span></span>
      <Selector class="sel-inline" label="Where threads run on {m.name}" options={[{ value: "sandboxed", label: "Sandboxed" }, { value: "direct", label: "Direct" }]} value={m.direct ? "direct" : "sandboxed"} onchange={(v) => act(() => setDirect(m.id, v === "direct"))} />
      <span class="a">
        <Button text={m.state === "draining" ? "Resume" : "Pause"} variant="quiet" disabled={busy} onclick={() => act(() => drainMachine(m.id, m.state !== "draining"))} />
        <Button text="Revoke" variant="quiet" disabled={busy} onclick={() => confirm(`Revoke ${m.name}? It can never connect again.`) && act(() => revokeMachine(m.id))} />
      </span>
    </div>
    {#each Object.entries(ready[m.id]?.providers ?? {}) as [pid, p] (pid)}
      {#if p}
      <div class="r sub" data-setting="machines.{m.id}.{pid}">
        <span class="n"><span class="t">{providerName(pid)}</span><span class="h">{standing(p)}{#if silent[`${m.id}/${pid}`] && !p.check}{" "}· No answer from the machine yet: it may be busy, or the provider's program is not installed there{/if}{#if p.check && p.check.installed && !p.check.signed_in}{" "}· Run this on {m.name}, then check again{/if}</span>
          {#if p.check && p.check.installed && !p.check.signed_in}<code class="cmd">{p.check.sign_in_command}</code>{/if}</span>
        <span class="a">{#if p.check && !p.check.signed_in && p.check.installed}<Button text={copied === p.check.sign_in_command ? "Copied" : "Copy command"} variant="secondary" onclick={() => copy(p.check!.sign_in_command)} />{/if}
          <Button text="Check again" variant="quiet" disabled={checking === `${m.id}/${pid}` || m.state === "offline"} onclick={() => check(m, pid)} /></span>
      </div>
      {/if}
    {/each}
  {:else}
    {#if loaded && !error}<div class="r"><span class="n"><span class="t">No machines yet</span><span class="h">A machine is a computer your threads run on: your own Mac or Linux box, or a cloud server. Add one to start a thread. Make a code here, then run the command below on that machine.</span></span></div>{/if}
  {/each}
  <div class="r"><span class="n"><span class="t">Add a machine</span><span class="h">{code ? `Valid until ${clockTime(code.expires_ms)}, one use.` : "Makes a one-use code for the machine to enroll with."}</span>{#if code}<code class="cmd">calternal machine enroll --server {location.origin} --code {code.code}</code>{/if}</span><Button class="settings-action" text={code ? "Make another code" : "Make a code"} variant={machines.length ? "secondary" : "primary"} disabled={busy} onclick={() => act(async () => { code = await enrollmentCode(); })} /></div>
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
