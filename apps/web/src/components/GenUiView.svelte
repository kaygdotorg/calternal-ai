<script lang="ts">
  import { validateView, checkAction, type ActionPolicy } from '../lib/genui';
  import type { GenUiView } from '../lib/types';
  import GenUiNode from './GenUiNode.svelte';
  import { app } from '../lib/store.svelte';

  let { view, policy, context = {}, depth = 0 }: {
    view: unknown; policy: ActionPolicy; context?: Record<string, unknown>; depth?: number;
  } = $props();

  const errors = $derived(validateView(view));
  let results = $state<Record<string, { busy: boolean; error?: string; data?: any }>>({});

  const ctx = $derived({
    policy, context,
    result: (id: string) => results[id],
    async run(id: string, operation: string, input: Record<string, unknown>) {
      // Re-check at dispatch time (plugin may have been disabled/replaced since render). The server re-authorizes too.
      const why = checkAction(policy, operation);
      if (why) { results[id] = { busy: false, error: why }; return; }
      results[id] = { busy: true };
      try {
        // Host context (open note's space_id/id) always wins: a plugin cannot retarget its action at another resource.
        const merged = { ...input, ...context };
        results[id] = { busy: false, data: await app.dispatch(operation, merged) };
      } catch (e) {
        results[id] = { busy: false, error: (e as Error).message };
      }
    }
  });
</script>

{#if errors.length}
  <div class="banner err" role="alert">
    <div><strong>View rejected.</strong> This GenUI tree did not pass client validation, so it was not rendered.
      <ul>{#each errors as er (er)}<li>{er}</li>{/each}</ul></div>
  </div>
{:else}
  <div class="genui" data-genui-version="1"><GenUiNode node={(view as GenUiView).root} {ctx} {depth} /></div>
{/if}

<style>
  .genui { display: grid; }
  ul { margin: 4px 0 0; padding-left: 18px; }
</style>
