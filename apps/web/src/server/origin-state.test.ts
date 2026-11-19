// The origin's leftovers are removed, but this app's own push worker is not.
import { afterEach, expect, it, vi } from "vitest";
import { dropWorkers } from "./origin-state";

const reg = (script: string) => ({ active: { scriptURL: location.origin + script }, unregister: vi.fn(async () => true) });
afterEach(() => vi.unstubAllGlobals());

it("unregisters the old app's worker and keeps the push worker", async () => {
  const old = reg("/sw.js"), push = reg("/push-sw.js");
  vi.stubGlobal("navigator", { serviceWorker: { getRegistrations: async () => [old, push] } });
  expect(await dropWorkers()).toBe(true);
  expect(old.unregister).toHaveBeenCalled();
  expect(push.unregister).not.toHaveBeenCalled();
});
it("finds nothing to remove when only the push worker is there", async () => {
  vi.stubGlobal("navigator", { serviceWorker: { getRegistrations: async () => [reg("/push-sw.js")] } });
  expect(await dropWorkers()).toBe(false);
});
