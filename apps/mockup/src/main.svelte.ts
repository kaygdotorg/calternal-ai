import { mount } from "svelte";
import "@calternal/ui/styles";
import "./shell/rows.css";
import { currentScheme, initInputModality, initScheme, loadFonts, registerFonts, applyScheme } from "@calternal/ui";
import "./apps";
import { cueCode } from "./lib/cue"; // the apps register their data before anything reads it
import App from "./App.svelte";
import { appearance } from "./vendor/appearance/prefs.svelte";
import { tweaks } from "./tweaks.svelte";

// The mockup's start: the app's fonts, the scheme (forced by the tweak panel or the address, else the system's), the wallpaper engine, then the shell.
registerFonts(import.meta.env.BASE_URL);
for (const [family, file, weight] of [["Caveat", "caveat-latin-wght-normal.woff2", "400 700"], ["Satisfy", "satisfy-latin-400-normal.woff2", "400"]] as const) {
  const f = new FontFace(family, `url(${import.meta.env.BASE_URL}fonts-extra/${file})`, { weight, display: "swap" });
  void f.load().then(() => document.fonts.add(f)).catch(() => {});
}
initInputModality();
initScheme();
if (tweaks.t.scheme !== "system") applyScheme(tweaks.t.scheme);
const FACES = { dancing: "var(--font-brand)", caveat: "'Caveat', cursive", satisfy: "'Satisfy', cursive" } as const;
const wm = () => document.documentElement.style.setProperty("--wm-font", FACES[tweaks.t.wordmark]);
$effect.root(() => { $effect(() => { const d = document.documentElement.dataset; d.sel = tweaks.t.selTint; d.caps = tweaks.t.caps; d.pop = tweaks.t.profile; d.winbg = tweaks.t.winBg; });
  $effect(() => { wm(); }); $effect(() => { if (tweaks.t.scheme !== "system") applyScheme(tweaks.t.scheme); else applyScheme("system"); }); });
void appearance.start();
cueCode();
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(() => { mount(App, { target: document.getElementById("app")! }); document.documentElement.dataset.ready = ""; });
