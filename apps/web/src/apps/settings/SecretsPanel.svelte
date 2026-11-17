<script lang="ts">
  // The current space's secrets on the real server, by name and label. The server never sends a value, and neither does this page; requests waiting for someone to supply a value are listed too.
  import { ServerError, server } from "../../server/client";
  import { spaces } from "../../shell/spaces.svelte";
  import "./panel.css";
  interface Secret { name: string; label: string; updated_at: number }
  interface Ask { id: string; name: string; label: string; reason: string }
  let secrets = $state<Secret[]>([]), asks = $state<Ask[]>([]), error = $state("");
  const space = $derived(spaces.current.id);
  $effect(() => {
    const id = space;
    server.operate<{ secrets: Secret[]; requests: Ask[] }>("secrets.list", { space_id: id }).then((r) => { secrets = r.secrets; asks = r.requests; error = ""; }, (e) => { error = e instanceof ServerError ? e.message : "Could not load the secrets."; });
  });
</script>

<div class="sp">
  {#each secrets as s (s.name)}<div class="r"><span class="n"><span class="t">{s.label || s.name}</span><span class="h">{s.name} · Set</span></span></div>{/each}
  {#each asks as a (a.id)}<div class="r"><span class="n"><span class="t">{a.label || a.name}</span><span class="h">Waiting for a value · {a.reason}</span></span></div>{/each}
  {#if !secrets.length && !asks.length && !error}<div class="r"><span class="h">No secrets in this space.</span></div>{/if}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
