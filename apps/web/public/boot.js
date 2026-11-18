// Runs first, before the app module, as a same-origin classic script (the server's CSP allows no inline script). Finds the colour scheme before the first paint and preloads only its font family
// (no flash, no fallback face); the app paints after the same family loads. Font addresses are resolved against this file, so it works wherever the app is mounted ("/" or "/ai/").
(function () {
  var d = document.documentElement, s = "light";
  try { var v = localStorage.getItem("cal.scheme"); s = v === "dark" || v === "light" ? v : matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"; } catch (e) {}
  d.dataset.scheme = s;
  // The bars (Safari's status bar and toolbar) take the page's edge colour when the page loads and do not look again, so the colour solved last time for this scheme is in place before the first paint (shell/chrome.ts stores it).
  try { var bars = JSON.parse(localStorage.getItem("cal.bars") || "null"); if (bars && /^#[0-9a-f]{6}$/.test(bars[s])) d.style.setProperty("--bars", bars[s]); } catch (e) {}
  // The very first visit (nothing stored): the default wallpaper's own top colour under its veil, solved once on a laptop and written down here.
  if (!d.style.getPropertyValue("--bars")) d.style.setProperty("--bars", s === "dark" ? "#17120e" : "#f3f0ec");
  // A visit with no stored wallpaper choice (or the default chosen) paints the cheap, pre-blurred default straight away; its two addresses are on <html> (vite.config.ts `cheapWallpaper`).
  try {
    var a = JSON.parse(localStorage.getItem("cal.appearance") || "null"), w = d.dataset.wallLight && (s === "dark" ? d.dataset.wallDark : d.dataset.wallLight);
    if (w && (!a || a.wallpaperId === "auto")) { d.dataset.cheap = ""; var im = document.createElement("link"); im.rel = "preload"; im.as = "image"; im.href = w; document.head.appendChild(im); }
  } catch (e) {}
  var here = document.currentScript && document.currentScript.src;
  function pre(f) { var l = document.createElement("link"); l.rel = "preload"; l.as = "font"; l.type = "font/woff2"; l.crossOrigin = "anonymous"; l.href = new URL("fonts/" + f, here || location.href).href; document.head.appendChild(l); }
  pre("google-sans-flex-latin-" + (s === "dark" ? "opsz-grad" : "opsz") + "-normal.woff2");
  pre("google-sans-flex-latin-rond-normal.woff2");
})();
