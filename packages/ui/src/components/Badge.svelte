<script lang="ts">
  // A one to three word status. The tone is never the only signal: the text says it too.
  let { text, tone = "neutral", dot = false }: { text: string; tone?: "neutral" | "good" | "warn" | "bad" | "info" | "accent"; dot?: boolean } = $props();
</script>

<span class="badge {tone}">{#if dot}<i aria-hidden="true"></i>{/if}{text}</span>

<style>
  .badge { display: inline-flex; align-items: center; gap: var(--s-2); height: calc(var(--s-7)); padding: 0 var(--s-4); border-radius: var(--r-pill); font-size: var(--text-section); font-weight: var(--strong-weight); white-space: nowrap; --c: var(--ink-3); color: var(--c); background: color-mix(in oklab, var(--c) 14%, transparent); }
  i { width: var(--s-3); height: var(--s-3); border-radius: 50%; background: currentColor; }
  .good { --c: var(--good); } .warn { --c: var(--warn); } .bad { --c: var(--danger); } .info { --c: var(--info); } .accent { --c: var(--accent); }
</style>
