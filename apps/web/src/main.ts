import { mount, unmount } from "svelte";
import "@calternal/ui/styles";
import { applySheetRadius, currentScheme, initInputModality, initScheme, loadFonts, registerFonts } from "@calternal/ui";
import App from "./App.svelte";
import { nativeReady } from "./shell/native";
import "./shell/chrome/native.css";
import { startNativeChrome, registerChrome } from "./shell/chrome/native-chrome.svelte";
import { threadsChrome } from "./apps/threads/chrome";
import { notesChrome } from "./apps/notes/chrome";
import { settingsChrome } from "./apps/settings/chrome";
import { appFor } from "./apps/registry";
import { segments } from "./lib/route.svelte";
import { interceptLinks } from "./lib/route.svelte";
import { shell } from "./shell/shell.svelte";
import { appearance } from "./appearance/prefs.svelte";
import { serverMode } from "./server/client";
import { startChrome } from "./shell/chrome";
import "./canvas/area"; // the canvas is the shell's window area (shell/window-area.ts)

registerFonts(import.meta.env.BASE_URL);
initInputModality();
applySheetRadius(new URLSearchParams(location.search).get("sheetcorners") === "device"); // an iPhone gets its display corners on sheets; `?sheetcorners=device` shows them anywhere (preview, tests)
initScheme();
shell.start();
interceptLinks();

// Nothing paints until the active scheme's font family is in: no fallback face is ever drawn. A slow network cannot block start-up
// for more than two seconds, after which the app paints with whatever has arrived (still only the app's own families).
void appearance.start();
// The real server's start-up checks (server/boot.ts) begin at once, in parallel with the fonts, so their chunk and their requests are not queued behind the two-second font wait; fixtures never load any of it.
// A guest link (`/s/<secret>`) is its own page, before and without any sign-in: the start-up checks (and their clean-up of other installations' leftovers) do not run for it, so nothing can reload or wipe the visit.
const guestPage = /^\/s\//.test(location.pathname.slice(import.meta.env.BASE_URL.replace(/\/$/, "").length));
const boot = serverMode() && !guestPage ? import("./server/boot").then((m) => ({ m, s: m.start() })) : null;
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(async () => {
  const target = document.getElementById("app")!;
  // A link someone shared (`/s/<secret>`) is the guest page: only that item, read-only, before and without any sign-in.
  if (guestPage) {
    const { default: Guest } = await import("./guest/Guest.svelte");
    mount(Guest, { target }); document.documentElement.dataset.ready = ""; return;
  }
  const ready = () => { document.documentElement.dataset.ready = ""; startChrome(); requestAnimationFrame(() => requestAnimationFrame(() => { nativeReady(); registerChrome("threads", threadsChrome); registerChrome("notes", notesChrome); registerChrome("settings", settingsChrome); startNativeChrome(() => appFor(segments()[0])?.kind ?? "threads"); })); /* the first frame is painted: the native shell (if any) lifts its launch cover */ };
  const gate = async (b: Exclude<import("./server/boot").Boot, { screen: "app" }>) => {
    // A fresh installation or an invitation is the stepped onboarding (who you are first, then the rest); a returning person signing in is the plain card; a reload part-way resumes the onboarding.
    if (b.screen === "onboarding" || (b.screen === "signin" && (b.needsSetup || b.invite))) {
      const { default: Onboarding } = await import("./shell/onboarding/Onboarding.svelte");
      return mount(Onboarding, { target, props: { who: b.screen === "signin" && !b.needsSetup ? "invited" : "owner", token: b.screen === "signin" ? (b.invite ?? "") : "" } });
    }
    if (b.screen === "email") { const { default: EmailGate } = await import("./shell/EmailGate.svelte"); return mount(EmailGate, { target, props: { mail: b.mail } }); }
    const { default: Gate } = await import("./shell/SignIn.svelte");
    return mount(Gate, { target, props: b.screen === "signin" ? { needsSetup: b.needsSetup, invite: b.invite } : b.screen === "offline" ? { offline: true } : { version: b } });
  };
  if (!boot) {
    // Fixtures: the onboarding can be looked at and tested with ?onboarding=1 (the owner's five steps) or ?onboarding=invited (three), against a simulated machine; the passkey is pretended.
    const demo = new URLSearchParams(location.search).get("onboarding");
    if (demo === "1" || demo === "invited") {
      const { default: Onboarding } = await import("./shell/onboarding/Onboarding.svelte");
      mount(Onboarding, { target, props: { who: demo === "invited" ? "invited" : "owner", demo: true, attached: new URLSearchParams(location.search).has("attached") } }); ready(); return;
    }
    // Fixtures: the sign-in card can be looked at with ?signin=1 (nothing is sent anywhere).
    if (new URLSearchParams(location.search).get("signin") === "1") {
      const { default: Gate } = await import("./shell/SignIn.svelte");
      mount(Gate, { target, props: {} }); ready(); return;
    }
    mount(App, { target }); ready(); return;
  }
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
