import { afterEach, describe, expect, test, vi } from "vitest";
import { flushSync } from "svelte";
import { nativeChrome, registerChrome, routeEvent, startNativeChrome } from "./native-chrome.svelte";

const w = window as unknown as { calternalNative?: unknown };
const st = $state({ selected: "r1", count: 10, title: "One" });
registerChrome("pub-test", {
  describe: () => ({ header: { title: st.title }, selectors: [], search: { placeholder: "Find" }, sections: [{ id: "s", title: null, count: st.count, rows: ["r1", "r2"].map((id) => ({ id, title: id, selected: st.selected === id })) }] }),
  handle: () => true,
});
const frame = () => new Promise((r) => requestAnimationFrame(() => r(null)));
afterEach(() => { delete w.calternalNative; nativeChrome.native = false; });

describe("native chrome: what is worth a publication", () => {
  test("the echo of the shell's own selection is not sent; a title change or a held-back count is", async () => {
    const chrome = vi.fn().mockResolvedValue({ native: true });
    w.calternalNative = { chrome, log: vi.fn(), chromeVersions: [1] } as never;
    const stop = startNativeChrome(() => "pub-test");
    flushSync(); await frame(); await Promise.resolve();
    expect(chrome).toHaveBeenCalledTimes(1);
    // the shell chose r2: the page's selected flags follow, and nothing goes out
    routeEvent("pub-test", { type: "row", id: "r2", revision: 1, sentAt: 0 });
    st.selected = "r2"; flushSync(); await frame(); await Promise.resolve();
    expect(chrome).toHaveBeenCalledTimes(1);
    // the title is new information for the shell: sent (with the selection)
    st.title = "Two"; flushSync(); await frame(); await Promise.resolve();
    expect(chrome).toHaveBeenCalledTimes(2);
    // while rows keep arriving (a key repeating) counts that move are held to one a second
    routeEvent("pub-test", { type: "row", id: "r2", revision: 2, sentAt: 0 });
    for (let n = 11; n < 16; n++) { st.count = n; flushSync(); await frame(); await Promise.resolve(); }
    expect(chrome).toHaveBeenCalledTimes(2);
    stop();
  });
});
