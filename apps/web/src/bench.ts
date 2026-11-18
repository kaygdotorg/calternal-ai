/**
 * The in-page benchmark for browsers that cannot be driven from outside (iOS Safari in the Simulator). Load the app with `?bench=1&test=1` on `/notes/n-1`: after a short settle it runs the
 * editor scenarios of `e2e/editor-perf.mjs` (open a short and the long note, type in the long note and in the composer, scroll the long note) and prints the results on screen and in `window.__bench`.
 * Typing goes through `execCommand("insertText")` on the focused field, which is the browser's own text-input path (beforeinput, DOM change, input), not a synthetic call into the editor.
 * No CPU slowdown exists in WebKit, so the numbers are the device's own. Loaded lazily from `main.ts` only when `bench=1` is in the address; nothing of it is in the first paint.
 */
const pct = (xs: number[], p: number) => { const s = [...xs].sort((a, c) => a - c); return s.length ? s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))] : NaN; };
const stats = (xs: number[]) => ({ n: xs.length, p50: +pct(xs, 0.5).toFixed(1), p95: +pct(xs, 0.95).toFixed(1), worst: +Math.max(...xs).toFixed(1) });
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const frame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
const frames2 = async () => { await frame(); await frame(); };
const until = (test: () => boolean) => new Promise<void>((res) => { const iv = setInterval(() => { if (test()) { clearInterval(iv); res(); } }, 1); });

const out: Record<string, unknown> = { ua: navigator.userAgent, dpr: devicePixelRatio, viewport: `${innerWidth}x${innerHeight}` };
const pre = document.createElement("pre");
pre.id = "bench";
pre.style.cssText = "position:fixed;left:0;right:0;top:0;z-index:99999;margin:0;padding:6px 8px;background:#000e;color:#9f9;font:10px/1.3 ui-monospace,monospace;white-space:pre-wrap;pointer-events:none";
const show = () => { pre.textContent = JSON.stringify(out).replace(/[{}"]/g, "").replace(/,(?=[a-zA-Z]+:)/g, " ").replace(/ua:[^,]*,/, ""); (window as unknown as { __bench: unknown }).__bench = out; };

async function typeInto(el: HTMLElement, text: string, gap = 60): Promise<{ keys: number[]; work: number[] }> {
  const keys: number[] = [], work: number[] = [];
  el.focus();
  for (const ch of text) {
    const t = performance.now();
    document.execCommand("insertText", false, ch);
    queueMicrotask(() => queueMicrotask(() => work.push(performance.now() - t)));
    requestAnimationFrame(() => keys.push(performance.now() - t));
    await sleep(gap);
  }
  await sleep(300);
  return { keys, work };
}

async function run() {
  document.body.append(pre);
  out.status = "running";
  show();
  await until(() => !!document.querySelector(".cm-content"));
  await sleep(800);
  const notes = () => [...document.querySelectorAll<HTMLAnchorElement>('nav[aria-label="Notes"] a.row')];
  // 1. Open: a short note, then the long one (warm: a click in the list).
  {
    const t0 = performance.now();
    notes().find((a) => a.getAttribute("href")?.endsWith("n-2"))?.click();
    await until(() => !!document.querySelector(".cm-content")?.textContent?.includes("Three roots"));
    await frames2();
    out.openShortMs = +(performance.now() - t0).toFixed(0);
  }
  show();
  const opens: number[] = [];
  for (let i = 0; i < 5; i++) {
    const long = notes().find((a) => a.textContent?.includes("The long note"));
    const other = notes().find((a) => !a.textContent?.includes("The long note"));
    other?.click(); await sleep(400);
    const t0 = performance.now();
    long?.click();
    await until(() => !!document.querySelector(".cm-content")?.textContent?.includes("Section 1"));
    await frames2();
    opens.push(performance.now() - t0);
    await sleep(300);
  }
  out.openLongWarm = stats(opens);
  show();
  // 2. Typing in the long note, at its end: key press to the next frame, and the script work.
  await sleep(800);
  const cm = document.querySelector<HTMLElement>(".cm-content")!;
  cm.focus();
  const sel = getSelection() as Selection & { modify?: (a: string, b: string, c: string) => void };
  sel?.modify?.("move", "forward", "documentboundary");
  await sleep(500);
  const typed = await typeInto(cm, "the quick brown fox jumps over the lazy dog, then naps. ");
  out.typeLongEnd = { keys: stats(typed.keys), work: stats(typed.work) };
  show();
  // 3. Scrolling the long note: frame times over 100 steps down and 100 up.
  const sc = document.querySelector<HTMLElement>(".note .scroll")!;
  const deltas: number[] = [];
  let last = performance.now();
  for (let i = 0; i < 200; i++) {
    sc.scrollTop += i < 100 ? 800 : -800;
    await frame();
    const n = performance.now(); deltas.push(n - last); last = n;
  }
  out.scroll = { ...stats(deltas), over50: deltas.filter((d) => d > 50).length };
  show();
  // 4. Typing in the composer (a plain field with the palette engine on it).
  history.pushState({}, "", "/threads/t-idle?bench=1&test=1");
  dispatchEvent(new PopStateEvent("popstate"));
  await until(() => !!document.querySelector("textarea"));
  await sleep(800);
  const ta = document.querySelector<HTMLTextAreaElement>("textarea")!;
  const c = await typeInto(ta, "the quick brown fox jumps over the lazy dog, then naps. ");
  out.typeComposer = { keys: stats(c.keys), work: stats(c.work) };
  out.status = "done";
  (document.activeElement as HTMLElement | null)?.blur();
  show();
}
run().catch((e) => { out.status = "error: " + String(e); show(); });

export {};
