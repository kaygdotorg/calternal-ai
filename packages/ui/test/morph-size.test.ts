/**
 * The bar capsule follows the layer it shows now (lib/morph-size.ts). The case that failed in the
 * Attachments e2e (pictures.spec.ts:95): a sheet closes, the tools layer mounts, and the observer's call
 * for it is late or lost, so the capsule stayed at the sheet's size and covered the page.
 */
import { describe, expect, test } from "vitest";
import { currentLayer, followLayer, measureLayer, type LayerEl, type Size } from "../src/lib/morph-size";

/** A capsule with a hand-driven observer and frame, so a test chooses when (and whether) each call lands. */
function bar() {
  let key = "sheet-insert";
  let size: Size | null = null;
  const calls: (() => void)[] = [];
  const frames: (() => void)[] = [];
  const layer = (k: string, w: number, h: number): LayerEl => ({ dataset: { key: k }, offsetWidth: w, offsetHeight: h, isConnected: true });
  const follow = (el: LayerEl) =>
    followLayer(el, {
      key: () => key,
      size: () => size,
      apply: (n) => (size = n),
      observe: (_el, call) => (calls.push(call), () => undefined),
      frame: (call) => (frames.push(call), () => undefined),
    });
  return { layer, follow, setKey: (k: string) => (key = k), size: () => size, apply: (n: Size) => (size = n), calls, frames };
}

describe("morph size", () => {
  test("a layer is measured as it mounts, with no observer call", () => {
    const b = bar();
    b.follow(b.layer("sheet-insert", 340, 580));
    expect(b.size()).toEqual({ w: 340, h: 580 });
  });

  test("sheet → toolbar: the capsule ends at the toolbar's size when the observer call never lands", () => {
    const b = bar();
    b.follow(b.layer("sheet-insert", 340, 580));
    b.setKey("tools");
    b.follow(b.layer("tools", 300, 44));
    expect(b.size()).toEqual({ w: 300, h: 44 });
  });

  test("sheet → toolbar: a layer that is not laid out yet is picked up in the next frame", () => {
    const b = bar();
    b.follow(b.layer("sheet-insert", 340, 580));
    b.setKey("tools");
    const tools = b.layer("tools", 0, 0);
    b.follow(tools);
    Object.assign(tools, { offsetWidth: 300, offsetHeight: 44 });
    b.frames.forEach((f) => f());
    expect(b.size()).toEqual({ w: 300, h: 44 });
  });

  test("a late observer call of the sheet layer cannot put the sheet's size back", () => {
    const b = bar();
    const sheet = b.layer("sheet-insert", 340, 580);
    b.follow(sheet);
    b.setKey("tools");
    b.follow(b.layer("tools", 300, 44));
    b.calls[0]();
    expect(b.size()).toEqual({ w: 300, h: 44 });
  });

  test("a removed layer sets nothing", () => {
    const b = bar();
    const sheet = b.layer("sheet-insert", 340, 580);
    b.follow(sheet);
    sheet.isConnected = false;
    Object.assign(sheet, { offsetWidth: 10, offsetHeight: 10 });
    b.calls[0]();
    expect(b.size()).toEqual({ w: 340, h: 580 });
  });

  test("the layer's own action never ran: the capsule still takes the size of the layer the key shows", () => {
    const b = bar();
    b.follow(b.layer("sheet-insert", 340, 580));
    b.setKey("tools");
    // The e2e failure: the new tools layer is in the page next to a leftover tabs layer, and nothing follows it.
    const parent = { children: [b.layer("tabs", 102, 44), b.layer("tools", 258, 44)] };
    const el = currentLayer(parent, "tools")!;
    measureLayer(el, { key: () => "tools", size: b.size, apply: (n) => b.apply(n) });
    expect(b.size()).toEqual({ w: 258, h: 44 });
  });

  test("currentLayer picks the last layer with the key, and none for an unknown key", () => {
    const b = bar();
    const old = b.layer("tools", 1, 1);
    const fresh = b.layer("tools", 2, 2);
    expect(currentLayer({ children: [old, b.layer("tabs", 3, 3), fresh] }, "tools")).toBe(fresh);
    expect(currentLayer({ children: [old] }, "sheet-insert")).toBeUndefined();
    expect(currentLayer(undefined, "tools")).toBeUndefined();
  });

  test("an equal size applies nothing", () => {
    const b = bar();
    let applied = 0;
    const el = b.layer("sheet-insert", 340, 580);
    followLayer(el, { key: () => "sheet-insert", size: () => ({ w: 340, h: 580 }), apply: () => applied++, observe: () => () => undefined, frame: () => () => undefined });
    expect(applied).toBe(0);
  });
});
