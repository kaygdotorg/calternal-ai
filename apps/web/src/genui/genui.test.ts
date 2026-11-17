import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import GenUi from "./GenUi.svelte";

describe("generated UI renderer", () => {
  it("draws a card with a table and a badge", () => {
    render(GenUi, { source: 'Card("Results", [Table(["Suite","Result"],[["store", Badge("passed","good")]])])' });
    expect(screen.getByRole("group", { name: "Results" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Suite" })).toBeTruthy();
    expect(screen.getByText("passed")).toBeTruthy();
  });
  it("draws a question form whose fields have names and whose state follows typing", async () => {
    const src = '$name="a"\n$ok=false\nForm([Input($name,"Branch name"), Toggle($ok,"Run tests"), Button("Go", Send("hi " + $name), "primary")], "Questions")';
    const onsend = vi.fn();
    render(GenUi, { source: src, onsend });
    expect(screen.getByRole("form", { name: "Questions" })).toBeTruthy();
    const input = screen.getByLabelText("Branch name") as HTMLInputElement;
    await userEvent.clear(input);
    await userEvent.type(input, "lane/x");
    await userEvent.click(screen.getByRole("switch", { name: "Run tests" }));
    expect(screen.getByRole("switch", { name: "Run tests" }).getAttribute("aria-checked")).toBe("true");
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    expect(onsend).toHaveBeenCalledWith("hi lane/x");
  });
  it("shows a plain fallback for a catalog node it does not draw yet", () => {
    render(GenUi, { source: 'Gauge(3, 0, 10, "Load", "neutral", "Three of ten")' });
    expect(screen.getByText(/Gauge is not drawn by this client yet/)).toBeTruthy();
  });
  it("refuses a description that fails validation and says why", () => {
    render(GenUi, { source: 'Button("")' });
    expect(screen.getByRole("alert").textContent).toContain("could not be drawn");
  });
  it("an input without a label is rejected by the description itself", () => {
    render(GenUi, { source: '$a=""\nInput($a, "")' });
    expect(screen.getByRole("alert")).toBeTruthy();
  });
});
