/**
 * The first twelve presets (docs/ui-backgrounds-and-themes.md section 4): a preset is a backdrop recipe, a palette per scheme, an accent, a reading face and a
 * material. Applying one sets the wallpaper and the palette together; the wallpaper and the theme are still two independent settings underneath, so a
 * person can change either afterwards. Community palettes keep their own paper and take no stock wallpaper ("none": a mesh derived from the palette).
 * Licences and sources are kept with each community palette. The choices (which twelve, which defaults) are data here, not code.
 */
import type { Palette, Theme } from "./theme";

/** A preset. `theme` null means the built-in tokens; `wallpaper` is a library id or "none". */
export interface Preset {
  id: string;
  name: string;
  wallpaper: string;
  theme: Theme | null;
  /** Accent from the wallpaper (true) or the theme's own. */
  accentFromWallpaper: boolean;
  /** "glass" is the shell's pane; "paper" is an opaque card. */
  material: "glass" | "paper";
}

const pal = (paper: string, ink: string, accent: string, danger: string, good: string, warn: string, info: string): Palette => ({ paper, ink, accent, danger, good, warn, info });
const theme = (id: string, name: string, light: Palette, dark: Palette, licence: string, source: string): Theme => ({ id, name, light, dark, licence, source });

export const PRESETS: Preset[] = [
  { id: "calternal", name: "Calternal", wallpaper: "haze", theme: null, accentFromWallpaper: true, material: "glass" },
  { id: "paper", name: "Paper", wallpaper: "none", theme: null, accentFromWallpaper: false, material: "paper" },
  { id: "slate", name: "Slate", wallpaper: "slate", theme: null, accentFromWallpaper: true, material: "glass" },
  { id: "dune", name: "Dune", wallpaper: "dune", theme: null, accentFromWallpaper: true, material: "glass" },
  { id: "sage", name: "Sage", wallpaper: "sage", theme: null, accentFromWallpaper: true, material: "glass" },
  { id: "dusk", name: "Dusk", wallpaper: "dusk", theme: null, accentFromWallpaper: true, material: "glass" },
  { id: "catppuccin", name: "Catppuccin", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("catppuccin", "Catppuccin", pal("#eff1f5", "#4c4f69", "#8839ef", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5"), pal("#1e1e2e", "#cdd6f4", "#cba6f7", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa"), "MIT", "https://github.com/catppuccin/catppuccin") },
  { id: "rose-pine", name: "Rosé Pine", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("rose-pine", "Rosé Pine", pal("#faf4ed", "#575279", "#907aa9", "#b4637a", "#286983", "#ea9d34", "#56949f"), pal("#191724", "#e0def4", "#c4a7e7", "#eb6f92", "#9ccfd8", "#f6c177", "#31748f"), "MIT", "https://github.com/rose-pine/rose-pine-theme") },
  { id: "tokyo-night", name: "Tokyo Night", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("tokyo-night", "Tokyo Night", pal("#e1e2e7", "#3760bf", "#2e7de9", "#f52a65", "#587539", "#8c6c3e", "#2e7de9"), pal("#1a1b26", "#c0caf5", "#7aa2f7", "#f7768e", "#9ece6a", "#e0af68", "#7dcfff"), "Apache-2.0", "https://github.com/folke/tokyonight.nvim") },
  { id: "nord", name: "Nord", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("nord", "Nord", pal("#eceff4", "#2e3440", "#5e81ac", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1"), pal("#2e3440", "#eceff4", "#88c0d0", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1"), "MIT", "https://github.com/nordtheme/nord") },
  { id: "gruvbox", name: "Gruvbox", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("gruvbox", "Gruvbox", pal("#fbf1c7", "#3c3836", "#d65d0e", "#cc241d", "#98971a", "#d79921", "#458588"), pal("#282828", "#ebdbb2", "#fe8019", "#fb4934", "#b8bb26", "#fabd2f", "#83a598"), "MIT", "https://github.com/morhetz/gruvbox") },
  { id: "mono", name: "Mono", wallpaper: "none", accentFromWallpaper: false, material: "paper", theme: theme("mono", "Mono", pal("#f7f7f7", "#262626", "#262626", "#b3412e", "#2f6d3d", "#8f5f0c", "#2f6db3"), pal("#1f1f1f", "#ebebeb", "#ebebeb", "#ee917b", "#7fc68f", "#e0b25a", "#86b5ee"), "own", "this repository") },
];

/** The preset with an id. */
export const presetById = (id: string): Preset | undefined => PRESETS.find((p) => p.id === id);
