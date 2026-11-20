import type { ContributionCall, ContributionEntryInvoke, ContributionFrame } from "../generated/author-types.js";

/** Encode the whole JSON frame plus LF, throwing contribution_frame_limit when oversized. */
export function encodeContributionFrame(frame: ContributionCall | ContributionEntryInvoke | ContributionFrame): string;
