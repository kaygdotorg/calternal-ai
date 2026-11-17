import { mount } from "svelte";
import "@calternal/ui/styles";
import { currentScheme, initScheme, loadFonts } from "@calternal/ui";
import App from "./App.svelte";
import { interceptLinks } from "./lib/route.svelte";
import { shell } from "./shell/shell.svelte";

initScheme();
shell.start();
interceptLinks();

// Nothing paints until the active scheme's font family is in: no fallback face is ever drawn. A slow network cannot block start-up
// for more than two seconds, after which the app paints with whatever has arrived (still only the app's own families).
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(() => {
  mount(App, { target: document.getElementById("app")! });
  document.documentElement.dataset.ready = "";
});
