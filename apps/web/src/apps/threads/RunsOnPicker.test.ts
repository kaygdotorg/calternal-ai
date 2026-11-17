import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import RunsOnPicker from "./RunsOnPicker.svelte";

const machines = ["dev-02", "dev-01", "laptop"];

describe("where-it-runs picker", () => {
  it("marks the current machine and mode, and says what the mode means", () => {
    render(RunsOnPicker, { machines, value: { machine: "dev-01", mode: "sandboxed" }, onchange() {} });
    expect(screen.getByRole("radio", { name: "dev-01" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("radio", { name: "Sandboxed" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText(/own sandbox/)).toBeTruthy();
  });
  it("changing the machine keeps the mode and changing the mode keeps the machine", async () => {
    const onchange = vi.fn();
    render(RunsOnPicker, { machines, value: { machine: "dev-02", mode: "direct" }, onchange });
    await userEvent.click(screen.getByRole("radio", { name: "laptop" }));
    expect(onchange).toHaveBeenLastCalledWith({ machine: "laptop", mode: "direct" });
    await userEvent.click(screen.getByRole("radio", { name: "Sandboxed" }));
    expect(onchange).toHaveBeenLastCalledWith({ machine: "dev-02", mode: "sandboxed" });
  });
  it("direct warns that there is no sandbox", () => {
    render(RunsOnPicker, { machines, value: { machine: "dev-02", mode: "direct" }, onchange() {} });
    expect(screen.getByText(/no sandbox/)).toBeTruthy();
  });
  it("has no accessibility violations", async () => {
    const { container } = render(RunsOnPicker, { machines, value: { machine: "dev-02", mode: "sandboxed" }, onchange() {} });
    expect((await axe.run(container, { rules: { "color-contrast": { enabled: false } } })).violations.map((v) => v.id)).toEqual([]);
  });
});
