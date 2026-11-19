import { beforeEach, describe, expect, test, vi } from "vitest";

// In server mode the spaces list starts as the fixture placeholder; no operation may carry a fixture id, so code that can run before the list arrives (a screen right after sign-up) asks `whenCurrent`.
async function fresh(serverOn: boolean, list: () => Promise<{ spaces: { id: string; name: string; role: string; created: string; personal?: boolean }[] }>) {
  vi.resetModules();
  localStorage.clear(); localStorage.setItem("cal.server", serverOn ? "1" : "0");
  const client = await import("../server/client");
  client.server.spaces.list = list as typeof client.server.spaces.list;
  return import("./spaces.svelte");
}

describe("the current space before the server's list is in", () => {
  beforeEach(() => { vi.restoreAllMocks(); });
  test("server mode: whenCurrent waits for spaces.list and answers the real id, never a fixture's", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    const m = await fresh(true, async () => { await gate; return { spaces: [{ id: "real-space-1", name: "Personal", role: "owner", created: "2026-01-01", personal: true }] }; });
    expect(m.FIXTURE_SPACES.map((s) => s.id)).toContain(m.spaces.current.id); // the placeholder an early reader would have sent
    const got = m.spaces.whenCurrent();
    let done = false; void got.then(() => (done = true));
    await Promise.resolve(); await Promise.resolve();
    expect(done).toBe(false);
    release();
    expect(await got).toBe("real-space-1");
  });
  test("fixtures mode: it answers at once with the fixture space", async () => {
    const m = await fresh(false, async () => { throw new Error("never asked"); });
    expect(await m.spaces.whenCurrent()).toBe(m.FIXTURE_SPACES[0]!.id);
  });
});
