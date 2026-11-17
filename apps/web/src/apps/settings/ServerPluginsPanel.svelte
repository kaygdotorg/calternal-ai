<script lang="ts">
  // Which plugins are on in the current space, on the real server: one switch each (`plugins.enable` / `plugins.disable`). Turning one off or on needs the Manager role; a refusal is shown in words.
  import { Switch } from "@calternal/ui";
  import { ServerError, server } from "../../server/client";
  import { spaces } from "../../shell/spaces.svelte";
  import "./panel.css";
  interface Plugin { enabled: boolean; manifest: { id: string; name?: string; description?: string } }
  let plugins = $state<Plugin[]>([]), error = $state("");
  const space = $derived(spaces.current.id);
  async function load() {
    try { plugins = (await server.operate<{ plugins: Plugin[] }>("plugins.list", { space_id: space })).plugins; error = ""; }
    catch (e) { error = e instanceof ServerError ? e.message : "Could not load the plugins."; }
  }
  async function set(p: Plugin, on: boolean) {
    try { await server.operate(on ? "plugins.enable" : "plugins.disable", { space_id: space, plugin_id: p.manifest.id }); error = ""; }
    catch (e) { error = e instanceof ServerError ? e.message : "That did not work."; }
    await load();
  }
  $effect(() => { void space; void load(); });
</script>

<div class="sp">
  {#each plugins as p (p.manifest.id)}
    <div class="r" data-setting="space.plugin.{p.manifest.id}"><Switch label={p.manifest.name ?? p.manifest.id} hint={p.manifest.description} checked={p.enabled} onchange={(v) => set(p, v)} /></div>
  {:else}
    {#if !error}<div class="r"><span class="h">No plugins are registered.</span></div>{/if}
  {/each}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
