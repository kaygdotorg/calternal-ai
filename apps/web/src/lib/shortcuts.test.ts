import { beforeEach, describe, expect, it, vi } from "vitest";
import { ariaKeys, chordOf, shortcuts as S, show } from "./shortcuts.svelte";

const key = (code: string, o: Partial<KeyboardEventInit> = {}) => new KeyboardEvent("keydown", { code, key: code, bubbles: true, cancelable: true, ...o });

describe("chords", () => {
  it("are built from modifiers then the physical key, Ctrl as Mod off a Mac", () => {
    expect(chordOf(key("KeyK", { ctrlKey: true }))).toBe("Mod+KeyK");
    expect(chordOf(key("Backslash", { ctrlKey: true, shiftKey: true }))).toBe("Mod+Shift+Backslash");
    expect(chordOf(key("Slash", { shiftKey: true }))).toBe("Shift+Slash");
    expect(chordOf(key("ControlLeft", { ctrlKey: true }))).toBeNull();
  });
  it("are shown the way the platform writes them", () => {
    expect(show("Mod+Shift+Backslash")).toBe("Ctrl+Shift+\\");
    expect(show("Alt+ArrowDown")).toBe("Alt+↓");
    expect(show(null)).toBe("Not set");
    expect(ariaKeys("Mod+KeyK")).toBe("Control+K");
  });
});

describe("the registry", () => {
  const ran: string[] = [];
  beforeEach(() => {
    localStorage.clear();
    S.overrides = {};
    S.problem = "";
    S.capturing = null;
    ran.length = 0;
    S.register([
      { id: "a", label: "Do A", group: "Navigation", where: "Anywhere", chord: "Mod+KeyK", run: () => { ran.push("a"); } },
      { id: "b", label: "Do B", group: "Navigation", where: "Anywhere", chord: "Slash", run: () => { ran.push("b"); } },
      { id: "c", label: "Do C", group: "Threads", where: "When it applies", chord: "Alt+Digit1", run: () => false },
    ]);
  });
  S.start();

  it("runs the command for a chord and stops the browser's own handling", () => {
    const e = key("KeyK", { ctrlKey: true });
    dispatchEvent(e);
    expect(ran).toEqual(["a"]);
    expect(e.defaultPrevented).toBe(true);
  });
  it("does nothing for a chord that does not apply, leaving the key to the browser", () => {
    const e = key("Digit1", { altKey: true });
    dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });
  it("a plain-key chord never fires while typing in a field, but does elsewhere", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.dispatchEvent(key("Slash"));
    expect(ran).toEqual([]);
    document.body.dispatchEvent(key("Slash"));
    expect(ran).toEqual(["b"]);
    input.remove();
  });
  it("a changed chord replaces the default and survives a reload of the store", () => {
    expect(S.set("a", "Mod+KeyJ")).toBe(true);
    dispatchEvent(key("KeyK", { ctrlKey: true }));
    dispatchEvent(key("KeyJ", { ctrlKey: true }));
    expect(ran).toEqual(["a"]);
    expect(JSON.parse(localStorage.getItem("cal.shortcuts")!)).toEqual({ a: "Mod+KeyJ" });
    S.reset("a");
    expect(S.chordFor("a")).toBe("Mod+KeyK");
  });
  it("refuses a chord another command has, in plain words", () => {
    expect(S.set("a", "Slash")).toBe(false);
    expect(S.problem).toMatch(/already do b/);
    expect(S.chordFor("a")).toBe("Mod+KeyK");
  });
  it("capturing takes the next chord, Escape cancels", () => {
    S.capturing = "a";
    dispatchEvent(key("Escape"));
    expect(S.capturing).toBeNull();
    S.capturing = "a";
    dispatchEvent(key("KeyY", { ctrlKey: true }));
    expect(S.chordFor("a")).toBe("Mod+KeyY");
    expect(S.capturing).toBeNull();
    expect(ran).toEqual([]);
  });
  it("commands can be switched off with null", () => {
    S.set("a", null);
    dispatchEvent(key("KeyK", { ctrlKey: true }));
    expect(ran).toEqual([]);
  });
  vi.fn();
});
