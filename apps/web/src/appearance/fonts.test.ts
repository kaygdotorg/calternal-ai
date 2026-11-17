import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_FONTS, FixtureFonts, applyFonts, faceOf, systemStack } from "./fonts";

describe("font files", () => {
  it("reads weight and italic from file names", () => {
    expect(faceOf("Foo-Regular.ttf")).toEqual({ weight: 400, italic: false });
    expect(faceOf("Foo-SemiBoldItalic.woff2")).toEqual({ weight: 600, italic: true });
    expect(faceOf("Foo-ExtraBold.otf").weight).toBe(800);
    expect(faceOf("Foo-Bold.ttf").weight).toBe(700);
    expect(faceOf("Foo-Light.ttf").weight).toBe(300);
  });
});

describe("font catalogues", () => {
  const lib = new FixtureFonts();
  it("lists a source's families, monospaced ones separately, and filters by query", async () => {
    expect((await lib.search("google", "", false)).map((f) => f.family)).toContain("Inter");
    expect((await lib.search("google", "", true)).every((f) => f.mono)).toBe(true);
    expect((await lib.search("fontsource", "maple", true)).map((f) => f.family)).toEqual(["Maple Mono"]);
    expect((await lib.search("fontshare", "sat", false)).map((f) => f.family)).toEqual(["Satoshi"]);
  });
  it("only shipped families can be drawn without the server, and the refusal says why", async () => {
    const [flex] = await lib.search("google", "google sans", false);
    expect(flex!.ready).toBe(true);
    await expect(lib.install(flex!)).resolves.toBeUndefined();
    const [inter] = await lib.search("google", "inter", false);
    expect(inter!.ready).toBe(false);
    await expect(lib.install(inter!)).rejects.toThrow(/server downloads and stores it/);
  });
  it("the system font is one entry and always ready", async () => {
    expect(await lib.search("system", "", false)).toEqual([{ family: "System", source: "system", mono: false, ready: true }]);
    expect(systemStack(true)).toContain("ui-monospace");
    expect(systemStack(false)).toContain("system-ui");
  });
  it("own fonts need a name, files and the right extensions", async () => {
    await expect(lib.addOwn("", [])).rejects.toThrow(/name/);
    await expect(lib.addOwn("X", [new File(["x"], "x.txt")])).rejects.toThrow(/not a font file/);
  });
});

describe("applying fonts", () => {
  beforeEach(() => document.documentElement.removeAttribute("style"));
  it("the defaults leave the scheme's own stack alone", async () => {
    await applyFonts(DEFAULT_FONTS);
    const st = document.documentElement.style;
    expect([st.getPropertyValue("--font-ui"), st.getPropertyValue("--font-chat"), st.getPropertyValue("--font-mono")]).toEqual(["", "", ""]);
  });
  it("a chosen family overrides one role and the system font uses the platform stacks", async () => {
    await applyFonts({ ...DEFAULT_FONTS, chat: { family: "Inter", source: "google" }, code: { family: "System", source: "system" } });
    const st = document.documentElement.style;
    expect(st.getPropertyValue("--font-chat")).toBe('"Inter"');
    expect(st.getPropertyValue("--font-mono")).toContain("ui-monospace");
    expect(st.getPropertyValue("--font-ui")).toBe("");
  });
});
