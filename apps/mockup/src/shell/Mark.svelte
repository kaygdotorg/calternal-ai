<script lang="ts">
  // A small round mark for who or what a row is about: an agent's provider (Claude, Codex, Gemini, a local model: its own glyph on its own tint, in place of initials), or a Cal's own icon on the colour its owner chose. A Cal's icon is a glyph
  // from the icon set (drawn on the colour) or an emoji; where the system has no colour emoji font the emoji is never drawn as an empty circle: the mark falls back to a glyph on the same colour.
  import { Icon, ICONS, type IconName } from "@calternal/ui";
  let { provider, emoji, hue, size = "var(--mark, 22px)" }: { provider?: "claude" | "codex" | "gemini" | "local"; /** An icon-set name, or an emoji. */ emoji?: string; hue?: number; size?: string } = $props();
  const NAME = { claude: "Claude", codex: "Codex", gemini: "Gemini", local: "Local model" } as const;
  const glyph = $derived(emoji && /^[a-z-]+$/.test(emoji) && emoji in ICONS ? (emoji as IconName) : null);
  const emo = $derived(emoji && !glyph && emojiOk() ? emoji : null);
  let cached: boolean | null = null;
  /** Whether this system draws colour emoji: draws one on a canvas and looks for saturated pixels (tofu boxes and text-presentation glyphs are grey). Cached. */
  function emojiOk(): boolean {
    if (cached !== null) return cached;
    try {
      const c = document.createElement("canvas"); c.width = c.height = 32; const x = c.getContext("2d", { willReadFrequently: true })!; x.fillStyle = "#fff"; x.fillRect(0, 0, 32, 32); x.font = "26px sans-serif"; x.textBaseline = "middle"; x.fillText("🚀", 2, 17);
      const d = x.getImageData(0, 0, 32, 32).data; let sat = 0; for (let i = 0; i < d.length; i += 4) if (Math.max(d[i]!, d[i + 1]!, d[i + 2]!) - Math.min(d[i]!, d[i + 1]!, d[i + 2]!) > 60) sat++;
      return (cached = sat > 12);
    } catch { return (cached = false); }
  }
</script>

{#if emoji}
  <span class="m" style:--sz={size} style:--h={hue ?? 205} role="img" aria-label="Icon">{#if glyph}<Icon name={glyph} inline />{:else if emo}<span class="e">{emo}</span>{:else}<Icon name="spark" inline />{/if}</span>
{:else if provider}<span class="m p" data-p={provider} style:--sz={size} role="img" aria-label={NAME[provider]}>{#if provider === "claude"}<Icon name="claude" inline />{:else if provider === "codex"}<Icon name="codex" inline />{:else if provider === "gemini"}<Icon name="spark" inline />{:else}<Icon name="terminal" inline />{/if}</span>{/if}

<style>
  .m { display: inline-grid; place-items: center; flex: none; width: var(--sz); height: var(--sz); border-radius: 50%; background: oklch(0.82 0.08 var(--h) / 0.6); color: var(--ink); }
  .e { font-size: calc(var(--sz) * 0.58); line-height: 1; }
  .p[data-p="claude"] { background: oklch(0.78 0.1 45); color: #3a1d0c; } .p[data-p="codex"] { background: #1c1c1e; color: #fff; } .p[data-p="gemini"] { background: oklch(0.78 0.1 260); color: #14204a; } .p[data-p="local"] { background: oklch(0.82 0.04 150); color: #1d3326; }
</style>
