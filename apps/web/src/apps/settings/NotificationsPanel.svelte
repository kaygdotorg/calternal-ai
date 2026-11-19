<script lang="ts">
  // Settings > Notifications (docs/web-push.md): a notification when a thread of yours needs you or has finished, on this device and any other you choose. Opt-in: nothing is asked on load; "Turn on for this device" is the one
  // place the browser's permission prompt is shown. The list is the person's devices (this one marked), each removable; "Send a test" sends a notification to this device. Where push cannot work the page says why in plain words:
  // on an iPhone or iPad it needs the app installed to the Home Screen. What a notification says is only the thread's title and "needs you" or "finished" (never the message): the payload carries nothing else.
  import { Button } from "@calternal/ui";
  import { say } from "../../server/say";
  import { currentEnv, currentSubscription, pushClient, supportOf, turnOff, turnOn, type PushDevice } from "../../notify/push";
  import "./panel.css";
  const client = pushClient();
  const env = currentEnv();
  const unsupported = $derived(supportOf(env));
  let devices = $state<PushDevice[]>([]), loaded = $state(false), error = $state(""), note = $state(""), busy = $state(false);
  let permission = $state<NotificationPermission>(typeof Notification === "undefined" ? "default" : Notification.permission);
  /** Whether this device is on the list: this installation is registered and the browser still holds its subscription (a cleared browser leaves a stale row). */
  const here = $derived(devices.find((d) => d.current));
  async function load() { try { devices = (await client.devices()).devices; error = ""; } catch (e) { error = say(e); } loaded = true; }
  async function run(f: () => Promise<void>) { busy = true; error = ""; note = ""; try { await f(); } catch (e) { error = e instanceof Error ? e.message : say(e); } permission = typeof Notification === "undefined" ? "default" : Notification.permission; busy = false; }
  const on = () => run(async () => { devices = await turnOn(client); note = "Notifications are on for this device."; });
  const off = (d: PushDevice) => run(async () => { if (d.current) await turnOff(client, d.id); else await client.remove(d.id); await load(); });
  const test = () => run(async () => { await client.test(); note = "A test notification is on its way to this device."; });
  const until = (ms: number) => new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
  void load();
  void currentSubscription();
</script>

<div class="sp" data-setting="notifications.list">
  {#if unsupported}
    <div class="r"><span class="n"><span class="t">Notifications are not available here</span><span class="h" data-reason={unsupported.reason}>{unsupported.text}</span></span></div>
  {:else}
    <div class="r">
      <span class="n">
        <span class="t">This device</span>
        <span class="h">{here ? "Notifications are on. You get a notice when a thread of yours needs you or has finished." : permission === "denied" ? "Blocked for this site. Allow notifications in your browser's site settings, then turn them on here." : "Off. Turning them on asks your browser for permission."}</span>
      </span>
      <span class="a">
        {#if here}
          <Button text="Send a test" variant="quiet" disabled={busy} onclick={test} />
          <Button text="Turn off" variant="quiet" disabled={busy} onclick={() => off(here)} />
        {:else}
          <Button text="Turn on for this device" variant="primary" disabled={busy || permission === "denied"} onclick={on} />
        {/if}
      </span>
    </div>
    {#if error}<p class="e" role="alert">{error}</p>{/if}
    {#if note}<p class="h" role="status" style="padding: var(--s-3) var(--s-6)">{note}</p>{/if}
    {#if devices.filter((d) => !d.current).length}
      <h2>Your other devices</h2>
      {#each devices.filter((d) => !d.current) as d (d.id)}
        <div class="r" data-setting="notifications.{d.id}">
          <span class="n"><span class="t">{d.name}</span><span class="h">Receives notifications until {until(d.expiresAt)}, and for another 90 days each time it is used.</span></span>
          <span class="a"><Button text="Remove" variant="quiet" disabled={busy} onclick={() => off(d)} /></span>
        </div>
      {/each}
    {:else if loaded && !here}
      <p class="h" style="padding: var(--s-3) var(--s-6)">No devices receive notifications yet.</p>
    {/if}
  {/if}
</div>
