import { mount } from "svelte";
import Node from "./Node.svelte";
import { store } from "./store.svelte";
import { initial, page } from "./gen";
import type { FromWorker, ToWorker } from "./protocol";
import "../../../../packages/ui/src/styles/index.css";

type Mode = "worker" | "direct";
const w = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
let mode: Mode = "worker";
const lat: number[] = []; // patch sent (worker clock) -> applied and painted (next frame)
const apply = (m: FromWorker) => {
  const t0 = performance.now();
  for (const p of m.patches) store.apply(p);
  (window as any).__applyMs.push(performance.now() - t0);
  const sent = m.sent - performance.timeOrigin;
  requestAnimationFrame(() => setTimeout(() => lat.push(performance.now() - sent), 0)); // painted ~ after the frame's callbacks and a task
};
w.onmessage = (e: MessageEvent<FromWorker>) => apply(e.data);
store.range = (id, first, end) => { if (mode === "worker") w.postMessage({ ev: "range", id, first, end } satisfies ToWorker); else apply({ patches: [page(id, first, end)], sent: performance.now() + performance.timeOrigin }); };
(window as any).__applyMs = [] as number[]; (window as any).__ph = [] as number[];
store.apply({ op: "tree", root: initial() });
mount(Node, { target: document.getElementById("app")!, props: { id: "root" } });
// Direct baseline: the same generator on the main thread (no worker, no structured clone).
let dtimer = 0;
(window as any).__bench = {
  setMode: (m: Mode) => (mode = m),
  lat: () => lat.splice(0),
  ph: () => (window as any).__ph.splice(0),
  applyMs: () => (window as any).__applyMs.splice(0),
  start(scenario: string) {
    if (mode === "worker") return w.postMessage({ ev: "run", scenario, seq: 0 } satisfies ToWorker);
    clearInterval(dtimer);
    const send = (patches: FromWorker["patches"]) => apply({ patches, sent: performance.now() + performance.timeOrigin });
    if (scenario === "stream") dtimer = window.setInterval(() => send([{ op: "append", id: "msg", text: "The quick brown fox jumps over the lazy dog. " }]), 1000 / 60);
    if (scenario === "resize") { let k = 0; dtimer = window.setInterval(() => { k++; send([{ op: "set", id: "w1", props: { title: "Window 1", w: 420 + (k % 120) } }]); }, 1000 / 60); }
    if (scenario === "window") { let open = false; import("./gen").then(({ windowNode }) => { dtimer = window.setInterval(() => { send(open ? [{ op: "remove", id: "w4" }] : [{ op: "insert", parent: "strip", index: 3, node: windowNode(4) }]); open = !open; }, 250); }); }
  },
  /** Scrollbar-drag jumps: set scrollTop far away and time until no placeholder row is in the viewport (the worker's page has arrived and been drawn). */
  async jumps(n: number) { const el = [...document.querySelectorAll<HTMLElement>('.vl *')].find((e) => e.scrollHeight > e.clientHeight + 50 && /(auto|scroll)/.test(getComputedStyle(e).overflowY))!; const rowH = 44; /* measured: 199,902 px of scroll is row 4,539 */ const out: number[] = []; for (let k = 0; k < n; k++) { const t0 = performance.now(); el.scrollTop = ((k * 7919) % 90) * 4400 + 100000; await new Promise<void>((res) => { const tEnd = performance.now() + 2000; const f = () => { if (performance.now() > tEnd) return res(); const rows = [...document.querySelectorAll<HTMLElement>('.vl .row')]; const want = el.scrollTop / rowH; const near = rows.some((r) => { const m = /^\s*Row (\d+)/.exec(r.textContent ?? ''); return m && Math.abs(+m[1] - want) < 150; }); if (rows.length && near && !rows.some((r) => r.textContent?.trim() === '…')) res(); else requestAnimationFrame(f); }; requestAnimationFrame(f); }); out.push(performance.now() - t0); await new Promise((r) => setTimeout(r, 60)); } return out; },
  stop() { clearInterval(dtimer); w.postMessage({ ev: "stop" } satisfies ToWorker); },
  scrollList(ms: number) { return new Promise<void>((res) => { const sc = document.querySelector(".vl [role=feed], .vl > div, .vl div") as HTMLElement; const el = [...document.querySelectorAll<HTMLElement>(".vl *")].find((e) => e.scrollHeight > e.clientHeight + 50 && /(auto|scroll)/.test(getComputedStyle(e).overflowY)) ?? sc; const t0 = performance.now(); const step = () => { const t = performance.now() - t0; el.scrollTop = t * 6; requestAnimationFrame(() => { const rows = [...document.querySelectorAll<HTMLElement>('.vl .row')]; (window as any).__ph.push(rows.length ? rows.filter((r) => r.textContent?.trim() === '…').length / rows.length : 1); }); if (t < ms) requestAnimationFrame(step); else res(); }; step(); }); },
};
