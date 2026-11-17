import { describe, expect, test } from "vitest";
import { ServerError } from "./client";
import { say } from "./say";

describe("say: failures in plain words with the next step", () => {
  test("a refusal for lack of a capability says who can help", () => {
    expect(say(new ServerError("forbidden", "x", 403))).toMatch(/owner or a manager/);
  });
  test("a network failure says it keeps trying", () => {
    expect(say(new TypeError("Failed to fetch"))).toMatch(/Can't reach the server/);
    expect(say(new ServerError("unavailable", "x"))).toMatch(/keeps trying/);
  });
  test("sign-in failures: wrong or used code, closed prompt, unknown passkey", () => {
    expect(say(new ServerError("setup_token_invalid", "x"))).toMatch(/wrong or already used/);
    expect(say(new ServerError("invite_invalid", "x"))).toMatch(/wrong or already used/);
    expect(say(new ServerError("cancelled", "x"))).toMatch(/closed or timed out/);
    expect(say(new ServerError("passkey_failed", "x"))).toMatch(/not recognised/);
  });
  test("an unknown failure still names the thing and says to try again", () => {
    expect(say(new Error("boom"), "load the members")).toBe("Could not load the members. Try again.");
  });
  test("the server's own message is never shown", () => {
    expect(say(new ServerError("internal", "SECRET DETAIL"))).not.toMatch(/SECRET/);
  });
});
