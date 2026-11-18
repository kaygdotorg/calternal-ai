import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import Switch from "../src/components/Switch.svelte";

describe("Switch", () => {
  it("is a switch named by its required label, off by default", () => {
    render(Switch, { label: "Animations" });
    const s = screen.getByRole("switch", { name: "Animations" });
    expect(s.getAttribute("aria-checked")).toBe("false");
  });
  it("a click, and the label, toggle it and report the new state", async () => {
    const onchange = vi.fn();
    render(Switch, { label: "Blur", hint: "Frosts the picture", onchange });
    const s = screen.getByRole("switch", { name: /Blur/ });
    await userEvent.click(s);
    expect(s.getAttribute("aria-checked")).toBe("true");
    await userEvent.click(screen.getByText("Blur"));
    expect(s.getAttribute("aria-checked")).toBe("false");
    expect(onchange.mock.calls.map((c) => c[0])).toEqual([true, false]);
  });
  it("Space and Enter toggle it from the keyboard", async () => {
    const onchange = vi.fn();
    render(Switch, { label: "Dim", onchange });
    await userEvent.tab();
    expect(document.activeElement).toBe(screen.getByRole("switch"));
    await userEvent.keyboard(" ");
    await userEvent.keyboard("{Enter}");
    expect(onchange.mock.calls.map((c) => c[0])).toEqual([true, false]);
  });
  it("disabled does nothing and cannot be focused", async () => {
    const onchange = vi.fn();
    render(Switch, { label: "Tint", disabled: true, onchange });
    await userEvent.click(screen.getByRole("switch"));
    expect(onchange).not.toHaveBeenCalled();
    expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
  });
});
