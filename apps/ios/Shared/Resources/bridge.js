// Injected into every page at document start. Gives the web app `window.calternalNative`, the only
// door to native features; on the web (no shell) the object does not exist, so pages feature-detect:
//   if (window.calternalNative) calternalNative.haptic('success');
// Reference: docs/ios-shell.md. Every call returns a Promise (resolved when the shell has acted).
(() => {
  if (window.calternalNative) return;
  const send = (op, args) => window.webkit.messageHandlers.calternal.postMessage({ op, ...args });
  window.calternalNative = Object.freeze({
    /** Bridge version; bump when an op changes meaning. */
    version: 1,
    /** 'ios' or 'macos'. */
    platform: window.__calternalPlatform || 'ios',
    /** Optional features of this shell: 'window-material' (Mac: the Appearance setting that chooses wallpaper, frost or glass under the window). */
    capabilities: window.__calternalCapabilities || [],
    /** Tactile feedback: 'selection' | 'light' | 'medium' | 'heavy' | 'soft' | 'rigid' | 'success' | 'warning' | 'error'. */
    haptic: (style) => send('haptic', { style }),
    /** Tell the shell what the page looks like: { scheme: 'light'|'dark', background: '#rrggbb' }.
     *  scheme drives the status bar; background the window behind the page. Call on theme change. */
    appearance: (o) => send('appearance', o),
    /** The first real frame is painted: the shell removes its launch cover. Call once per load. */
    ready: () => send('ready', {}),
    /** System share sheet: { text?, url?, anchor?: {x,y,width,height} } -> { completed: boolean }. */
    share: (o) => send('share', o),
    /** Native menu at a point: { anchor: {x,y,width,height}, title?, items: [{id,title,symbol?,destructive?,disabled?}] }
     *  -> { id: string | null } (null when dismissed). */
    menu: (o) => send('menu', o),
    /** Keep unsaved state (a JSON-able object, at most 64 KB) the shell hands back after iOS ends the page's process. */
    saveState: (state) => send('saveState', { state }),
    /** Chrome description versions this shell draws (from the shell, e.g. [1]). Publish only a version in the list; on no match keep drawing your own chrome. */
    chromeVersions: window.__calternalChromeVersions || [],
    /** Experimental native chrome: publish the chrome description (docs/ios-shell.md, "Chrome description") or null to withdraw
     *  -> { native: boolean } (false: switch off, keep drawing your own). Native events arrive as 'calternal:chrome'. */
    chrome: (description) => send('chrome', { chrome: description }),
    /** Round-trip test: resolves { t } (shell clock, ms) when the shell has answered. */
    ping: () => send('ping', {}),
    /** Writes a line to the app's log (development). */
    log: (message) => send('log', { message: String(message) }),
    /** Experimental native glass bar: { items: [{id,title,symbol}], selected: id } or null to remove -> { native: boolean }
     *  (false: the switch is off, draw your own). Taps arrive as the 'calternal:bar' event; --native-bar-inset is set on :root while shown. */
    bar: (o) => send('bar', { bar: o }),
  });
})();
