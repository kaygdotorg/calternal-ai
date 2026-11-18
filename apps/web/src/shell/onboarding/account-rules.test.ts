import { describe, expect, it } from "vitest";
import { cleanCode, codesText, emailCodeOk, emailLooksOk, groupCode, usernameProblem } from "./account-rules";

describe("account rules", () => {
  it("usernames: 3 to 32 letters, digits and single interior hyphens, minus the reserved names", () => {
    for (const ok of ["kay", "Kay-g", "a1b", "x".repeat(32)]) expect(usernameProblem(ok)).toBeNull();
    for (const bad of ["", "ab", "x".repeat(33), "kay_g", "-kay", "kay-", "ka--y", "Admin", "ROOT", "calternal", "3f2504e0-4f89-11d3-9a0c-0305e82c3301", "kay g"]) expect(usernameProblem(bad), bad).not.toBeNull();
  });
  it("the email code is eight digits, leading zeroes kept", () => {
    expect(emailCodeOk("00123456")).toBe(true);
    expect(emailCodeOk("123456")).toBe(false);
    expect(emailCodeOk("1234567a")).toBe(false);
  });
  it("addresses are only checked for their shape", () => { expect(emailLooksOk("a@b.co")).toBe(true); expect(emailLooksOk("a@b")).toBe(false); });
  it("recovery codes are shown in groups of eight and copied as the server issued them", () => {
    const c = "00112233445566778899aabbccddeeff";
    expect(groupCode(c)).toBe("00112233 44556677 8899aabb ccddeeff");
    expect(codesText([c, c])).toBe(`${c}\n${c}\n`);
    expect(cleanCode("0011 2233-4455 6677 8899 aabb ccdd EEFF")).toBe(c);
  });
});
