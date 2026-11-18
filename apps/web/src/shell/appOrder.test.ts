import { describe, expect, it } from "vitest";
import { applyOrder, move } from "./appOrder";

const apps = [{ kind: "threads" }, { kind: "notes" }, { kind: "settings" }];
describe("the person's app order", () => {
  it("follows the chosen order, leaves the rest in their own order and ignores unknown names", () => {
    expect(applyOrder(apps, ["settings", "x", "threads"]).map((a) => a.kind)).toEqual(["settings", "threads", "notes"]);
    expect(applyOrder(apps, []).map((a) => a.kind)).toEqual(["threads", "notes", "settings"]);
  });
  it("moves one place and stops at the ends", () => {
    expect(move(["threads", "notes", "settings"], "notes", -1)).toEqual(["notes", "threads", "settings"]);
    expect(move(["threads", "notes", "settings"], "threads", -1)).toEqual(["threads", "notes", "settings"]);
    expect(move(["threads", "notes", "settings"], "settings", 1)).toEqual(["threads", "notes", "settings"]);
  });
});
