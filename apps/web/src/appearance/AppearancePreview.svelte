<script lang="ts">
  // The live preview beside Appearance. Ported from the existing app's (components/appearance/AppearancePreview.svelte): the real thing drawn smaller, a full-size window (1280 x 800) scaled down with ONE transform from the
  // width of its box, so the type, the frost and the dim are the app's own, in proportion; it has no state of its own and follows every change because it only reads the same CSS variables. What differs: the existing
  // app drew its own picture and blur; here the picture and veil are the shell's own variables (`--wall-side`, `--veil-l`), and what it shows is this app's shell (a sidebar with its rows and capsule, the middle card as a window
  // in the person's window look, a message from the person, one from the agent with its provider mark and a code block, a choice and a switch, the composer). Read-only: inert, hidden from assistive technology, fixture content.
  import { Card, Icon, Selector, Switch } from "@calternal/ui";
  import { canvasPrefs } from "../canvas/prefs.svelte";
  const W = 1280, H = 800;
  const variant = $derived(canvasPrefs.look === "card" ? "pane" : canvasPrefs.look === "clear" ? "clear" : "solid");
  // The veil values the shell sets on the root for every step of Clear to tinted (`--l-tint`, `--c-dim-a`, `--veil-r`, …). If the preview simply inherited them, each step would restyle the whole scaled copy in
  // the same frame as the step itself. Instead the preview wears its own copy of them (an inline style on its root, so the root's change finds the same values here and stops), brought up to date in a frame
  // of its own after the movement settles, at most about eight times a second while a slider is being dragged. Measured by e2e/clarity-perf.mjs.
  const VEIL = /^--(?:[lcr]-(?:tint|dim|ink2|ink3)(?:-a)?|veil-[lcr])$/;
  const worn = (): string => { const st = document.documentElement.style; let out = ""; for (let i = 0; i < st.length; i++) { const n = st[i]!; if (VEIL.test(n)) out += `${n}:${st.getPropertyValue(n)};`; } return out; };
  let pinned = $state(worn());
  $effect(() => {
    let timer = 0, frame = 0;
    const apply = () => { timer = 0; frame = requestAnimationFrame(() => { frame = 0; const w = worn(); if (w !== pinned) pinned = w; }); };
    const mo = new MutationObserver(() => { if (!timer && !frame) timer = window.setTimeout(apply, 120); });
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["style"] });
    return () => { mo.disconnect(); if (timer) clearTimeout(timer); if (frame) cancelAnimationFrame(frame); };
  });
  let box = $state<HTMLElement>();
  let k = $state(0.3);
  $effect(() => {
    if (!box) return;
    const ro = new ResizeObserver(([e]) => { const w = e!.contentRect.width; if (w > 0) k = w / W; });
    ro.observe(box);
    return () => ro.disconnect();
  });
</script>

<div class="stage pv" aria-hidden="true" inert style={pinned}>
  <div class="frame" bind:this={box} style="aspect-ratio: {W} / {H}">
    <div class="device" style="width: {W}px; height: {H}px; scale: {k}">
      <div class="art"></div><div class="wash"></div>
      <div class="side">
        <p class="mark">Calternal</p>
        <p class="row"><Icon name="plus" inline /> New thread</p>
        <p class="row on"><Icon name="threads" inline /> Fix the flaky sign-in</p>
        <p class="row"><Icon name="threads" inline /> Draft the release notes</p>
        <p class="row"><Icon name="threads" inline /> Review the schema change</p>
        <div class="cap"><Icon name="threads" /><Icon name="file" /><Icon name="settings" /></div>
      </div>
      <Card label="Preview" {variant} class="mid">
        <div class="msgs">
          <p class="user">Why does the passkey sign-in flake on the dev VM?</p>
          <div class="agent"><Icon name="claude" /><div><p>Two things are going on, and only one of them is a bug. The <code>catalog()</code> call rebuilt its map on every request.</p><pre>let tail = journal.read_tail(max)?;
verify_chain(&amp;tail)?;
Ok(tail.into_entries())</pre></div></div>
          <div class="ctl"><Selector label="Preview choice" options={[{ value: "a", label: "Wide" }, { value: "b", label: "Centred" }]} value="a" onchange={() => {}} fill={false} /><Switch label="Notify me" checked onchange={() => {}} /></div>
        </div>
        <p class="comp">Message Claude…<span class="send"><Icon name="up" inline /></span></p>
      </Card>
    </div>
  </div>
</div>

<style>
  .stage { display: grid; width: 100%; }
  .frame { position: relative; width: 100%; overflow: hidden; border-radius: var(--r-card); box-shadow: 0 0 0 1px rgb(var(--ink-rgb) / 0.12), 0 6px 18px rgb(0 0 0 / 0.1); contain: paint; isolation: isolate; }
  .device { position: absolute; left: 0; top: 0; transform-origin: 0 0; overflow: hidden; pointer-events: none; user-select: none; display: grid; grid-template-columns: 280px minmax(0, 1fr); padding: 8px; gap: 8px; box-sizing: border-box; }
  .art, .wash { position: absolute; inset: 0; }
  .art { background: var(--wall-side, var(--wall-fallback, var(--backdrop))) center / cover no-repeat; }
  .wash { background: var(--veil-l, linear-gradient(rgb(var(--tint) / 0.5) 0 0)); }
  .side { position: relative; display: grid; align-content: start; gap: 6px; padding: 14px 10px; min-width: 0; }
  .mark { font-family: var(--font-brand); font-size: 30px; padding: 0 10px 10px; }
  .row { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-radius: var(--r-select); color: var(--ink-2); font-size: var(--text-ui); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row.on { background: var(--accent-soft); color: var(--accent); }
  .cap { position: absolute; left: 10px; bottom: 14px; display: flex; gap: 16px; padding: 12px 20px; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.08); color: var(--ink-2); }
  .device :global(.mid) { position: relative; z-index: 1; min-width: 0; display: grid; grid-template-rows: minmax(0, 1fr) auto; padding: 24px 32px 20px; gap: 18px; overflow: hidden; }
  .msgs { display: grid; align-content: start; gap: 20px; min-height: 0; overflow: hidden; font-size: var(--text-ui); }
  .user { justify-self: end; max-width: 70%; padding: 12px 18px; border-radius: var(--r-pane); background: rgb(var(--ink-rgb) / 0.07); }
  .agent { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 12px; color: var(--ink); font-family: var(--font-chat, var(--font-ui)); max-width: 80%; }
  .agent code { font-family: var(--font-mono); font-size: var(--text-code); background: rgb(var(--ink-rgb) / 0.07); padding: 0 6px; border-radius: var(--r-select); }
  .agent pre { margin: 10px 0 0; padding: 14px 18px; border-radius: var(--r-card); background: rgb(var(--ink-rgb) / 0.07); font-family: var(--font-mono); font-size: var(--text-code); overflow: hidden; }
  .ctl { display: flex; align-items: center; gap: 24px; max-width: 70%; }
  .comp { display: flex; align-items: center; justify-content: space-between; padding: 10px 10px 10px 22px; border-radius: var(--r-pill); background: rgb(var(--ink-rgb) / 0.07); color: var(--ink-3); font-size: var(--text-ui); }
  .send { display: grid; place-items: center; width: var(--pill-h); height: var(--pill-h); border-radius: 50%; background: var(--accent-base); color: #fff; }
</style>
