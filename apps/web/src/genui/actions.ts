/** What a described action asks the client to do. The package describes actions and never performs them; this file is the client's side. */
import type { Value } from "@calternal/genui";

/** A described action as evaluated by `@calternal/genui`. */
export type Action = { kind: "action"; type: "set"; name: string; value: Value } | { kind: "action"; type: "send" | "copy"; text: string } | { kind: "action"; type: "openUrl"; url: string };

/** The client callbacks a rendered description can reach. Anything not given is ignored, so a card in a message cannot send or copy unless the host allows it. */
export interface Host {
  /** Puts an answer in the user's message (`Send`). */
  send?: (text: string) => void;
  /** Sets a state variable and re-evaluates. */
  set: (name: string, value: Value) => void;
}

/** Performs one action through the host. `OpenUrl` only ever opens http(s) in a new context with no opener. */
export function perform(a: Action, host: Host): void {
  if (a.type === "set") host.set(a.name, a.value);
  else if (a.type === "send") host.send?.(a.text);
  else if (a.type === "copy") void navigator.clipboard?.writeText(a.text);
  else if (a.type === "openUrl" && /^https?:\/\//i.test(a.url)) window.open(a.url, "_blank", "noopener,noreferrer");
}
