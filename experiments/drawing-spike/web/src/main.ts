import { mount } from "svelte";
import "@calternal/ui/styles";
import { applyScheme, currentScheme, initInputModality, initScheme, loadFonts, registerFonts } from "@calternal/ui";
import App from "./App.svelte";

registerFonts(import.meta.env.BASE_URL);
initInputModality();
initScheme();
const forced = new URLSearchParams(location.search).get("scheme");
if (forced === "light" || forced === "dark") applyScheme(forced);
Promise.race([loadFonts(currentScheme()).catch(() => {}), new Promise((r) => setTimeout(r, 2000))]).then(() => {
  mount(App, { target: document.getElementById("app")! });
  document.documentElement.dataset.ready = "";
});
