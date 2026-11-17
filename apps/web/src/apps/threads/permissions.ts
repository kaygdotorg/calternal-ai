/**
 * The words for the two per-thread access chips. The permission names are fixed by the product; the one-line explanations are
 * placeholder wording until the owner supplies the real text.
 */
import type { IconName } from "@calternal/ui";
import type { Permission, RunsOn } from "./model";

/** The three permission modes in menu order, with the chip's icon. */
export const PERMISSIONS: { id: Permission; label: string; sub: string; icon: IconName }[] = [
  { id: "least", label: "Least access", sub: "Asks before every change or command", icon: "lock" },
  { id: "approve", label: "Approve for me", sub: "Safe actions go ahead; the rest ask you", icon: "shield" },
  { id: "full", label: "Full access", sub: "Runs everything without asking", icon: "warning" },
];

/** A permission's display label. */
export const permissionLabel = (p: Permission): string => PERMISSIONS.find((x) => x.id === p)!.label;

/** "calternal-dev-02-std · Sandboxed". */
export const runsOnLabel = (r: RunsOn): string => `${r.machine} · ${r.mode === "sandboxed" ? "Sandboxed" : "Direct"}`;
