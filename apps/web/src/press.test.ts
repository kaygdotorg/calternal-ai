// The row press's timeline (packages/ui/src/lib/press.ts): a hold of half a second opens the press (a touch); a finger that moves, lifts early or is cancelled opens nothing; a mouse right-click and
// the menu key open the menu alone, with no hold.
import { afterEach, beforeEach, expect, test, vi } from "vitest";
vi.mock("svelte", async (orig) => ({ ...(await orig<typeof import("svelte")>()), mount: vi.fn() }));
vi.mock("../../../packages/ui/src/components/PressSurface.svelte", () => ({ default: {} }));
import { LIFT_MS, MENU_MS, PRIME_MS, pressRow, pressRows, STILL } from "../../../packages/ui/src/lib/press";
import { pressState } from "../../../packages/ui/src/lib/press-state.svelte";

let list: HTMLElement, row: HTMLElement, stop: { destroy(): void };
const bag = () => ({ label: "Row", items: () => [{ id: "a", label: "A" }], onpick: () => {}, preview: { kind: "t", item: 1 } });
const ptr = (type: string, o: { x?: number; y?: number; kind?: string } = {}) => {
  const e = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: o.x ?? 10, clientY: o.y ?? 10 });
  Object.defineProperties(e, { pointerType: { value: o.kind ?? "touch" }, isPrimary: { value: true } });
  return e;
};
const settle = async () => { await vi.advanceTimersByTimeAsync(0); await Promise.resolve(); await vi.advanceTimersByTimeAsync(0); };

beforeEach(() => {
  vi.useFakeTimers();
  pressState.cur = null;
  list = document.createElement("div"); row = document.createElement("div"); list.append(row); document.body.append(list);
  pressRow(row, bag); stop = pressRows(list);
  (HTMLElement.prototype as { animate?: unknown }).animate = () => ({ reverse() {}, cancel() {} });
});
afterEach(() => { stop.destroy(); list.remove(); vi.useRealTimers(); });

test("a touch held for half a second opens the press with the preview", async () => {
  row.dispatchEvent(ptr("pointerdown"));
  await vi.advanceTimersByTimeAsync(MENU_MS - 1);
  expect(pressState.cur).toBeNull();
  await vi.advanceTimersByTimeAsync(2); await settle();
  expect(pressState.cur?.preview).toBe(true);
});

test("a finger that moves more than a little first is a scroll: nothing opens", async () => {
  row.dispatchEvent(ptr("pointerdown"));
  await vi.advanceTimersByTimeAsync(200);
  row.dispatchEvent(ptr("pointermove", { y: 10 + STILL + 2 }));
  await vi.advanceTimersByTimeAsync(MENU_MS * 2); await settle();
  expect(pressState.cur).toBeNull();
});

test("a finger lifted or cancelled before the half second opens nothing", async () => {
  row.dispatchEvent(ptr("pointerdown")); await vi.advanceTimersByTimeAsync(300); row.dispatchEvent(ptr("pointerup"));
  await vi.advanceTimersByTimeAsync(MENU_MS * 2); await settle();
  expect(pressState.cur).toBeNull();
  row.dispatchEvent(ptr("pointerdown")); await vi.advanceTimersByTimeAsync(300); row.dispatchEvent(ptr("pointercancel"));
  await vi.advanceTimersByTimeAsync(MENU_MS * 2); await settle();
  expect(pressState.cur).toBeNull();
});

test("a mouse right-click opens the menu alone, at once", async () => {
  row.dispatchEvent(ptr("pointerdown", { kind: "mouse" }));
  const e = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 30, clientY: 40 });
  row.dispatchEvent(e); await settle();
  expect(e.defaultPrevented).toBe(true);
  expect(pressState.cur?.preview).toBe(false);
  expect([pressState.cur?.x, pressState.cur?.y]).toEqual([30, 40]);
});

test("the menu key on a focused row opens the menu alone", async () => {
  row.dispatchEvent(new KeyboardEvent("keydown", { key: "ContextMenu", bubbles: true, cancelable: true })); await settle();
  expect(pressState.cur?.preview).toBe(false);
});

test("the held row is staged a little into the hold, before the menu opens, and the lift stands over it", async () => {
  row.dispatchEvent(ptr("pointerdown"));
  await vi.advanceTimersByTimeAsync(PRIME_MS + 5); await settle();
  expect(pressState.stage?.row).toBe(row);
  expect(pressState.cur).toBeNull();
  await vi.advanceTimersByTimeAsync(LIFT_MS - PRIME_MS + 5);
  expect(row.style.visibility).toBe("hidden");
  expect(document.body.lastElementChild).not.toBe(list);
});

test("a press that ends before the half second unstages the card and shows the row again", async () => {
  row.dispatchEvent(ptr("pointerdown"));
  await vi.advanceTimersByTimeAsync(LIFT_MS + 20); await settle();
  row.dispatchEvent(ptr("pointerup")); await vi.advanceTimersByTimeAsync(400);
  expect(pressState.stage).toBeNull();
  expect(row.style.visibility).toBe("");
});

test("a row that is not registered is not pressable", async () => {
  const plain = document.createElement("div"); list.append(plain);
  plain.dispatchEvent(ptr("pointerdown")); await vi.advanceTimersByTimeAsync(MENU_MS + 10); await settle();
  expect(pressState.cur).toBeNull();
});
