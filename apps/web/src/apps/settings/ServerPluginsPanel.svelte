<script lang="ts">
  // Which plugins are on in the current space, on the real server: one switch each (`plugins.enable` / `plugins.disable`). Turning one off or on needs the Manager role; a refusal is shown in words.
  import { Switch } from "@calternal/ui";
  import { server } from "../../server/client";
  import { say } from "../../server/say";
  import { spaces } from "../../shell/spaces.svelte";
  import "./panel.css";
  interface Plugin { enabled: boolean; manifest: { id: string; name?: string; description?: string } }
  let plugins = $state<Plugin[]>([]), error = $state("");
  const space = $derived(spaces.current.id);
  async function load() {
    try { plugins = (await server.operate<{ plugins: Plugin[] }>("plugins.list", { space_id: space })).plugins; error = ""; }
    catch (e) { error = say(e); }
  }
  async function set(p: Plugin, on: boolean) {
    try { await server.operate(on ? "plugins.enable" : "plugins.disable", { space_id: space, plugin_id: p.manifest.id }); error = ""; }
    catch (e) { error = say(e); }
    await load();
  }
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  {#each plugins as p (p.manifest.id)}
    <div class="r" data-setting="space.plugin.{p.manifest.id}"><Switch label={p.manifest.name ?? p.manifest.id} hint={p.manifest.description} checked={p.enabled} onchange={(v) => set(p, v)} /></div>
  {:else}
    {#if !error}<div class="r"><span class="n"><span class="t">No plugins registered</span><span class="h">A plugin adds an app, like Threads or Notes, to a space. The server lists the ones it has installed here.</span></span></div>{/if}
  {/each}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
