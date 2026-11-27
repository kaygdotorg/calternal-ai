<script lang="ts">
  // The Cal's icon picker: a grid of emoji and a row of colours; the person's choice is the Cal's own mark everywhere it appears (the row, the strip, the window). Opens from the Cal's menu.
  import { Sheet } from "@calternal/ui";
  import Mark from "./Mark.svelte";
  import { arr } from "../arrange.svelte";
  let { open, id, name, onclose }: { open: boolean; id: string; name: string; onclose: () => void } = $props();
  const GLYPHS = ["spark", "play", "search", "branch", "shield", "key", "bell", "code", "pulse", "terminal", "user", "folder", "wrench", "link", "apps", "clock", "edit", "archive", "share", "check"];
  const EMOJI = ["🚀", "🔎", "🧭", "📚", "🛠️", "🧪", "🌱", "🎯", "📝", "🗓️", "💡", "🧹", "🧾", "🎨", "🛰️", "🦉", "🐝", "🍵", "⚙️", "✨"];
  const HUES = [28, 55, 150, 175, 205, 240, 280, 330, 12];
  const cur = $derived(arr.cal[id] ?? { icon: "spark", hue: 205 });
</script>

<Sheet {open} label={`${name}: icon`} side="center" {onclose}>
  <div class="p" data-icon-picker>
    <header><Mark emoji={cur.icon} hue={cur.hue} size="2.5rem" /><strong>{name}</strong></header>
    <div class="g" role="radiogroup" aria-label="Icon">{#each GLYPHS as e (e)}<button type="button" role="radio" aria-checked={cur.icon === e} aria-label={e} class:on={cur.icon === e} onclick={() => arr.setCal(id, { icon: e })}><Mark emoji={e} hue={cur.hue} size="28px" /></button>{/each}</div>
    <div class="g" role="radiogroup" aria-label="Emoji">{#each EMOJI as e (e)}<button type="button" role="radio" aria-checked={cur.icon === e} class:on={cur.icon === e} onclick={() => arr.setCal(id, { icon: e })}>{e}</button>{/each}</div>
    <div class="h" role="radiogroup" aria-label="Colour">{#each HUES as h (h)}<button type="button" role="radio" aria-checked={cur.hue === h} aria-label={`Colour ${h}`} class:on={cur.hue === h} style:--h={h} onclick={() => arr.setCal(id, { hue: h })}></button>{/each}</div>
  </div>
</Sheet>

<style>
  .p { display: grid; gap: var(--s-6); padding: var(--s-7); min-width: min(22rem, 86vw); } header { display: flex; align-items: center; gap: var(--s-5); font-size: var(--text-heading); }
  .g { display: grid; grid-template-columns: repeat(5, 1fr); gap: var(--s-3); } .g button { aspect-ratio: 1; display: grid; place-items: center; font-size: 22px; border-radius: var(--r-menu); } .g button:hover { background: var(--hover); } .g button.on { background: var(--accent-soft); box-shadow: inset 0 0 0 2px var(--accent); }
  .h { display: flex; gap: var(--s-4); flex-wrap: wrap; } .h button { width: 28px; height: 28px; border-radius: 50%; background: oklch(0.78 0.1 var(--h)); } .h button.on { box-shadow: 0 0 0 2px var(--paper, #fff), 0 0 0 4px var(--accent); }
</style>
