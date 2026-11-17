import { mount, unmount } from "svelte";
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
// The real server's start-up checks (server/boot.ts) begin at once, in parallel with the fonts, so their chunk and their requests are not queued behind the two-second font wait; fixtures never load any of it.
const boot = serverMode() ? import("./server/boot").then((m) => ({ m, s: m.start() })) : null;
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(async () => {
  const target = document.getElementById("app")!;
  const ready = () => { document.documentElement.dataset.ready = ""; };
  const gate = async (b: Exclude<import("./server/boot").Boot, { screen: "app" }>) => {
    const { default: Gate } = await import("./shell/SignIn.svelte");
    return mount(Gate, { target, props: b.screen === "signin" ? { needsSetup: b.needsSetup } : b.screen === "offline" ? { offline: true } : { version: b } });
  };
  if (!boot) { mount(App, { target }); ready(); return; }
  const { s } = await boot.catch(() => ({ s: { paintNow: true, result: new Promise<never>(() => {}) } }));
  if (s.paintNow) {
    // This device knows the account (a fresh record): the app paints now and the server confirms a round trip later; if it says otherwise the screen turns into the sign-in or version screen.
    const app = mount(App, { target }); ready();
    void s.result.then(async (b) => {
      if (b.screen === "offline") { (await import("./server/link.svelte")).link.failed(); return; }
      if (b.screen !== "app") { unmount(app); await gate(b); }
    });
  } else {
    // A first visit, a signed-out device or an old record: nothing paints until the server has answered (no flash of the wrong screen).
    const b = await s.result.catch(() => ({ screen: "app" as const }));
    if (b.screen === "app") mount(App, { target }); else await gate(b);
    ready();
  }
});

// In-page benchmark for browsers that cannot be driven from outside (docs/ui-performance.md): loaded only with ?bench=1.
if (new URLSearchParams(location.search).has("bench")) void document.fonts.ready.then(() => setTimeout(() => import("./bench"), 1500));
