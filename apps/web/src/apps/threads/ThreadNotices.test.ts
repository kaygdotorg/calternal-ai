import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import ThreadNotices from "./ThreadNotices.svelte";
import type { ThreadNotice, ThreadSummary } from "./model";

const base: ThreadSummary = { id: "t", spaceId: "s", title: "T", parentId: null, state: "idle", workspace: "w", updatedAt: 0, provider: "claude", model: "default", permission: "approve", runsOn: { machine: "dev-02", mode: "sandboxed" }, messageCount: 0 };
const withNotice = (notice?: ThreadNotice): ThreadSummary => ({ ...base, notice });

describe("thread notices: plain words and the next step", () => {
  it("shows nothing when nothing needs attention", () => {
    const { container } = render(ThreadNotices, { thread: base, onresume: () => {} });
    expect(container.querySelector("[role=status]")).toBeNull();
  });
  it("no machine leads to Settings > Machines", async () => {
    render(ThreadNotices, { thread: withNotice({ kind: "no-machine" }), onresume: () => {} });
    expect(screen.getByRole("status", { name: /No machine/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open Machines" })).toBeTruthy();
  });
  it("an offline machine is named, and says nothing moves by itself", () => {
    render(ThreadNotices, { thread: withNotice({ kind: "machine-offline", machine: "dev-02" }), onresume: () => {} });
    expect(screen.getByText(/dev-02 is offline/)).toBeTruthy();
    expect(screen.getByText(/Nothing moves to another machine by itself/)).toBeTruthy();
  });
  it("a provider that is not signed in gives the command to copy", async () => {
    render(ThreadNotices, { thread: withNotice({ kind: "provider-signin", machine: "dev-02", provider: "Claude Code" }), onresume: () => {} });
    expect(screen.getByText(/Claude Code is not signed in on dev-02/)).toBeTruthy();
    expect(screen.getByText("claude auth login")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy the command" })).toBeTruthy();
  });
  it("a failed turn says what Calternal already did and offers Try again", async () => {
    const onresume = vi.fn();
    render(ThreadNotices, { thread: withNotice({ kind: "failed", failure: "transient", reason: "The provider was overloaded" }), onresume });
    expect(screen.getByText(/five times, from 30 seconds up to 8 minutes/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onresume).toHaveBeenCalled();
  });
  it("a waiting retry counts down and shows the attempt", () => {
    render(ThreadNotices, { thread: withNotice({ kind: "retrying", rule: "provider_backoff", reason: "Overloaded", attempt: 3, of: 5, nextAt: Date.now() + 125_000 }), onresume: () => {} });
    expect(screen.getByText(/Trying again in 2:0\d/)).toBeTruthy();
    expect(screen.getByText(/Attempt 3 of 5/)).toBeTruthy();
  });
  it("an interrupted turn offers Resume and says nothing moved", async () => {
    const onresume = vi.fn();
    render(ThreadNotices, { thread: withNotice({ kind: "interrupted", machine: "dev-02", resumable: true }), onresume });
    expect(screen.getByText(/nothing moved to another machine/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Resume" }));
    expect(onresume).toHaveBeenCalled();
  });
  it("has no accessibility violations in any kind", async () => {
    for (const n of [{ kind: "no-machine" }, { kind: "failed", failure: "transient", reason: "x" }, { kind: "provider-signin", machine: "m", provider: "P" }, { kind: "waiting-capacity", queuePosition: 2 }, { kind: "failed", failure: "usage_limit" }, { kind: "retrying", rule: "api_availability", nextAt: Date.now() + 3 * 3600_000 }, { kind: "interrupted", resumable: false }] as ThreadNotice[]) {
      const { container, unmount } = render(ThreadNotices, { thread: withNotice(n), onresume: () => {} });
      const r = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
      expect(r.violations).toEqual([]);
      unmount();
    }
  });
  it("says which rule is waiting: the 3-hourly check, and the plan's reset", () => {
    const { unmount } = render(ThreadNotices, { thread: withNotice({ kind: "retrying", rule: "api_availability", reason: "The provider is not answering", nextAt: Date.now() + 3 * 3600_000 }), onresume: () => {} });
    expect(screen.getByText(/Checking again in 3 hours/)).toBeTruthy();
    expect(screen.getByText(/every 3 hours/)).toBeTruthy();
    unmount();
    render(ThreadNotices, { thread: withNotice({ kind: "retrying", rule: "subscription_reset", nextAt: Date.now() + 90 * 60_000 }), onresume: () => {} });
    expect(screen.getByText(/Continues when your limit resets/)).toBeTruthy();
  });
  it("tells a usage limit from other failures, and a queue from a missing machine", () => {
    const { unmount } = render(ThreadNotices, { thread: withNotice({ kind: "failed", failure: "usage_limit", resetAtMs: Date.now() + 3600_000 }), onresume: () => {} });
    expect(screen.getByText("Your plan’s limit is reached")).toBeTruthy();
    unmount();
    render(ThreadNotices, { thread: withNotice({ kind: "waiting-capacity", queuePosition: 3 }), onresume: () => {} });
    expect(screen.getByText(/you are number 3 in the queue/)).toBeTruthy();
  });
  it("an interrupted turn that cannot be resumed offers no Resume and says why", () => {
    render(ThreadNotices, { thread: withNotice({ kind: "interrupted", resumable: false, why: "It is not known whether the last step finished" }), onresume: () => {} });
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(screen.getByText(/cannot be resumed safely/)).toBeTruthy();
  });
  it("Check again is offered on the sign-in notice when the page can check", async () => {
    const oncheck = vi.fn();
    render(ThreadNotices, { thread: withNotice({ kind: "provider-signin", provider: "Claude Code", machine: "dev-02" }), onresume: () => {}, oncheck });
    await userEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(oncheck).toHaveBeenCalled();
  });
  it("the sign-in notice copies the command the server reported", async () => {
    render(ThreadNotices, { thread: withNotice({ kind: "provider-signin", provider: "Codex", command: "codex login" }), onresume: () => {} });
    expect(screen.getByText("codex login")).toBeTruthy();
  });
});
