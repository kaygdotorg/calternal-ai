<script lang="ts">
  // One node of a generated description. The catalog has 67 components; the web client draws the ones the shell needs first (question
  // forms, cards, tables, status) and shows a plain fallback line for the rest, so a description never breaks the page.
  import type { UiNode, Value } from "@calternal/genui";
  import { Badge, Button } from "@calternal/ui";
  import Md from "../lib/Md.svelte";
  import type { Action, Host } from "./actions";
  import GenNode from "./GenNode.svelte";

  let { node, host, act }: { node: UiNode; host: Host; act: (a: Action[] | undefined) => void } = $props();
  const p = $derived(node.props as Record<string, any>);
  const type = $derived(node.type);
  const kids = $derived((p.children ?? []) as UiNode[]);
  const isNode = (v: unknown): v is UiNode => typeof v === "object" && v !== null && (v as UiNode).kind === "node";
  const bind = $derived(p.value as { name: string; value: Value } | undefined);
  const opt = (o: any): { value: string; label: string; detail?: string } => (typeof o === "object" && o ? { value: String(o.value), label: String(o.label ?? o.value), detail: o.detail } : { value: String(o), label: String(o) });
  const tone = (t?: string) => (t === "bad" ? "bad" : t === "good" || t === "warn" || t === "info" ? t : "neutral");
  const id = $props.id();
</script>

{#if type === "Error"}
  <p class="fallback" role="alert">{p.message}</p>
{:else if type === "Card"}
  <div class="card" role="group" aria-label={p.title ?? "Card"}>{#if p.title}<p class="h" role="heading" aria-level="2">{p.title}</p>{/if}{#each kids as k (k.key)}<GenNode node={k} {host} {act} />{/each}</div>
{:else if type === "Stack"}
  <div class="stack">{#each kids as k (k.key)}<GenNode node={k} {host} {act} />{/each}</div>
{:else if type === "Row"}
  <div class="rowl">{#each kids as k (k.key)}<GenNode node={k} {host} {act} />{/each}</div>
{:else if type === "Text"}
  <div class="text {p.tone ?? ''}"><Md text={String(p.text)} /></div>
{:else if type === "Callout"}
  <div class="callout {tone(p.tone)}" role="note">{#if p.title}<strong>{p.title}</strong>{/if}<Md text={String(p.text)} /></div>
{:else if type === "Badge"}
  <Badge text={String(p.text)} tone={tone(p.tone) as any} />
{:else if type === "Divider"}
  <hr />
{:else if type === "Code"}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <pre tabindex="0" aria-label="Code"><code>{p.code}</code></pre>
{:else if type === "Details"}
  <details open={p.open}><summary>{p.title}</summary>{#each kids as k (k.key)}<GenNode node={k} {host} {act} />{/each}</details>
{:else if type === "Progress"}
  <div class="progress"><span id="{id}-l">{p.label}</span><div class="bar {tone(p.tone)}" role="progressbar" aria-labelledby="{id}-l" aria-describedby="{id}-d" aria-valuemin="0" aria-valuemax={p.max ?? 100} aria-valuenow={p.value}><i style:width="{Math.min(100, (p.value / (p.max ?? 100)) * 100)}%"></i></div><span class="visually-hidden" id="{id}-d">{p.description}</span></div>
{:else if type === "KeyValue"}
  <dl class="kv">{#each Object.entries(p.pairs ?? {}) as [k, v]}<dt>{k}</dt><dd>{String(v)}</dd>{/each}</dl>
{:else if type === "Table"}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="tablewrap" role="group" aria-label="Table: {p.columns.map(String).join(", ")}" tabindex="0">
    <table>
      <thead><tr>{#each p.columns as c}<th scope="col">{isNode(c) ? "" : String(c)}</th>{/each}</tr></thead>
      <tbody>{#each p.rows as r}<tr>{#each r as cell}<td>{#if isNode(cell)}<GenNode node={cell} {host} {act} />{:else}{String(cell)}{/if}</td>{/each}</tr>{/each}</tbody>
    </table>
  </div>
{:else if type === "Form"}
  <form class="form" aria-label={p.label} onsubmit={(e) => e.preventDefault()}>
    <p class="h" role="heading" aria-level="2">{p.label}</p>
    {#each kids as k (k.key)}<GenNode node={k} {host} {act} />{/each}
  </form>
{:else if type === "Input" || type === "TextArea"}
  <label class="field"><span>{p.label}</span>
    {#if type === "Input"}<input type="text" value={String(bind?.value ?? "")} placeholder={p.placeholder} oninput={(e) => host.set(bind!.name, e.currentTarget.value)} />
    {:else}<textarea rows="3" value={String(bind?.value ?? "")} placeholder={p.placeholder} oninput={(e) => host.set(bind!.name, e.currentTarget.value)}></textarea>{/if}
  </label>
{:else if type === "Select"}
  <label class="field"><span>{p.label}</span>
    <select value={String(bind?.value ?? "")} onchange={(e) => host.set(bind!.name, e.currentTarget.value)}>{#each p.options as o}{@const x = opt(o)}<option value={x.value}>{x.label}</option>{/each}</select>
  </label>
{:else if type === "Choice"}
  <div class="field" role="radiogroup" aria-labelledby="{id}-c"><span id="{id}-c">{p.label}</span>
    <div class="choice">{#each p.options as o}{@const x = opt(o)}<button type="button" role="radio" aria-checked={String(bind?.value) === x.value} onclick={() => host.set(bind!.name, x.value)}>{x.label}</button>{/each}</div>
  </div>
{:else if type === "Radio"}
  <div class="field" role="radiogroup" aria-labelledby="{id}-r"><span id="{id}-r">{p.label}</span>
    <div class="radios">{#each p.options as o}{@const x = opt(o)}<button type="button" role="radio" aria-checked={String(bind?.value) === x.value} onclick={() => host.set(bind!.name, x.value)}><b>{x.label}</b>{#if x.detail}<small>{x.detail}</small>{/if}</button>{/each}</div>
  </div>
{:else if type === "Toggle"}
  <button type="button" class="toggle" role="switch" aria-checked={!!bind?.value} onclick={() => host.set(bind!.name, !bind!.value)}><span>{p.label}</span><i class="track"><b></b></i></button>
{:else if type === "Checklist"}
  <fieldset class="field"><legend>{p.label}</legend>
    {#each p.options as o}{@const x = opt(o)}{@const on = ((bind?.value as string[]) ?? []).includes(x.value)}
      <label class="check"><input type="checkbox" checked={on} onchange={() => host.set(bind!.name, on ? ((bind!.value as string[]) ?? []).filter((v) => v !== x.value) : [...((bind!.value as string[]) ?? []), x.value])} />{x.label}</label>
    {/each}
  </fieldset>
{:else if type === "Button"}
  <Button text={String(p.label)} variant={p.variant === "destructive" ? "destructive" : p.variant === "primary" ? "primary" : "secondary"} onclick={() => act(p.action)} />
{:else}
  <p class="fallback">{type} is not drawn by this client yet.</p>
{/if}

<style>
  .card { display: grid; gap: var(--s-4); padding: var(--s-5) var(--s-6); border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.05); min-width: 0; }
  .h { font-size: var(--text-ui); font-weight: var(--strong-weight); }
  .stack, .form { display: grid; gap: var(--s-4); }
  .rowl { display: flex; flex-wrap: wrap; gap: var(--s-4); align-items: center; }
  .fallback { color: var(--ink-3); font-size: var(--text-label); padding: var(--s-3) var(--s-5); border-radius: var(--r-control); background: rgb(var(--ink-rgb) / 0.05); }
  .callout { display: grid; gap: var(--s-2); padding: var(--s-4) var(--s-5); border-radius: var(--r-control); background: color-mix(in oklab, var(--c, var(--ink-3)) 12%, transparent); }
  .callout.good { --c: var(--good); } .callout.warn { --c: var(--warn); } .callout.bad { --c: var(--danger); } .callout.info { --c: var(--info); }
  hr { border: 0; height: 1px; background: var(--line); width: 100%; }
  pre { margin: 0; padding: var(--s-4) var(--s-5); border-radius: var(--r-control); background: rgb(var(--ink-rgb) / 0.06); overflow-x: auto; font-size: var(--text-label); }
  details summary { cursor: pointer; font-weight: var(--strong-weight); min-height: var(--row-h); display: flex; align-items: center; }
  .progress { display: grid; gap: var(--s-2); font-size: var(--text-label); color: var(--ink-2); }
  .bar { height: var(--s-3); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.1); overflow: hidden; --c: var(--accent); }
  .bar.good { --c: var(--good); } .bar.warn { --c: var(--warn); } .bar.bad { --c: var(--danger); }
  .bar i { display: block; height: 100%; background: var(--c); border-radius: inherit; transition: width var(--dur-base) var(--ease-out); }
  .kv { display: grid; grid-template-columns: max-content 1fr; gap: var(--s-2) var(--s-6); margin: 0; font-size: var(--text-label); }
  dt { color: var(--ink-3); } dd { margin: 0; }
  .tablewrap { overflow-x: auto; border-radius: var(--r-control); }
  table { border-collapse: collapse; width: 100%; font-size: var(--text-label); }
  th { text-align: start; color: var(--ink-3); font-weight: var(--strong-weight); }
  th, td { padding: var(--s-3) var(--s-5) var(--s-3) 0; white-space: nowrap; }
  tbody tr + tr td { border-top: 1px solid var(--line); }
  .field { display: grid; gap: var(--s-2); border: 0; padding: 0; margin: 0; font-size: var(--text-label); color: var(--ink-2); font-weight: var(--strong-weight); }
  input[type="text"], textarea, select { width: 100%; min-height: calc(var(--pill-h) + var(--s-2)); padding: 0 var(--s-5); border: 0; border-radius: var(--r-control); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); font-weight: var(--ui-weight); }
  textarea { padding: var(--s-4) var(--s-5); resize: vertical; }
  .choice { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .choice button, .radios button { min-height: max(var(--pill-h), var(--hit-min)); padding: 0 var(--s-5); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink); font-size: var(--text-ui); font-weight: var(--ui-weight); }
  .choice button[aria-checked="true"], .radios button[aria-checked="true"] { background: var(--accent-soft); color: var(--accent); font-weight: var(--strong-weight); }
  .radios { display: grid; gap: var(--s-2); } .radios button { display: grid; text-align: start; border-radius: var(--r-control); padding: var(--s-3) var(--s-5); } small { font-weight: var(--ui-weight); color: var(--ink-3); }
  .toggle { display: flex; align-items: center; justify-content: space-between; gap: var(--s-5); width: 100%; min-height: max(var(--pill-h), var(--hit-min)); font-size: var(--text-ui); text-align: start; }
  .track { position: relative; flex: none; width: calc(var(--s-9) + var(--s-4)); height: calc(var(--s-8) + var(--s-1) * 2); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.18); transition: background-color var(--dur-fast) var(--ease-out); }
  .track b { position: absolute; top: var(--s-1); left: var(--s-1); width: var(--s-8); height: var(--s-8); border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgb(0 0 0 / 0.3); transition: translate var(--dur-base) var(--spring); }
  .toggle[aria-checked="true"] .track { background: var(--accent-base); }
  .toggle[aria-checked="true"] .track b { translate: calc(var(--s-9) + var(--s-4) - var(--s-8) - var(--s-1) * 2) 0; }
  .check { display: flex; align-items: center; gap: var(--s-4); min-height: max(var(--pill-h), var(--hit-min)); font-weight: var(--ui-weight); color: var(--ink); font-size: var(--text-ui); }
  .check input { width: var(--s-7); height: var(--s-7); accent-color: var(--accent-base); }
</style>
