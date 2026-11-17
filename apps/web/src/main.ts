import { mount } from "svelte";
import "@calternal/ui/styles";
import { currentScheme, initInputModality, initScheme, loadFonts, registerFonts } from "@calternal/ui";
import App from "./App.svelte";
import { interceptLinks } from "./lib/route.svelte";
import { shell } from "./shell/shell.svelte";
import { appearance } from "./appearance/prefs.svelte";

registerFonts(import.meta.env.BASE_URL);
initInputModality();
initScheme();
shell.start();
interceptLinks();

// Nothing paints until the active scheme's font family is in: no fallback face is ever drawn. A slow network cannot block start-up
// for more than two seconds, after which the app paints with whatever has arrived (still only the app's own families).
void appearance.start();
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(() => {
  mount(App, { target: document.getElementById("app")! });
  document.documentElement.dataset.ready = "";
});

// In-page benchmark for browsers that cannot be driven from outside (docs/ui-performance.md): loaded only with ?bench=1.
if (new URLSearchParams(location.search).has("bench")) void document.fonts.ready.then(() => setTimeout(() => import("./bench"), 1500));
