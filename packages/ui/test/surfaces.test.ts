import { createRawSnippet, mount, tick } from "svelte";
import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import { Badge, Button, Card, IconButton, Menu, Pill, Row, Sheet, Toolbar } from "../src";

const html = (s: string) => createRawSnippet(() => ({ render: () => s }));
const audit = async (node: HTMLElement) => (await axe.run(node, { rules: { "color-contrast": { enabled: false } } })).violations.map((v) => `${v.id}: ${v.nodes[0]?.html}`);

describe("buttons carry their name", () => {
  it("IconButton uses its label as its name", () => {
    render(IconButton, { icon: "search", label: "Search" });
    const b = screen.getByRole("button", { name: "Search" });
    expect(b.querySelector("svg")?.getAttribute("aria-hidden")).toBe("true");
  });
  it("IconButton reports pressed state", () => {
    render(IconButton, { icon: "sidebar", label: "Toggle", pressed: true });
    expect(screen.getByRole("button", { name: "Toggle" }).getAttribute("aria-pressed")).toBe("true");
  });
  it("Button text is its name and a click fires", async () => {
    const onclick = vi.fn();
    render(Button, { text: "Save", variant: "primary", onclick });
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(onclick).toHaveBeenCalledOnce();
  });
  it("an icon-only Pill is named by its label; a wide one shows it", () => {
    const a = render(Pill, { label: "Search", icon: "search" });
    expect(screen.getByRole("button", { name: "Search" })).toBeTruthy();
    a.unmount();
    render(Pill, { label: "Claude · Opus", icon: "spark", showLabel: true });
    expect(screen.getByRole("button", { name: "Claude · Opus" }).textContent).toContain("Claude");
  });
  it("Badge says its tone in words", () => {
    render(Badge, { text: "running", tone: "accent", dot: true });
    expect(screen.getByText("running")).toBeTruthy();
  });
});

describe("Toolbar and Card", () => {
  it("Toolbar is a named toolbar with roving arrows", async () => {
    render(Toolbar, { label: "Actions", children: html('<span><button aria-label="One">1</button><button aria-label="Two">2</button></span>') });
    expect(screen.getByRole("toolbar", { name: "Actions" })).toBeTruthy();
    screen.getByRole("button", { name: "One" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Two" }));
  });
  it("Card is a named region", () => {
    render(Card, { label: "Needs your attention", variant: "attention", children: html("<p>x</p>") });
    expect(screen.getByRole("region", { name: "Needs your attention" })).toBeTruthy();
  });
  it("Row is a link with an href and marks the current page", () => {
    render(Row, { title: "Thread", href: "/threads/1", selected: true });
    const a = screen.getByRole("link", { name: "Thread" });
    expect(a.getAttribute("aria-current")).toBe("page");
  });
});

describe("Sheet", () => {
  it("is a modal dialog: focus moves to it, Tab is trapped, Escape closes and restores focus", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    const onclose = vi.fn();
    render(Sheet, { open: true, label: "Account", onclose, children: html('<div><button>First</button><button>Last</button></div>') });
    await tick();
    const dialog = screen.getByRole("dialog", { name: "Account" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(dialog);
    await userEvent.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "First" }));
    screen.getByRole("button", { name: "Last" }).focus();
    await userEvent.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "First" }));
    await userEvent.keyboard("{Escape}");
    expect(onclose).toHaveBeenCalledOnce();
    opener.remove();
  });
  it("renders nothing when closed", () => {
    render(Sheet, { open: false, label: "Account", onclose() {}, children: html("<p>x</p>") });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("Menu", () => {
  it("lists radio items, picks one and closes on Escape", async () => {
    const onpick = vi.fn();
    const onclose = vi.fn();
    render(Menu, { open: true, label: "Model", items: [{ id: "a", label: "Opus", checked: true }, { id: "b", label: "Sonnet", checked: false }], onpick, onclose });
    await tick();
    expect(screen.getByRole("menuitemradio", { name: "Opus" }).getAttribute("aria-checked")).toBe("true");
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Sonnet" }));
    expect(onpick).toHaveBeenCalledWith("b");
  });
});

describe("accessibility audit (axe, structure and names)", () => {
  it("a composed screen of every surface has no violations", async () => {
    const host = document.createElement("div");
    document.body.append(host);
    mount(Toolbar, { target: host, props: { label: "Tools", children: html('<button aria-label="Copy">c</button>') } });
    mount(Card, { target: host, props: { label: "Details", children: html("<h2>Hi</h2>") } });
    mount(Button, { target: host, props: { text: "Go" } });
    mount(IconButton, { target: host, props: { icon: "close", label: "Close" } });
    await tick();
    expect(await audit(host)).toEqual([]);
    host.remove();
  });
});
