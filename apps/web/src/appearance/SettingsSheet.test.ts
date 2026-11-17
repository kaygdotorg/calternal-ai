import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it } from "vitest";
import SettingsSheet from "./SettingsSheet.svelte";
import { DEFAULTS, appearance } from "./prefs.svelte";
import type { Wallpaper, WallpaperLibrary } from "./library";

const w = (id: string, name: string) => ({ id, name, source: "builtin" as const, url: `blob:${id}`, thumb: `blob:${id}` });
const saved: Wallpaper[] = [];
const library: WallpaperLibrary = {
  list: async () => [w("dune", "Dune"), w("aurora", "Aurora"), ...saved],
  upload: async (f) => ({ ...w("up-1", f.name), source: "upload" as const }),
  remove: async () => {},
  search: async (q) => (q ? [{ id: `${q}:0`, description: `${q} photo`, thumb: "blob:t", credit: { author: "A. Person", link: "https://unsplash.com/" } }] : []),
  save: async (r) => { const x = { ...w("us-1", r.description), source: "unsplash" as const, credit: r.credit }; saved.push(x); return x; },
};

beforeEach(async () => {
  localStorage.clear();
  appearance.library = library;
  appearance.prefs = structuredClone(DEFAULTS);
  saved.length = 0;
  appearance.wallpapers = await library.list();
});

describe("appearance settings", () => {
  it("picks a wallpaper from the library or none, as a radio group", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    const group = screen.getByRole("radiogroup", { name: "Wallpaper" });
    expect(within(group).getByRole("radio", { name: /Automatic/ }).getAttribute("aria-checked")).toBe("true");
    await userEvent.click(within(group).getByRole("radio", { name: "Aurora" }));
    expect(appearance.prefs.wallpaperId).toBe("aurora");
    await userEvent.click(within(group).getByRole("radio", { name: "No wallpaper" }));
    expect(appearance.prefs.wallpaperId).toBeNull();
  });

  it("searches Unsplash through the adapter and saves the pick with its credit", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    await userEvent.type(screen.getByLabelText("Search Unsplash"), "mountains");
    await userEvent.click(screen.getByRole("button", { name: "Search" }));
    await userEvent.click(await screen.findByRole("button", { name: /Use: mountains photo, by A. Person/ }));
    await screen.findByText(/Photo by/);
    expect(appearance.prefs.wallpaperId).toBe("us-1");
  });

  it("glass sliders change the stored strengths", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    const blur = screen.getByLabelText("Card blur") as HTMLInputElement;
    blur.value = "40";
    blur.dispatchEvent(new Event("input", { bubbles: true }));
    expect(appearance.prefs.card.blur).toBe(40);
    expect(document.documentElement.style.getPropertyValue("--card-tint-a")).toBe("0.85");
  });

  it("chooses a font per role: the shipped one applies, one only the server can fetch says so", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    await userEvent.click(screen.getByRole("button", { name: /Chat/ }));
    const list = await screen.findByRole("radiogroup", { name: "Chat font" });
    await within(list).findByRole("radio", { name: /Inter/ });
    await userEvent.click(within(list).getByRole("radio", { name: /Inter/ }));
    expect((await screen.findByRole("alert")).textContent).toMatch(/server downloads and stores it/);
    expect(appearance.prefs.fonts.chat.family).toBe("Google Sans Flex");
    await userEvent.click(screen.getByRole("radio", { name: "System" }));
    await userEvent.click(await within(screen.getByRole("radiogroup", { name: "Chat font" })).findByRole("radio", { name: /System/ }));
    expect(appearance.prefs.fonts.chat).toEqual({ family: "System", source: "system" });
  });

  it("code fonts list monospaced families only", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    await userEvent.click(screen.getByRole("button", { name: /Code/ }));
    const list = await screen.findByRole("radiogroup", { name: "Code font" });
    await within(list).findByRole("radio", { name: /Maple Mono/ });
    await userEvent.click(screen.getByRole("radio", { name: "Google" }));
    await within(await screen.findByRole("radiogroup", { name: "Code font" })).findByRole("radio", { name: /JetBrains Mono/ });
    expect(within(list).queryByRole("radio", { name: /Lora/ })).toBeNull();
  });

  it("restores defaults", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    appearance.change((x) => (x.card.dim = 80));
    await userEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    expect(appearance.prefs.card.dim).toBe(30);
  });

  it("has no accessibility violations on either view", async () => {
    render(SettingsSheet, { open: true, onclose() {} });
    const run = async () => (await axe.run(document.body, { rules: { "color-contrast": { enabled: false } } })).violations.map((v) => `${v.id}: ${v.nodes[0]?.html.slice(0, 80)}`);
    expect(await run()).toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: /Interface/ }));
    await screen.findByRole("radiogroup", { name: "Interface font" });
    expect(await run()).toEqual([]);
  });
});
