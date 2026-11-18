import type { FromWorker, ToWorker } from "./protocol";
import { page, windowNode } from "./gen";
const send = (patches: FromWorker["patches"], seq?: number) => (self as unknown as Worker).postMessage({ patches, sent: performance.now() + performance.timeOrigin, seq } satisfies FromWorker);
let timer = 0, w = 4;
const chunk = "The quick brown fox jumps over the lazy dog. ".repeat(1);
onmessage = (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  if (m.ev === "range") send([page(m.id, m.first, m.end)], undefined);
  else if (m.ev === "stop") clearInterval(timer);
  else if (m.ev === "run") {
    clearInterval(timer);
    if (m.scenario === "stream") { let i = 0; timer = self.setInterval(() => { send([{ op: "append", id: "msg", text: chunk }], i++); }, 1000 / 60); }
    if (m.scenario === "window") { let open = false; const id = `w${w}`; timer = self.setInterval(() => { if (!open) { send([{ op: "insert", parent: "strip", index: 3, node: windowNode(w) }], 1); open = true; } else { send([{ op: "remove", id }], 2); open = false; } }, 250); }
    if (m.scenario === "resize") { let k = 0; timer = self.setInterval(() => { k++; send([{ op: "set", id: "w1", props: { title: "Window 1", w: 420 + (k % 120) } }], k); }, 1000 / 60); }
  }
};
