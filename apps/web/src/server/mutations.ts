/**
 * The changes Settings and onboarding make that the server may ask a fresh passkey proof for: deleting or leaving a space, a machine's mode, pausing, revoking and enrolling, and turning a space's plugins on and off.
 * Each goes through `fresh` (`withFreshProof`: re-verify once on `passkey_reverification_required`, retry once, otherwise show the server's own words), so no screen matches on `forbidden` or runs the ceremony itself.
 * The account's own passkey and device changes are in `auth.ts` (`account`), the people, group and invitation changes in `people.ts`. One call each, plus one ceremony only when the proof is stale.
 */
import { fresh } from "./auth";
import { server } from "./client";

/** Deletes a space for everyone in it (the owner only). */
export const deleteSpace = (space_id: string): Promise<unknown> => fresh(() => server.operate("spaces.delete", { space_id }));
/** Leaves a space: the person loses access to everything in it. */
export const leaveSpace = (space_id: string): Promise<unknown> => fresh(() => server.operate("spaces.leave", { space_id }));
/** Turns a plugin on or off in a space. */
export const setPlugin = (space_id: string, plugin_id: string, on: boolean): Promise<unknown> => fresh(() => server.operate(on ? "plugins.enable" : "plugins.disable", { space_id, plugin_id }));

/** A machine route (`/api/v1/machines…`): JSON in, JSON out. */
const machine = <T,>(path: string, body: unknown): Promise<T> => fresh(() => server.json<T>(`/api/v1/machines${path}`, { body }));
/** Where threads run on a machine: sandboxed, or directly on it. */
export const setDirect = (machine_id: string, direct: boolean): Promise<unknown> => fresh(() => server.operate("machines.set", { machine_id, direct }));
/** Stops (or resumes) sending new work to a machine. */
export const drainMachine = (machine_id: string, draining: boolean): Promise<unknown> => machine(`/${machine_id}/drain`, { draining });
/** Revokes a machine: it can never connect again. */
export const revokeMachine = (machine_id: string): Promise<unknown> => machine(`/${machine_id}/revoke`, {});
/** Makes a one-use enrollment code for a machine to join with. */
export const enrollmentCode = (): Promise<{ code: string; expires_ms: number }> => machine<{ code: string; expires_ms: number }>("/codes", {});
/** Makes the one-line install script (single use, 15 minutes) that adds a machine from a terminal. */
export const installScript = (): Promise<{ script: string; expires_ms: number }> => fresh(() => server.operate<{ script: string; expires_ms: number }>("machines.install", {}));
