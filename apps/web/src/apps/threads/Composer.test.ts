import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import Composer from "./Composer.svelte";
import type { ProviderOption, ThreadSummary } from "./model";

const thread: ThreadSummary = { id: "t", title: "T", parentId: null, state: "idle", workspace: "w", updatedAt: 0, provider: "claude", model: "sonnet-5-5", permission: "approve", runsOn: { machine: "dev-02", mode: "sandboxed" }, messageCount: 0 };
const providers: ProviderOption[] = [{ id: "claude", name: "Claude Code", models: [{ id: "sonnet-5-5", name: "Sonnet 5.5" }, { id: "opus-5-5", name: "Opus 5.5" }] }];
const props = (o = {}) => ({ thread, providers, machines: ["dev-02", "laptop"], onsend() {}, onstop() {}, onprovider() {}, onpermission() {}, onrunson() {}, ...o });

describe("composer", () => {
  it("shows permission mode and where it runs as two separate chips", () => {
    render(Composer, props());
    expect(screen.getByRole("button", { name: "Approve for me" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "dev-02 · Sandboxed" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Claude Code · Sonnet 5.5" })).toBeTruthy();
  });
  it("changing permission is its own menu and does not touch where it runs", async () => {
    const onpermission = vi.fn();
    const onrunson = vi.fn();
    render(Composer, props({ onpermission, onrunson }));
    await userEvent.click(screen.getByRole("button", { name: "Approve for me" }));
    const menu = screen.getByRole("menu", { name: "Permission mode" });
    expect([...menu.querySelectorAll("[role=menuitemradio]")].map((e) => e.textContent?.trim().split("\n")[0])).toEqual(["Least access", "Approve for me", "Full access"].map((l) => expect.stringContaining(l)) as never);
    await userEvent.click(screen.getByRole("menuitemradio", { name: /Full access/ }));
    expect(onpermission).toHaveBeenCalledWith("full");
    expect(onrunson).not.toHaveBeenCalled();
  });
  it("a mode picked applies at once, wider or narrower, with no confirmation, and says it is for the next turn while one runs", async () => {
    const onpermission = vi.fn();
    render(Composer, props({ onpermission, thread: { ...thread, state: "running", permission: "least" } }));
    await userEvent.click(screen.getByRole("button", { name: "Least access, applies to the next turn" }));
    expect(screen.getByRole("menu", { name: "Permission mode for the next turn" })).toBeTruthy();
    await userEvent.click(screen.getByRole("menuitemradio", { name: /Full access/ }));
    expect(onpermission).toHaveBeenCalledWith("full");
    expect(screen.queryByRole("dialog")).toBeNull(); // no "are you sure"
  });
  it("where it runs is two choices, a machine then Sandboxed or Direct, in one picker", async () => {
    const onrunson = vi.fn();
    render(Composer, props({ onrunson }));
    await userEvent.click(screen.getByRole("button", { name: "dev-02 · Sandboxed" }));
    const picker = screen.getByRole("dialog", { name: "Where it runs" });
    expect(within(picker).getByRole("radiogroup", { name: "Machine" })).toBeTruthy();
    await userEvent.click(within(picker).getByRole("radio", { name: "laptop" }));
    expect(onrunson).toHaveBeenLastCalledWith({ machine: "laptop", mode: "sandboxed" });
    await userEvent.click(within(picker).getByRole("radio", { name: /Direct/ }));
    expect(onrunson).toHaveBeenLastCalledWith({ machine: "dev-02", mode: "direct" });
  });
  it("Enter sends, Shift+Enter does not, and the send button is named", async () => {
    const onsend = vi.fn();
    render(Composer, props({ onsend }));
    const field = screen.getByLabelText("Message");
    await userEvent.type(field, "hello{Shift>}{Enter}{/Shift}there");
    expect(onsend).not.toHaveBeenCalled();
    await userEvent.type(field, "{Enter}");
    expect(onsend).toHaveBeenCalledWith("hello\nthere");
  });
  it("has no accessibility violations", async () => {
    const { container } = render(Composer, props());
    expect((await axe.run(container, { rules: { "color-contrast": { enabled: false } } })).violations.map((v) => v.id)).toEqual([]);
  });
});
