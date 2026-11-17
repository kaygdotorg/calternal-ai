<script lang="ts">
  // The Machines page on the real server: your machines with their state, where threads run on each (sandboxed or direct: `machines.set`), stop sending work (drain) and revoke, and "Add a machine", which
  // makes a one-use enrollment code to give to that machine (`calternal machine enroll --server <this server> --code <code>`). Adding by SSH has no server operation, so that flow is not offered.
  import { Button, Capsule } from "@calternal/ui";
  import { ServerError, server } from "../../server/client";
  import "./panel.css";
  interface Machine { id: string; name: string; os: string; arch: string; direct: boolean; state: "online" | "draining" | "offline" | "revoked" }
  let machines = $state<Machine[]>([]), error = $state(""), code = $state<{ code: string; expires_ms: number } | null>(null), busy = $state(false);
  const STATE: Record<string, string> = { online: "Online", draining: "Not taking new work", offline: "Offline", revoked: "Revoked" };
  async function call<T>(path: string, body?: unknown): Promise<T> {
    const res = await server.request(`/api/v1/machines${path}`, { method: body === undefined ? "GET" : "POST", headers: body === undefined ? {} : { "content-type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body) });
    const j = (await res.json().catch(() => ({}))) as { error?: { message?: string } } & T;
    if (!res.ok) throw new ServerError("internal", j.error?.message ?? `The server refused (${res.status}).`, res.status);
    return j;
  }
  async function load() { try { machines = (await call<{ machines: Machine[] }>("")).machines.filter((m) => m.state !== "revoked"); error = ""; } catch (e) { error = (e as Error).message === "Failed to fetch" ? "Can't reach the server." : (e as Error).message; } }
  async function act(f: () => Promise<unknown>) { busy = true; try { await f(); error = ""; } catch (e) { error = (e as Error).message === "Failed to fetch" ? "Can't reach the server." : (e as Error).message; } await load(); busy = false; }
  void load();
</script>

<div class="sp">
  {#each machines as m (m.id)}
    <div class="r" data-setting="machines.{m.id}">
      <span class="n"><span class="t">{m.name}</span><span class="h">{STATE[m.state] ?? m.state} · {m.os} {m.arch}</span></span>
      <Capsule label="Where threads run on {m.name}" kind="radio" glass={false} items={[{ id: "sandboxed", label: "Sandboxed" }, { id: "direct", label: "Direct" }]} value={m.direct ? "direct" : "sandboxed"} onchange={(v) => act(() => server.operate("machines.set", { machine_id: m.id, direct: v === "direct" }))} />
      <span class="a">
        <Button text={m.state === "draining" ? "Resume" : "Pause"} variant="quiet" disabled={busy} onclick={() => act(() => call(`/${m.id}/drain`, { draining: m.state !== "draining" }))} />
        <Button text="Revoke" variant="quiet" disabled={busy} onclick={() => confirm(`Revoke ${m.name}? It can never connect again.`) && act(() => call(`/${m.id}/revoke`, {}))} />
      </span>
    </div>
  {:else}
    {#if !error}<div class="r"><span class="h">No machines yet.</span></div>{/if}
  {/each}
  <div class="r"><span class="n"><span class="t">Add a machine</span><span class="h">{code ? `Code ${code.code}, valid until ${new Date(code.expires_ms).toLocaleTimeString()}. On the machine: calternal machine enroll --server ${location.origin} --code ${code.code}` : "Makes a one-use code for the machine to enroll with."}</span></span><Button text="Make a code" disabled={busy} onclick={() => act(async () => { code = await call<{ code: string; expires_ms: number }>("/codes", {}); })} /></div>
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
