import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { compile, evaluate } from "@calternal/genui";
import { folderLabel, listView, lit, messageView, setupView } from "@calternal/mail/views";
import { beforeAll, describe, expect, it, vi } from "vitest";
import GenUi from "../../genui/GenUi.svelte";
import { FixtureMailClient, FIXTURE_ACCOUNT } from "./fixtures";
import { MailStore } from "./store.svelte";

/** A description must compile and evaluate without a single Error node (the shell would draw an alert for one). */
function clean(source: string) {
  const c = compile(source);
  expect(c.diagnostics.filter((d) => d.severity === "error"), source.slice(0, 200)).toEqual([]);
  const snap = evaluate(c.program!);
  const walk = (n: any): string[] => [...(n.type === "Error" ? [n.props.message] : []), ...((n.props?.children ?? []) as any[]).flatMap((k) => (k?.kind === "node" ? walk(k) : []))];
  expect(snap.roots.flatMap(walk)).toEqual([]);
  return snap;
}

describe("GenUI literals", () => {
  it("cannot be broken out of by message content", () => {
    for (const t of ['a"b', "c\\d", "$x ${y}", "line\nbreak\ttab", "\u0000 ", "\ud800 lone", "`tick`", "😀"]) {
      const snap = clean(`Text(${lit(t)})`);
      expect(snap.roots).toHaveLength(1);
    }
  });
  it("decodes IMAP folder names for display", () => {
    expect(folderLabel("Archive/&AOk-t&AOk-")).toBe("Archive/été");
    expect(folderLabel("R&AOk-sum&AOk-s")).toBe("Résumés");
    expect(folderLabel("A&-B")).toBe("A&B");
    expect(folderLabel("INBOX")).toBe("INBOX");
  });
});

describe("Mail views", () => {
  it("every screen compiles for the awkward fixture messages", async () => {
    const c = new FixtureMailClient();
    const folders = await c.folders(FIXTURE_ACCOUNT.id);
    const page = await c.headers(FIXTURE_ACCOUNT.id, "INBOX");
    const base = { accounts: [FIXTURE_ACCOUNT, { ...FIXTURE_ACCOUNT, id: "x", title: "other" }], account: FIXTURE_ACCOUNT.id, folders, folder: "INBOX", headers: page.headers, more: true, selected: page.headers[0]!.uid };
    clean(listView(base));
    clean(listView({ ...base, headers: [], error: "Could not list.", loading: true, notice: "No password yet." }));
    clean(listView({ ...base, folders: folders.slice(0, 3), accounts: [FIXTURE_ACCOUNT] }));
    for (let i = 0; i < 7; i++) clean(messageView(await c.read(FIXTURE_ACCOUNT.id, "INBOX", page.uid_validity, page.headers[i]!.uid)));
    clean(setupView({ host: 'h"$', port: "993", username: "u" }, { error: "bad", waiting: true, back: true }));
  });
  it("uses the shared Selector for a few folders and a menu for many; unread rows are marked", async () => {
    const c = new FixtureMailClient();
    const folders = await c.folders(FIXTURE_ACCOUNT.id);
    const page = await c.headers(FIXTURE_ACCOUNT.id, "INBOX");
    const base = { accounts: [FIXTURE_ACCOUNT], account: FIXTURE_ACCOUNT.id, headers: page.headers, more: false, selected: null, folder: "INBOX" };
    expect(listView({ ...base, folders })).toContain("Select($folder");
    expect(listView({ ...base, folders: folders.slice(0, 3) })).toContain("Choice($folder");
    expect(listView({ ...base, folders })).toContain('Badge("Unread"');
  });
  it("never turns a sender or date into Markdown that could load an image", () => {
    const src = listView({ accounts: [], account: null, folders: [], folder: "INBOX", more: false, selected: null, headers: [{ uid: 1, subject: "s", from: ["![x](https://t.test/p.png)@a.test"], date: "![y](https://t.test/q.png)", read: true, bytes: 1 }] });
    expect(src).not.toContain("](");
  });
  it("says remote images are not loaded where a message has HTML, and lists attachments", async () => {
    const c = new FixtureMailClient();
    const m = await c.read(FIXTURE_ACCOUNT.id, "INBOX", 7, 199);
    expect(messageView(m)).toContain("Remote images are never loaded on their own");
    expect(messageView(m)).toContain("receipt-4411.pdf");
  });
  it("draws in the shell's renderer, with no password field", async () => {
    render(GenUi, { source: setupView({ host: "", port: "993", username: "" }) });
    expect(screen.getByLabelText("Mail server")).toBeTruthy();
    expect(screen.getByLabelText("Port")).toBeTruthy();
    expect(screen.getByLabelText("Username")).toBeTruthy();
    expect(screen.queryByLabelText(/password/i)).toBeNull();
    expect(document.querySelector("input[type=password]")).toBeNull();
  });
});

describe("Mail store", () => {
  it("loads the inbox, pages, opens a message and marks it read", async () => {
    const s = new MailStore(new FixtureMailClient());
    await s.start();
    expect(s.folder).toBe("INBOX");
    expect(s.headers).toHaveLength(20);
    await s.more();
    expect(s.headers).toHaveLength(40);
    await s.more();
    expect(s.headers).toHaveLength(45);
    expect(s.cursor).toBeNull();
    const first = s.headers[0]!;
    expect(first.read).toBe(false);
    await s.view(first.uid);
    expect(s.open?.header.subject).toContain("Welcome");
    await waitFor(() => expect(s.headers[0]!.read).toBe(true));
  });
  it("a command from a screen is performed, and a stale folder answer is dropped", async () => {
    const s = new MailStore(new FixtureMailClient());
    await s.start();
    const calls: string[] = [];
    const fx = { open: (u: number) => calls.push(`open ${u}`), setup: () => calls.push("setup"), leave: () => calls.push("leave") };
    s.apply({ go: "open:199" }, fx);
    s.apply({ go: "setup" }, fx);
    expect(calls).toEqual(["open 199", "setup"]);
    const slow = s.useFolder("Sent");
    await s.useFolder("INBOX");
    await slow;
    expect(s.folder).toBe("INBOX");
    expect(s.headers.length).toBe(20);
  });
  it("adding an account validates the port, keeps the draft on failure and waits for the protected prompt", async () => {
    const s = new MailStore(new FixtureMailClient());
    await s.start();
    s.setupDraft = { host: "imap.example.test", port: "abc", username: "me" };
    expect(await s.add()).toBe(false);
    expect(s.setupError).toContain("port");
    s.setupDraft = { host: "unreachable.example.test", port: "993", username: "me" };
    expect(await s.add()).toBe(false);
    expect(s.setupError).toContain("Could not reach");
    s.setupDraft = { host: "imap.example.test", port: "993", username: "me" };
    expect(await s.add()).toBe(true);
    expect(s.setupWaiting).toBe(true);
    expect(s.accounts).toHaveLength(2);
    await s.useAccount(s.accounts[1]!.id);
    expect(s.notice).toContain("no password");
    expect(JSON.stringify(s)).not.toMatch(/password":/);
  });
});

describe("Mail screens in the shell", () => {
  // jsdom has no IntersectionObserver or matchMedia; the header tracker (shared, tested in packages/ui) only needs them to exist.
  beforeAll(() => {
    vi.stubGlobal("IntersectionObserver", class { observe() {} unobserve() {} disconnect() {} takeRecords() { return []; } });
    if (!window.matchMedia) vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  });
  it("choosing a message in the sidebar opens it in the pane, marks it read and a typed form value survives a redraw", async () => {
    const { mail } = await import("./store.svelte");
    const Sidebar = (await import("./Sidebar.svelte")).default;
    const Main = (await import("./Main.svelte")).default;
    const { navigate, route } = await import("../../lib/route.svelte");
    render(Sidebar);
    const row = await screen.findByRole("button", { name: /Welcome to the fixture mailbox/ });
    await userEvent.click(row);
    await waitFor(() => expect(route.path).toBe("/mail/200"));
    const pane = render(Main, { id: "200" });
    await waitFor(() => expect(pane.container.querySelector("h1")?.textContent).toContain("Welcome"));
    expect(await pane.findByText(/An indented line, kept as written/)).toBeTruthy();
    await waitFor(() => expect(mail.headers.find((h) => h.uid === 200)?.read).toBe(true));
    navigate("/mail/setup");
    pane.rerender({ id: "setup" });
    const host = (await pane.findByLabelText("Mail server")) as HTMLInputElement;
    await userEvent.type(host, "imap.example.test");
    expect(host.value).toBe("imap.example.test");
    expect(pane.queryByLabelText(/password/i)).toBeNull();
  });
});
