// #268's rule for which mode the bar shows, in order of what wins.
import { describe, expect, it } from "vitest";
import { pickBarKey, type BarState } from "../src/bar/pick";

const idle: BarState = { ask: false, toast: null, sheet: null, mode: "idle", typing: false, tools: false };
const pick = (s: Partial<BarState>) => pickBarKey({ ...idle, ...s });

describe("the bar's mode", () => {
  it("nothing open: the tabs; a note open: its tools", () => {
    expect(pick({})).toBe("tabs");
    expect(pick({ tools: true })).toBe("tools");
  });
  it("typing on a phone: the text bar, not the tools", () => {
    expect(pick({ tools: true, typing: true })).toBe("text");
  });
  it("blocks or words selected win over typing", () => {
    expect(pick({ tools: true, typing: true, mode: "block" })).toBe("block");
    expect(pick({ tools: true, mode: "text" })).toBe("text");
  });
  it("an open sheet wins over the rest", () => {
    expect(pick({ tools: true, typing: true, mode: "text", sheet: "sheet-insert" })).toBe("sheet-insert");
  });
  it("a toast is the bar, but a question waits for its answer", () => {
    expect(pick({ tools: true, typing: true, toast: "t1" })).toBe("toast-t1");
    expect(pick({ ask: true, toast: "t1" })).toBe("ask");
  });
});
