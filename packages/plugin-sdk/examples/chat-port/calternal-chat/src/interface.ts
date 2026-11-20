import type { OperationContext } from "@calternal/plugin-sdk";
import type { AttachmentPort } from "./attachments.ts";

/** Fixture ports proposed on #78 and not in the 14.44 catalogue; a host may add them to the context. */
export type Ports = { attachments?: AttachmentPort };

/** Read the proposed ports a host added, if any. */
export const ports = (ctx: object): Ports => ctx as Ports;

/** The context of the undeclared people prototype handlers: every broker method plus the proposed ports. */
export type Context = OperationContext & Ports;
