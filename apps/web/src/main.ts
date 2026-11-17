import { mount } from "svelte";
import "@calternal/ui/styles";
import { currentScheme, initInputModality, initScheme, loadFonts, registerFonts } from "@calternal/ui";
import App from "./App.svelte";
import { interceptLinks } from "./lib/route.svelte";
import { shell } from "./shell/shell.svelte";
import { appearance } from "./appearance/prefs.svelte";
import { devToken, serverMode } from "./server/client";

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
  // The real server without a development token needs a session: ask once, and show the sign-in (or first-owner setup) screen instead of the app. Fixtures never load this.
  const gate = serverMode() && !devToken() ? import("./server/auth").then((a) => a.status()).catch(() => null) : Promise.resolve(null);
  void gate.then(async (st) => {
    if (st && !st.signed_in) {
      const { default: SignIn } = await import("./shell/SignIn.svelte");
      mount(SignIn, { target, props: { needsSetup: st.setup } });
    } else {
      // With the real server the app starts knowing its spaces (an adapter reads the current space the moment it starts); a slow or failing server does not hold it back for long.
      if (serverMode()) await Promise.race([import("./shell/spaces.svelte").then((m) => m.spaces.load()).catch((e) => console.error("spaces:", e?.code, e?.message)) , new Promise((r) => setTimeout(r, 4000))]);
      mount(App, { target });
    }
    document.documentElement.dataset.ready = "";
  });
});

// In-page benchmark for browsers that cannot be driven from outside (docs/ui-performance.md): loaded only with ?bench=1.
if (new URLSearchParams(location.search).has("bench")) void document.fonts.ready.then(() => setTimeout(() => import("./bench"), 1500));
