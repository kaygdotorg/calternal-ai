<script lang="ts">
  // What wrote a message, as a card (docs/threads-ui.md, "Who wrote it"): the harness is the title, then Provider (the service that served it), Model, Reasoning, Service tier (only when it is not the default) and Context (how much
  // of the window the thread has used). Two designs of the same facts: the detailed one, in the owner's layout, and a compact icon-led one; the fixtures offer a switch between them (`cal.modelcard`), the real build shows the detailed one.
  import { Icon } from "@calternal/ui";
  import LevelMark from "./LevelMark.svelte";
  import type { MessageBy, ProviderId } from "./model";
  let { by, provider, providers, compact = false, ontoggle }: {
    by?: MessageBy; provider: ProviderId;
    providers: { id: ProviderId; name: string; models: { id: string; name: string; levels?: string[] }[] }[];
    compact?: boolean; ontoggle?: () => void;
  } = $props();
  const id = $derived(by?.provider ?? provider);
  const p = $derived(providers.find((x) => x.id === id));
  const m = $derived(by?.model ? p?.models.find((x) => x.id === by.model) : undefined);
  const model = $derived(by?.model && by.model !== "default" ? (m?.name ?? by.model) : null);
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const level = $derived(by?.reasoning ? cap(by.reasoning) : null);
  const pct = $derived(by?.context ? Math.min(100, Math.round((by.context.used / by.context.window) * 100)) : null);
  const k = (n: number) => (n >= 1000 ? `${Math.round(n / 1000)}k` : String(n));
  const ctx = $derived(by?.context ? `${k(by.context.used)} of ${k(by.context.window)}` : null);
  const R = 7, C = 2 * Math.PI * R;
</script>

<div class="card" class:compact data-card={compact ? "compact" : "detailed"}>
  <div class="head"><span class="ic"><Icon name={id === "codex" ? "codex" : id === "claude" ? "claude" : "spark"} inline /></span><span class="title">{p?.name ?? "Agent"}</span></div>
  {#if compact}
    <div class="chips">
      {#if by?.service}<span class="c" title="Provider"><Icon name="shield" inline />{by.service}</span>{/if}
      {#if model}<span class="c" title="Model"><Icon name="spark" inline />{model}</span>{/if}
      {#if level}<span class="c" title="Reasoning"><LevelMark level={by!.reasoning!} levels={m?.levels ?? ["low", "medium", "high"]} />{level}</span>{/if}
      {#if by?.tier}<span class="c" title="Service tier"><Icon name="pulse" inline />{cap(by.tier)}</span>{/if}
      {#if pct !== null}<span class="c" title="Context used {ctx}"><svg viewBox="0 0 18 18" width="14" height="14" aria-hidden="true"><circle cx="9" cy="9" r={R} class="track" /><circle cx="9" cy="9" r={R} class="fill" stroke-dasharray="{(C * pct) / 100} {C}" transform="rotate(-90 9 9)" /></svg>{pct}%</span>{/if}
    </div>
  {:else}
    <dl>
      {#if by?.service}<dt>Provider</dt><dd>{by.service}</dd>{/if}
      {#if model}<dt>Model</dt><dd>{model}</dd>{/if}
      {#if level}<dt>Reasoning</dt><dd class="lv"><LevelMark level={by!.reasoning!} levels={m?.levels ?? ["low", "medium", "high"]} />{level}</dd>{/if}
      {#if by?.tier}<dt>Service tier</dt><dd>{cap(by.tier)}</dd>{/if}
      {#if pct !== null}<dt>Context</dt><dd class="cx"><span class="bar" role="img" aria-label="{pct}% of the context window used"><span style:width="{pct}%"></span></span><span class="n">{pct}% · {ctx}</span></dd>{/if}
    </dl>
  {/if}
  {#if ontoggle}<button type="button" class="swap" onclick={ontoggle}>{compact ? "Detailed view" : "Compact view"}</button>{/if}
</div>

<style>
  .card { display: grid; gap: var(--s-4); padding: var(--s-5) var(--s-6); min-width: 15rem; max-width: 22rem; color: var(--ink); font-size: var(--text-ui); }
  .head { display: flex; align-items: center; gap: var(--s-3); }
  .ic { display: grid; place-items: center; width: var(--s-8); height: var(--s-8); border-radius: 50%; background: rgb(var(--ink-rgb) / 0.08); }
  .title { font-size: var(--text-heading); font-weight: var(--strong-weight); line-height: 1.2; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: var(--s-2) var(--s-6); margin: 0; }
  dt { color: var(--ink-3); font-size: var(--text-label); align-self: center; }
  dd { margin: 0; font-weight: var(--strong-weight); text-align: end; display: inline-flex; align-items: center; justify-content: flex-end; gap: var(--s-2); }
  .cx { flex-direction: column; align-items: flex-end; gap: var(--s-1); }
  .bar { display: block; width: 8rem; height: 4px; border-radius: 2px; background: rgb(var(--ink-rgb) / 0.12); overflow: hidden; }
  .bar > span { display: block; height: 100%; background: var(--accent-base); border-radius: inherit; }
  .n { font-size: var(--text-label); color: var(--ink-2); font-weight: var(--body-weight); font-variant-numeric: tabular-nums; }
  .chips { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .c { display: inline-flex; align-items: center; gap: var(--s-2); padding: 0 var(--s-4); min-height: var(--pill-h); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .track { fill: none; stroke: rgb(var(--ink-rgb) / 0.18); stroke-width: 2.4; }
  .fill { fill: none; stroke: var(--accent-base); stroke-width: 2.4; stroke-linecap: round; }
  .compact { min-width: 0; }
  .swap { justify-self: end; color: var(--ink-3); font-size: var(--text-label); text-decoration: underline; text-underline-offset: 2px; }
</style>
