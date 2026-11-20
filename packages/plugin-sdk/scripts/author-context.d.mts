import type { RevisionedAuthorContext } from "../generated/author-types.js";

/** Adapt a validated host invoke without exposing transport credentials to the handler. */
export function authorContext(
  message: {
    author_context: RevisionedAuthorContext["author"];
    job_context?: RevisionedAuthorContext["job"];
    alarm?: RevisionedAuthorContext["alarm"];
  },
  call: RevisionedAuthorContext["call"],
): RevisionedAuthorContext;
