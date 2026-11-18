/**
 * What the machine and provider steps of the onboarding need, behind one interface so the screens are the same against the real server and against fixtures. The real one is built on what the server answers today
 * (the machines routes, `agent.readiness`, `agent.check`); the relayed provider sign-in and the co-located machine enrolled at setup are the backend lane #48's, and its operations replace the notes marked below
 * when it sends shapes. The fixture one is a small simulation (a machine that connects a few seconds after the command is run, a provider that signs in after a check) for `?server=0&onboarding=1`, tests and the design.
 */
import { server } from "../../server/client";
import { checkProvider, providerName, readiness } from "../../server/readiness";
import { spaces } from "../spaces.svelte";

/** A machine of the person's. */
export interface MachineInfo { id: string; name: string; os: string; arch: string; online: boolean }
/** A one-use code for a machine to enroll with, and the command that uses it. */
export interface EnrolCode { code: string; expiresMs: number; command: string }
/** One provider on one machine. `command` is the provider's own sign-in command, shown to copy (never run here). */
export interface ProviderRow { machineId: string; machineName: string; id: string; name: string; state: "signed_in" | "not_signed_in" | "not_installed" | "unknown" | "off"; command?: string }

/** The calls the steps make. */
export interface OnboardingApi {
  /** The person's machines (revoked ones left out). #48: the instance's co-located machine is already among them when one was set up at deploy. */
  machines(): Promise<MachineInfo[]>;
  /** Makes a one-use enrollment code. #48: becomes the printed script. */
  makeCode(): Promise<EnrolCode>;
  /** Every provider on every machine, as the server last knew them. #48: adds the relayed sign-in (a link to open and a code to confirm). */
  providers(): Promise<ProviderRow[]>;
  /** Asks the machine to check one provider now. */
  check(machineId: string, providerId: string): Promise<void>;
}

/** The real server. */
export const serverApi: OnboardingApi = {
  async machines() {
    const { machines } = await server.json<{ machines: { id: string; name: string; os: string; arch: string; state: string }[] }>("/api/v1/machines", {});
    return machines.filter((m) => m.state !== "revoked").map((m) => ({ id: m.id, name: m.name, os: m.os, arch: m.arch, online: m.state === "online" }));
  },
  async makeCode() {
    const c = await server.json<{ code: string; expires_ms: number }>("/api/v1/machines/codes", {});
    return { code: c.code, expiresMs: c.expires_ms, command: `calternal machine enroll --server ${location.origin} --code ${c.code}` };
  },
  async providers() {
    const r = await readiness(spaces.current.id);
    return r.machines.flatMap((m) => Object.entries(m.providers).flatMap(([id, p]) => p ? [{
      machineId: m.machine_id, machineName: m.name ?? "Machine", id, name: providerName(id),
      state: !p.enabled ? "off" as const : p.ready ? "signed_in" as const : !p.check ? "unknown" as const : !p.check.installed ? "not_installed" as const : !p.check.signed_in ? "not_signed_in" as const : "unknown" as const,
      command: p.check?.sign_in_command,
    }] : []));
  },
  check: (machineId, providerId) => checkProvider(spaces.current.id, machineId, providerId).then(() => undefined),
};

/** A simulation for fixtures: the machine connects on the third look after the code was made, and the provider signs in on the second check. */
export function fixtureApi(): OnboardingApi {
  let made = 0, looks = 0, checks = 0;
  const machine: MachineInfo = { id: "m-1", name: "dev-machine", os: "linux", arch: "x86_64", online: true };
  return {
    async machines() { await new Promise((r) => setTimeout(r, 450)); if (made && ++looks < 3) return []; return made ? [machine] : []; },
    async makeCode() { made++; return { code: "K7QM-4D2X", expiresMs: Date.now() + 15 * 60_000, command: "calternal machine enroll --server https://calternal.example --code K7QM-4D2X" }; },
    async providers() { return made ? [{ machineId: machine.id, machineName: machine.name, id: "calternal-claude-code", name: "Claude Code", state: checks >= 2 ? "signed_in" : "not_signed_in", command: "claude login" }] : []; },
    async check() { await new Promise((r) => setTimeout(r, 500)); checks++; },
  };
}
