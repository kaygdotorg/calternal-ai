<script lang="ts">
  // The plugin install sheet, on the search surface (GlassSheet) like "New space". First the catalogue (three mock plugins), then the plugin's page: one plain statement per line with an icon, always in the same order (what it stores, where it sends data, which sign-ins it uses, which programs it runs), and a warning line only when it ships web code. Fixture only: Install says it is installed and nothing else.
  import { Icon, toast } from "@calternal/ui";
  import GlassSheet from "./GlassSheet.svelte";
  import { PLUGINS, type PluginInfo } from "../data/plugins";
  let { open, onclose, start = null }: { open: boolean; onclose: () => void; start?: string | null } = $props();
  import { arr } from "../arrange.svelte";
  import { tokenMs } from "@calternal/ui";
  // MOTION (all from the motion tokens; order, duration and curve stated; every part interruptible; reduced motion turns each duration into 1 ms through the shell's global rule, and the script paths below skip their animation too):
  //  1. open: the dim fades in over --dur-sheet on --ease-sheet while the surface rises and scales in (Sheet / GlassSheet); close: both fade over --dur-leave.
  //  2. catalogue row to plugin page: the sheet's height eases from the catalogue's to the page's over --dur-base on --ease-out while the old content fades out (first 40 %) and the page's content fades and rises 6 px in (the rest); going back is the same in reverse.
  //  3. the statements arrive in order, 40 ms apart (6 at most), each a --dur-base rise of 8 px on --ease-out; the web-code warning comes after them and draws the eye ONCE: one expanding ring over 900 ms, no loop.
  //  4. press: rows and buttons scale to --press-scale over --dur-fast on --ease-out.
  //  5. Install: pressed (scale), working (the label becomes a spinner, 700 ms in the mock), done (the button turns into the confirmation "Installed", 600 ms), then the sheet leaves and the new sidebar arrives in the card under the wordmark (the card's own rise and slide).
  let pick = $state<string | null>(start), installed = $state<string[]>([]), phase = $state<"idle" | "working" | "done">("idle");
  let box = $state<HTMLElement>();
  $effect(() => { if (open) { pick = start; phase = "idle"; } });
  const plugin = $derived<PluginInfo | undefined>(PLUGINS.find((p) => p.id === pick));
  const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  /** Moves between the catalogue and a plugin page: height morph plus a cross-fade, interruptible (a new call cancels the running animations from where they are). */
  function go(to: string | null) {
    const el = box; if (!el || reduced()) { pick = to; return; }
    el.getAnimations({ subtree: false }).forEach((a) => a.cancel());
    const h0 = el.getBoundingClientRect().height; el.style.height = h0 + "px"; el.style.overflow = "hidden";
    const d = tokenMs("--dur-base", 220), ease = getComputedStyle(document.documentElement).getPropertyValue("--ease-out").trim() || "ease-out";
    const inner = el.firstElementChild as HTMLElement | null;
    const out = inner?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: d * 0.4, easing: "linear", fill: "forwards" });
    const done = () => {
      pick = to;
      queueMicrotask(() => requestAnimationFrame(() => {
        el.style.height = "auto"; const h1 = el.getBoundingClientRect().height; el.style.height = h0 + "px";
        const grow = el.animate([{ height: h0 + "px" }, { height: h1 + "px" }], { duration: d, easing: ease, fill: "forwards" });
        const n = el.firstElementChild as HTMLElement | null; n?.animate([{ opacity: 0, transform: "translateY(6px)" }, { opacity: 1, transform: "none" }], { duration: d * 0.6, easing: ease, fill: "both" });
        grow.onfinish = () => { el.style.height = ""; el.style.overflow = ""; grow.cancel(); };
      }));
    };
    if (out) out.onfinish = done; else done();
  }
  const close = () => { onclose(); };
  function install() {
    if (!plugin || phase !== "idle") return; const p = plugin; phase = "working";
    setTimeout(() => { phase = "done"; installed = [...installed, p.id]; setTimeout(() => { onclose(); if (!arr.custom.some((c) => c.name === p.name)) arr.addSidebar(p.name); toast(`${p.name} installed`); }, reduced() ? 0 : 600); }, reduced() ? 0 : 700);
  }
</script>

<GlassSheet {open} label="Plugins" onclose={close}>
  <div class="ps" data-plugin-sheet bind:this={box}><div class="pg" data-page={plugin?.id ?? "catalogue"}>
    {#if !plugin}
      <h2>Add a plugin</h2>
      <p class="intro">A plugin adds an app to this space. Each one says plainly what it keeps, where it sends things and what it runs before you install it.</p>
      <ul class="cat">{#each PLUGINS as p (p.id)}<li><button type="button" onclick={() => go(p.id)}><span class="nm">{p.name}{#if p.official}<span class="off"><Icon name="shield" inline />Official</span>{/if}</span><span class="bl">{p.blurb}</span><span class="by">{p.by}{#if installed.includes(p.id)} · installed{/if}</span></button></li>{/each}</ul>
      <div class="bt"><button type="button" onclick={close}>Close</button></div>
    {:else}
      <header><button type="button" class="back" onclick={() => go(null)} aria-label="Back to plugins"><Icon name="back" inline /></button>
        <div><h2>{plugin.name}{#if plugin.official}<span class="off"><Icon name="shield" inline />Official</span>{/if}</h2><p class="by">{plugin.by}</p></div></header>
      <ul class="st" aria-label="What this plugin does">
        {#each plugin.stores as s, i (s.what)}<li style:--i={i}><span class="ic"><Icon name={s.editable ? "file" : "lock"} /></span><span class="tx"><b>Stores</b>{" "}{s.what}{#if s.where}{" in "}<code>{s.where}</code>{/if}<small class:ed={s.editable}>{s.editable ? "You can edit these files yourself." : "Changed only through the app."}</small></span></li>{/each}
        <li style:--i={plugin.stores.length}><span class="ic"><Icon name="external" /></span><span class="tx">{#if plugin.sends.length}<b>Sends data to</b>{" "}{plugin.sends.length === 1 ? "one place" : `${plugin.sends.length} places`}{#each plugin.sends as d (d.host)}<small><code>{d.host}</code>: {d.why}.</small>{/each}{:else}<b>Sends data nowhere.</b>{/if}</span></li>
        <li style:--i={plugin.stores.length + 1}><span class="ic"><Icon name="key" /></span><span class="tx">{#if plugin.signins.length}<b>Uses {plugin.signins.length === 1 ? "one sign-in" : `${plugin.signins.length} sign-ins`}:</b> {plugin.signins.join(", ")}<small>The server holds it. The plugin never sees it.</small>{:else}<b>Uses no sign-ins.</b>{/if}</span></li>
        <li style:--i={plugin.stores.length + 2}><span class="ic"><Icon name="terminal" /></span><span class="tx">{#if plugin.programs.length}<b>Runs {plugin.programs.length === 1 ? "one program" : `${plugin.programs.length} programs`}:</b> {plugin.programs.join(", ")}<small>Background work counts against this account's hours.</small>{:else}<b>Runs no programs.</b>{/if}</span></li>
      </ul>
      {#if plugin.webCode}<p class="warn" role="alert" style:--i={plugin.stores.length + 3}><Icon name="warning" /><span><b>Ships web code.</b> It runs in your browser, on this page's behalf. Install it only if you trust {plugin.by.replace(" (third party)", "")}.</span></p>{/if}
      <div class="bt"><button type="button" onclick={close}>Cancel</button><button type="button" class="pri" class:work={phase === "working"} class:done={phase === "done"} disabled={phase !== "idle"} onclick={install} aria-live="polite">{#if phase === "working"}<span class="sp" aria-hidden="true"></span>Installing…{:else if phase === "done"}<Icon name="check" inline />Installed{:else}Install {plugin.name}{/if}</button></div>
    {/if}
  </div></div>
</GlassSheet>

<style>
  .ps { display: grid; gap: var(--s-5); padding: var(--s-7); min-width: min(34rem, 88vw); box-sizing: border-box; } h2 { margin: 0; display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; font-size: var(--text-heading); font-weight: var(--strong-weight); } p { margin: 0; }
  .intro, .by { color: var(--ink-2); font-size: var(--text-label); line-height: 1.35; }
  .off { display: inline-flex; align-items: center; gap: var(--s-2); padding: 0 var(--s-4); min-height: 22px; border-radius: var(--r-pill); background: var(--accent-soft); color: color-mix(in oklab, var(--accent) 70%, var(--ink)); font-size: var(--text-label); font-weight: var(--strong-weight); }
  .cat { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-2); } .cat button { width: 100%; display: grid; gap: 1px; text-align: start; padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.06); color: var(--ink); } .cat .nm { display: flex; align-items: center; gap: var(--s-4); font-weight: var(--strong-weight); font-size: var(--text-ui); } .cat .bl { color: var(--ink-2); font-size: var(--text-label); } .cat .by { color: var(--ink-3); }
  header { display: flex; align-items: flex-start; gap: var(--s-4); } .back { width: var(--tb-h); height: var(--tb-h); display: grid; place-items: center; border-radius: 50%; background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); flex: none; }
  .st { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-2); } .st li { display: flex; align-items: flex-start; gap: var(--s-5); padding: var(--s-4) var(--s-5); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); } .ic { flex: none; display: grid; place-items: center; width: var(--lead); min-height: 1.5em; color: var(--ink-2); }
  .tx { display: block; font-size: var(--text-ui); line-height: 1.4; color: var(--ink); } .tx b { font-weight: var(--strong-weight); } .tx small { display: block; margin-top: 2px; color: var(--ink-2); font-size: var(--text-label); line-height: 1.35; } .tx small.ed { color: color-mix(in oklab, var(--accent) 65%, var(--ink)); } code { font-family: var(--font-mono, ui-monospace, monospace); font-size: 0.92em; }
  .warn { display: flex; align-items: flex-start; gap: var(--s-5); padding: var(--s-5); border-radius: var(--r-select); background: color-mix(in oklab, var(--danger) 16%, transparent); color: var(--ink); font-size: var(--text-ui); line-height: 1.4; } .warn :global(svg) { flex: none; color: color-mix(in oklab, var(--danger) 80%, var(--ink)); } .warn b { font-weight: var(--strong-weight); }
  .bt { display: flex; justify-content: flex-end; gap: var(--s-4); } .bt button { min-height: var(--row-h); padding: 0 var(--s-6); border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink); font-size: var(--text-ui); } .bt button.pri { background: var(--badge-bg); color: #fff; }
  .pg { display: grid; gap: var(--s-5); }
  /* press feedback on every row and button */
  .cat button, .bt button, .back { transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out); } .cat button:active, .bt button:active:not(:disabled), .back:active { scale: var(--press-scale); }
  @media (hover: hover) and (pointer: fine) { .cat button:hover { background: rgb(var(--ink-rgb) / 0.1); } }
  /* the statements arrive one after another */
  .st li { animation: arrive var(--dur-base) var(--ease-out) both; animation-delay: calc(var(--i, 0) * 40ms + 60ms); }
  @keyframes arrive { from { opacity: 0; transform: translateY(8px); } }
  /* the warning draws the eye once: it arrives after the statements, then ONE ring expands and fades (no loop) */
  .warn { animation: arrive var(--dur-base) var(--ease-out) both, ring 900ms var(--ease-out) 1 both; animation-delay: calc(var(--i, 0) * 40ms + 60ms), calc(var(--i, 0) * 40ms + 200ms); }
  @keyframes ring { 0% { box-shadow: 0 0 0 0 color-mix(in oklab, var(--danger) 55%, transparent); } 100% { box-shadow: 0 0 0 12px transparent; } }
  /* install: working, then the confirmation */
  .bt button.pri { display: inline-flex; align-items: center; gap: var(--s-4); transition: scale var(--dur-fast) var(--ease-out), background-color var(--dur-base) var(--ease-out), min-width var(--dur-base) var(--ease-out); } .bt button.pri:disabled { opacity: 1; } .bt button.pri.done { background: var(--good, oklch(0.55 0.14 150)); }
  .sp { width: 14px; height: 14px; border-radius: 50%; border: 2px solid rgb(255 255 255 / 0.4); border-top-color: #fff; animation: spin 700ms linear infinite; } @keyframes spin { to { rotate: 360deg; } }
</style>
