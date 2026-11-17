/**
 * What the signed-in person may do, as far as the shell shows it. Fixtures: the person is the operator of the fixture server (holds the server-admin capability) unless the address says
 * `?as=member` (remembered for this browser; `?as=operator` puts it back), so both cases can be looked at. The real answer comes from the server's capability set for the caller.
 */
const KEY = "cal.role";

function read(): "operator" | "member" {
  try {
    const q = new URLSearchParams(location.search).get("as");
    if (q === "member" || q === "operator") localStorage.setItem(KEY, q);
    return localStorage.getItem(KEY) === "member" ? "member" : "operator";
  } catch {
    return "operator";
  }
}

/** The capabilities the shell checks. Read once at load. */
export const caps = { serverAdmin: read() === "operator" };
