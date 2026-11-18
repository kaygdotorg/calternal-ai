import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import RequestBar from "./RequestBar.svelte";
import type { Request } from "./model";

const Q: Request = { id: "q", kind: "question", threadId: "t", title: "Before I open the pull request", form: '$branch="lane/x"\n$how="Squash"\nForm([Input($branch,"Branch name"), Choice($how,["Rebase","Squash"],"How should I land it?")], "Before I open the pull request")' };
const A: Request = { id: "a", kind: "approval", threadId: "t", title: "Run a command outside the workspace", action: "Push the branch", command: "git push origin lane/x", lifetimes: ["once", "thread"] };
const S: Request = { id: "s", kind: "secret", threadId: "t", name: "FORGEJO_TOKEN", label: "Forgejo access token", why: "To publish pages.", scope: "personal", required: true, expiresAt: 1_000_000 + 15 * 60_000 };
const now = () => 1_000_000;
const A1: Request = { ...(A as Extract<Request, { kind: "approval" }>), lifetimes: ["once"] };

describe("request bar", () => {
  it("is absent when nothing waits", () => {
    render(RequestBar, { requests: [], onanswer() {} });
    expect(screen.queryByRole("region", { name: "Waiting on you" })).toBeNull();
  });

  it("question: draws the described form and answers with its values", async () => {
    const onanswer = vi.fn();
    render(RequestBar, { requests: [Q], onanswer, now });
    expect(await screen.findByRole("form", { name: "Before I open the pull request" }, { timeout: 15000 })).toBeTruthy(); // the form renderer is a lazy chunk: wait for it, not for a guessed second (a loaded machine is slower, not wrong)
    const input = screen.getByLabelText("Branch name") as HTMLInputElement;
    await userEvent.clear(input);
    await userEvent.type(input, "lane/invites");
    await userEvent.click(screen.getByRole("tab", { name: "Rebase" }));
    await userEvent.click(screen.getByRole("button", { name: "Send answers" }));
    expect(onanswer).toHaveBeenCalledWith("q", { kind: "question", values: { branch: "lane/invites", how: "Rebase" } });
  });

  it("approval: drawn by the shell with its own buttons; each button is a distinct decision", async () => {
    const onanswer = vi.fn();
    render(RequestBar, { requests: [A], onanswer, now });
    expect(screen.getByText(/Drawn by Calternal/)).toBeTruthy();
    expect(screen.getByLabelText("The command").textContent).toBe("git push origin lane/x");
    await userEvent.click(screen.getByRole("button", { name: "Allow once" }));
    await userEvent.click(screen.getByRole("button", { name: "Allow for this thread" }));
    await userEvent.click(screen.getByRole("button", { name: "Deny" }));
    expect(onanswer.mock.calls.map((c) => c[1].decision)).toEqual(["once", "thread", "deny"]);
  });

  it("approval text from an agent is never drawn as markup", () => {
    const evil: Request = { ...A, action: "<img src=x onerror=alert(1)>", command: "<script>x</script>" };
    const { container } = render(RequestBar, { requests: [evil], onanswer() {}, now });
    expect(container.querySelector("img, script")).toBeNull();
  });

  it("secret: masked entry, Save needs a value, the value is sent once and cleared", async () => {
    const onanswer = vi.fn();
    render(RequestBar, { requests: [S], onanswer, now });
    const field = screen.getByLabelText("Forgejo access token") as HTMLInputElement;
    expect(field.type).toBe("password");
    expect(field.autocomplete).toBe("off");
    const save = screen.getByRole("button", { name: "Save" }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await userEvent.type(field, "s3cret");
    expect(save.disabled).toBe(false);
    await userEvent.click(save);
    expect(onanswer).toHaveBeenCalledWith("s", { kind: "secret", value: "s3cret" });
    expect(field.value).toBe("");
    expect(screen.getByText(/never the value/)).toBeTruthy();
  });

  it("secret: shows its countdown and, when expired, offers to ask again instead of a field", () => {
    const { unmount } = render(RequestBar, { requests: [S], onanswer() {}, now });
    expect(screen.getByText("15:00")).toBeTruthy();
    unmount();
    render(RequestBar, { requests: [{ ...S, expiresAt: 999_000 }], onanswer() {}, now });
    expect(screen.getByText(/This request expired/)).toBeTruthy();
    expect(screen.queryByLabelText("Forgejo access token")).toBeNull();
    expect(screen.getByRole("button", { name: "Ask again" })).toBeTruthy();
  });

  it("several requests: one at a time with a pager, and the bar can be folded", async () => {
    render(RequestBar, { requests: [Q, A, S], onanswer() {}, now });
    expect(screen.getByText("1 of 3")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Previous request" }).hasAttribute("disabled")).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Next request" }));
    expect(screen.getByText("Approval needed")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Fold the request" }));
    expect(screen.queryByText("Run a command outside the workspace")).toBeNull();
    expect(screen.getByRole("button", { name: "Show the request" })).toBeTruthy();
  });

  it("has no accessibility violations in any of its three forms", async () => {
    for (const r of [Q, A, S]) {
      const { container, unmount } = render(RequestBar, { requests: [r], onanswer() {}, now });
      const v = (await axe.run(container, { rules: { "color-contrast": { enabled: false } } })).violations;
      expect(v.map((x) => `${r.kind} ${x.id}: ${x.nodes[0]?.html}`)).toEqual([]);
      unmount();
    }
  });

  it("a thread-less region lookup finds the bar by name", () => {
    render(RequestBar, { requests: [A], onanswer() {}, now });
    const bar = screen.getByRole("region", { name: "Waiting on you" });
    expect(within(bar).getByRole("heading", { name: "Push the branch" })).toBeTruthy();
  });
});

describe("single-use approvals", () => {
  it("offers only Allow once when the server says the action cannot last for the thread", () => {
    render(RequestBar, { requests: [A1], onanswer: () => {}, now });
    expect(screen.getByRole("button", { name: "Allow once" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Allow for this thread" })).toBeNull();
    expect(screen.getByText(/can only be allowed once/)).toBeTruthy();
  });
  it("offers both when the action may last for the thread", () => {
    render(RequestBar, { requests: [A], onanswer: () => {}, now });
    expect(screen.getByRole("button", { name: "Allow for this thread" })).toBeTruthy();
  });
});

