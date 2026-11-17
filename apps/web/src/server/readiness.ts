/**
 * Machine and provider readiness (docs/agent-turns.md, "Machine and provider readiness"): per machine of the signed-in account, whether it is connected, how it runs turns, and for each provider whether it is installed and
 * signed in there, with the provider program's own sign-in command (shown, never run here). `agent.readiness` reads the server's cache; `agent.check` refreshes one provider on one machine, and the account feed
 * (`account/live?readiness=true`) says when to read again. The types are the generated ones from `packages/contracts`.
 */
import type { PersonalMachineReadiness, ProviderCheck, ProviderReadiness, ReadinessResult } from "../../../../packages/contracts/src/index";
import { server } from "./client";

export type { ProviderCheck, ProviderReadiness };
/** One of the person's machines. */
export type MachineState = PersonalMachineReadiness;
/** The cached readiness read. */
export type Readiness = ReadinessResult;

/** Reads the cache for a space (no machine traffic). */
export const readiness = (space_id: string): Promise<Readiness> => server.operate<Readiness>("agent.readiness", { space_id });
/** Asks the machine to run the provider's own check now; the answer arrives through the account feed and a new read. */
export const checkProvider = (space_id: string, machine_id: string, provider_id: string) => server.operate("agent.check", { space_id, machine_id, provider_id });

/** A provider's name for a person: `calternal-claude-code` → "Claude Code". */
export const providerName = (id: string): string => id.replace(/^calternal-/, "").split("-").map((w) => w[0]!.toUpperCase() + w.slice(1)).join(" ");

/**
 * Follows the account feed for readiness changes: calls `onChange` for each (and once at the start, to read), reconnecting after a drop until stopped. Returns the stop function. A server without the feed simply
 * never calls back after the first read.
 */
export function watchReadiness(onChange: () => void): () => void {
  const ac = new AbortController();
  void (async () => {
    onChange();
    while (!ac.signal.aborted) {
      try { await server.readinessLive(onChange, ac.signal); } catch { /* the feed dropped */ }
      if (ac.signal.aborted) break;
      await new Promise((r) => setTimeout(r, 3000));
      onChange(); // a snapshot after a reconnect: frames in between are not replayed
    }
  })();
  return () => ac.abort();
}
