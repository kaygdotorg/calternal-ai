/**
 * What the machine and provider steps of the onboarding need, behind one interface so the screens are the same against the real server and against fixtures. The shapes are the backend lane #48's (not in dev yet when this
 * was written; the server implementation below is built on them and falls back to what the server answers today where an operation is missing):
 *  - `machines.onboarding {}` gives `{ attached, ready, machine_id }`: the co-located machine is enrolled as a pool machine at first-owner setup, `ready` says it can run threads (shown as connected, no code needed);
 *  - `machines.install {}` gives `{ script, expires_ms }`: a 15-minute single-use script that adds a machine;
 *  - `provider.sign_in.start { space_id, provider_id, thread_id?, machine_id?, pool? }` gives `{ id, status }`; the request appears in `requests.list` with `detail: { kind: "provider_sign_in", provider_id, label, url, display_code,
 *    needs_code, expires_at_ms }` (url and display_code may be null; private to the signed-in person); `provider.sign_in.answer { id, code }` and `provider.sign_in.status { id }` give `{ id, status }` with status `pending`,
 *    `done` or `failed` (a missing or expired id is `not_found`).
 * The fixture implementation is a small simulation for `?server=0&onboarding=1`, tests and the design.
 */
import { enrollmentCode, installScript, setDirect } from "../../server/mutations";
import { server } from "../../server/client";
import { checkProvider, providerName, readiness } from "../../server/readiness";
import { spaces } from "../spaces.svelte";

/** A machine of the person's. */
export interface MachineInfo { id: string; name: string; os: string; arch: string; online: boolean }
/** Whether the instance has a machine that can run threads: `attached` (one was set up with the instance), `ready` (it can run now) and, when there is one, what it is. */
export interface MachineStatus { attached: boolean; ready: boolean; machine: MachineInfo | null }
/** The script that adds a machine, and when it stops working. */
export interface Install { script: string; expiresMs: number }
/** One provider on one machine. `command` is the provider's own sign-in command, shown as the fallback (never run here). */
export interface ProviderRow { machineId: string; machineName: string; id: string; name: string; state: "signed_in" | "not_signed_in" | "not_installed" | "unknown" | "off"; command?: string }
/** A sign-in's progress. */
export type SignInStatus = "pending" | "done" | "failed";
/** What a person must do to finish a provider's sign-in: open the link, and (when `needsCode`) type the code the provider shows back. `displayCode` is a code to confirm on the provider's page. */
export interface SignInRequest { id: string; providerId: string; label: string; url: string | null; displayCode: string | null; needsCode: boolean; expiresAtMs: number }

/** A repository threads can work on. */
export interface RepoInfo { id: string; name: string; url: string }

/** The calls the steps make. */
export interface OnboardingApi {
  /** Whether a machine is ready. */
  machineStatus(): Promise<MachineStatus>;
  /** Makes the script that adds a machine. */
  install(): Promise<Install>;
  /** Every provider on every machine, as the server last knew them. */
  providers(): Promise<ProviderRow[]>;
  /** Asks the machine to check one provider now (the fallback to the sign-in below). */
  check(machineId: string, providerId: string): Promise<void>;
  /** Starts a provider's sign-in on a machine. */
  startSignIn(providerId: string, machineId?: string): Promise<{ id: string; status: SignInStatus }>;
  /** The link and code of a started sign-in, or null while the machine has not produced them yet. */
  signInRequest(id: string): Promise<SignInRequest | null>;
  /** Gives the code the provider showed back. */
  answerSignIn(id: string, code: string): Promise<SignInStatus>;
  /** How a sign-in is going. */
  signInStatus(id: string): Promise<SignInStatus>;
  /** The repositories threads may work on. Needs the server's repositories operations (none exist yet): the real implementation says so. */
  repositories(): Promise<RepoInfo[]>;
  /** Adds a repository by its address. */
  addRepository(url: string): Promise<RepoInfo>;
  /** Where threads run on a machine: sandboxed (false) or directly on it (true), `machines.set`. */
  setDirect(machineId: string, direct: boolean): Promise<void>;
}

const missing = (e: unknown) => (e as { code?: string } | null)?.code === "not_found" || (e as { status?: number } | null)?.status === 404 || /unknown operation|not_found/i.test(String((e as Error)?.message ?? e));
async function listMachines(): Promise<MachineInfo[]> {
  const { machines } = await server.json<{ machines: { id: string; name: string; os: string; arch: string; state: string }[] }>("/api/v1/machines", {});
  return machines.filter((m) => m.state !== "revoked").map((m) => ({ id: m.id, name: m.name, os: m.os, arch: m.arch, online: m.state === "online" }));
}

/** The real server. */
export const serverApi: OnboardingApi = {
  async machineStatus() {
    const machines = await listMachines().catch(() => [] as MachineInfo[]);
    try {
      const r = await server.operate<{ attached: boolean; ready: boolean; machine_id: string | null }>("machines.onboarding", {});
      return { attached: r.attached, ready: r.ready, machine: machines.find((m) => m.id === r.machine_id) ?? (r.ready ? { id: r.machine_id ?? "", name: "Your instance's machine", os: "", arch: "", online: true } : null) };
    } catch (e) {
      if (!missing(e)) throw e;
      const m = machines.find((x) => x.online) ?? null; // an older server: any machine that is connected
      return { attached: false, ready: !!m, machine: m };
    }
  },
  async install() {
    try { const r = await installScript(); return { script: r.script, expiresMs: r.expires_ms }; }
    catch (e) {
      if (!missing(e)) throw e;
      const c = await enrollmentCode(); // an older server: the one-use code and its command
      return { script: `calternal machine enroll --server ${location.origin} --code ${c.code}`, expiresMs: c.expires_ms };
    }
  },
  async providers() {
    const r = await readiness(await spaces.whenCurrent());
    return r.machines.flatMap((m) => Object.entries(m.providers).flatMap(([id, p]) => p ? [{
      machineId: m.machine_id, machineName: m.name ?? "Machine", id, name: providerName(id),
      state: !p.enabled ? "off" as const : p.ready ? "signed_in" as const : !p.check ? "unknown" as const : !p.check.installed ? "not_installed" as const : !p.check.signed_in ? "not_signed_in" as const : "unknown" as const,
      command: p.check?.sign_in_command,
    }] : []));
  },
  check: (machineId, providerId) => checkProvider(spaces.current.id, machineId, providerId).then(() => undefined),
  startSignIn: (providerId, machineId) => server.operate("provider.sign_in.start", { space_id: spaces.current.id, provider_id: providerId, ...(machineId ? { machine_id: machineId } : {}) }),
  async signInRequest(id) {
    const { requests } = await server.operate<{ requests: { id: string; detail?: { kind?: string; provider_id: string; label: string; url: string | null; display_code: string | null; needs_code: boolean; expires_at_ms: number } }[] }>("requests.list", { space_id: spaces.current.id });
    const r = requests.find((x) => x.id === id && x.detail?.kind === "provider_sign_in");
    return r?.detail ? { id: r.id, providerId: r.detail.provider_id, label: r.detail.label, url: r.detail.url, displayCode: r.detail.display_code, needsCode: r.detail.needs_code, expiresAtMs: r.detail.expires_at_ms } : null;
  },
  async answerSignIn(id, code) { return (await server.operate<{ status: SignInStatus }>("provider.sign_in.answer", { id, code })).status; },
  async signInStatus(id) { return (await server.operate<{ status: SignInStatus }>("provider.sign_in.status", { id })).status; },
  async repositories() { throw new Error("repositories: no server operation yet"); },
  async addRepository() { throw new Error("repositories: no server operation yet"); },
  setDirect: (machineId, direct) => setDirect(machineId, direct).then(() => undefined),
};

/** A simulation for fixtures: a machine that connects on the third look after the script was made (or is there from the start with `attached`), a sign-in that waits for its code. */
export function fixtureApi(attached = false): OnboardingApi {
  let made = 0, looks = 0, signed = false, started = 0;
  const repos: RepoInfo[] = [];
  const machine: MachineInfo = { id: "m-1", name: attached ? "This instance" : "dev-machine", os: "linux", arch: "x86_64", online: true };
  return {
    async machineStatus() { await new Promise((r) => setTimeout(r, 450)); if (attached) return { attached: true, ready: true, machine }; if (made && ++looks >= 3) return { attached: false, ready: true, machine }; return { attached: false, ready: false, machine: null }; },
    async install() { made++; return { script: "curl -fsSL https://calternal.example/install.sh | sh -s -- --code K7QM-4D2X", expiresMs: Date.now() + 15 * 60_000 }; },
    async providers() { return attached || (made && looks >= 3) ? [{ machineId: machine.id, machineName: machine.name, id: "calternal-claude-code", name: "Claude Code", state: signed ? "signed_in" : "not_signed_in", command: "claude login" }] : []; },
    async check() { await new Promise((r) => setTimeout(r, 500)); },
    async startSignIn() { started = Date.now(); return { id: "si-1", status: "pending" }; },
    async signInRequest() { return Date.now() - started > 600 ? { id: "si-1", providerId: "calternal-claude-code", label: "Claude Code", url: "https://claude.ai/oauth/authorize?code=true&client=calternal", displayCode: "ABCD-1234", needsCode: true, expiresAtMs: Date.now() + 10 * 60_000 } : null; },
    async answerSignIn(_id, code) { await new Promise((r) => setTimeout(r, 700)); if (code.trim()) { signed = true; return "done"; } return "failed"; },
    async signInStatus() { return signed ? "done" : "pending"; },
    async repositories() { return repos; },
    async addRepository(url) { const r = { id: `r-${repos.length + 1}`, name: url.replace(/\.git$/, "").split("/").at(-1) || url, url }; repos.push(r); return r; },
    async setDirect() { await new Promise((r) => setTimeout(r, 300)); },
  };
}
