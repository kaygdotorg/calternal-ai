import { mount } from "svelte";
import "@calternal/ui/styles";
import { currentScheme, initInputModality, initScheme, loadFonts, registerFonts } from "@calternal/ui";
import App from "./App.svelte";
import { interceptLinks } from "./lib/route.svelte";
import { shell } from "./shell/shell.svelte";
import { appearance } from "./appearance/prefs.svelte";
import { serverMode } from "./server/client";

registerFonts(import.meta.env.BASE_URL);
initInputModality();
initScheme();
shell.start();
interceptLinks();

// Nothing paints until the active scheme's font family is in: no fallback face is ever drawn. A slow network cannot block start-up
// for more than two seconds, after which the app paints with whatever has arrived (still only the app's own families).
void appearance.start();
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(() => {
  const target = document.getElementById("app")!;
  // The real server needs its start-up checks first (session, protocol, who, spaces: server/boot.ts); fixtures never load any of it.
  const next = serverMode() ? import("./server/boot").then((m) => m.boot()).catch(() => ({ screen: "app" as const })) : Promise.resolve({ screen: "app" as const });
  void next.then(async (b) => {
    if (b.screen === "app") mount(App, { target });
    else {
      const { default: Gate } = await import("./shell/SignIn.svelte");
      mount(Gate, { target, props: b.screen === "signin" ? { needsSetup: b.needsSetup } : { version: b } });
    }
    document.documentElement.dataset.ready = "";
  });
});

// In-page benchmark for browsers that cannot be driven from outside (docs/ui-performance.md): loaded only with ?bench=1.
if (new URLSearchParams(location.search).has("bench")) void document.fonts.ready.then(() => setTimeout(() => import("./bench"), 1500));
