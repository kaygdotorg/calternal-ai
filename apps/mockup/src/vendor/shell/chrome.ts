/** The browser's bar colour: every `theme-color` meta tag and the root's `--bars` take the colour at the top of the page (the web app's shell/chrome.ts, reduced to what the mockup needs: no native shell). */
export function setThemeColor(color: string): void {
  for (const m of document.querySelectorAll('meta[name="theme-color"]')) if (m.getAttribute("content") !== color) m.setAttribute("content", color);
  if (document.documentElement.style.getPropertyValue("--bars") !== color) document.documentElement.style.setProperty("--bars", color);
}
