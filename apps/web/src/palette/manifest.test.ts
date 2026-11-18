import { describe, expect, it } from "vitest";
import { allCommands, replaceServerManifests } from "./manifest";

describe("the server's palette manifests (plugins.manifests)", () => {
  it("adds commands and a command for each Settings section, and leaves the built-ins alone", () => {
    const built = allCommands().map((c) => c.id);
    replaceServerManifests([{ plugin: "acme-tasks", version: "1.0.0", commands: [{ id: "acme.open", label: "Open Acme", group: "Acme", go: "/acme", words: ["acme"] }], settings_sections: [{ id: "acme", label: "Acme", go: "/settings/acme" }] }]);
    const ids = allCommands().map((c) => c.id);
    expect(built.every((i) => ids.includes(i))).toBe(true);
    expect(ids).toContain("acme.open");
    expect(allCommands().find((c) => c.id === "acme-tasks:settings:acme")).toMatchObject({ label: "Settings: Acme", group: "Settings", go: "/settings/acme" });
  });
  it("replaces the whole set on the next read, so a plugin turned off stops offering its commands", () => {
    replaceServerManifests([]);
    expect(allCommands().some((c) => c.id.startsWith("acme"))).toBe(false);
  });
  it("shows a command id once when the built-in app and the server both declare it (the built-in wins)", () => {
    replaceServerManifests([{ plugin: "calternal-threads", commands: [{ id: "threads.open", label: "Open Threads (server)", group: "Threads", go: "/threads" }] }]);
    const open = allCommands().filter((c) => c.id === "threads.open");
    expect(open).toHaveLength(1);
    expect(open[0]!.label).toBe("Open Threads");
    replaceServerManifests([]);
  });
});
