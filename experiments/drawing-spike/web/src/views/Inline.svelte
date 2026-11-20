<script lang="ts">
  // A record body in the closed inline Markdown subset: `code`, **strong**, [label](person:id) as a mention, [label](url) as
  // a link that opens through the host. Plain text otherwise; no HTML. Line breaks are kept.
  import { viewer } from "../lib/model";
  let { body }: { body: string } = $props();
  const parts = $derived(body.split(/(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/).filter(Boolean));
  const link = (p: string) => /^\[([^\]]+)\]\(([^)]+)\)$/.exec(p);
</script>

<p class="t">{#each parts as p, i (i)}{#if p.startsWith("`")}<code>{p.slice(1, -1)}</code>{:else if p.startsWith("**")}<strong>{p.slice(2, -2)}</strong>{:else if link(p)}{@const m = link(p)!}{#if m[2]!.startsWith("person:")}<span class="mn" class:me={m[2] === `person:${viewer.id}`}>{m[1]}</span>{:else}<a href={m[2]} rel="noreferrer noopener" target="_blank">{m[1]}</a>{/if}{:else}{p}{/if}{/each}</p>

<style>
  .t { margin: 0; font-family: var(--font-chat, inherit); font-size: var(--text-ui); line-height: 1.45; white-space: pre-wrap; overflow-wrap: anywhere; }
  code { font-family: var(--font-mono); font-size: var(--text-code); padding: 0 var(--s-1); border-radius: 4px; background: rgb(var(--ink-rgb) / 0.08); }
  strong { font-weight: var(--strong-weight); } .mn { color: var(--accent); font-weight: var(--strong-weight); } .mn.me { background: var(--accent-soft); border-radius: 4px; padding: 0 2px; }
  a { color: var(--accent); }
</style>
