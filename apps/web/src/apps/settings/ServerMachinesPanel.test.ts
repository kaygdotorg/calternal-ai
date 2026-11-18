import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { server } from "../../server/client";
import ServerMachinesPanel from "./ServerMachinesPanel.svelte";

// Recorded shapes: GET /api/v1/machines and `agent.readiness` / `agent.check` (docs/agent-turns.md, "Machine and provider readiness").
const MACHINE = { id: "m-1", name: "build-01", os: "linux", arch: "x86_64", direct: false, state: "online" };
const calls: { op: string; input: Record<string, unknown> }[] = [];
let signedIn = false;
beforeEach(() => {
  calls.length = 0; signedIn = false;
  server.json = (async (path: string) => (path.endsWith("/machines") ? { machines: [MACHINE] } : {})) as typeof server.json;
  server.accountLive = (() => new Promise<void>(() => {})) as typeof server.accountLive; // the feed stays open
  server.operate = (async (op: string, input: Record<string, unknown> = {}) => {
    calls.push({ op, input });
    if (op === "agent.readiness") return { check_ttl_ms: 300000, machines: [{ machine_id: "m-1", name: "build-01", enrolled: true, connected: true, available: true, execution: "sandboxed", last_seen_ms: 1, providers: { "calternal-claude-code": { enabled: true, ready: signedIn, check: { installed: true, signed_in: signedIn, sign_in_command: "claude auth login" } } } }] };
    if (op === "agent.check") { signedIn = true; return {}; }
    return {};
  }) as typeof server.operate;
});

describe("Machines page: provider readiness", () => {
  it("shows a provider that is not signed in with the official command to copy, and Check again refreshes it", async () => {
    render(ServerMachinesPanel);
    expect(await screen.findByText("Claude Code")).toBeTruthy();
    expect(await screen.findByText(/Not signed in/)).toBeTruthy();
    expect(screen.getByText("claude auth login")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy command" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(calls.some((c) => c.op === "agent.check" && c.input.machine_id === "m-1" && c.input.provider_id === "calternal-claude-code")).toBe(true));
    expect(await screen.findByText("Signed in")).toBeTruthy();
    expect(screen.queryByText("claude auth login")).toBeNull();
  });
});
