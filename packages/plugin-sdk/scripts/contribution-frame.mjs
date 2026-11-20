import { contributionFrameBytes } from "../generated/contribution-limit.mjs";

/** Encode a complete contribution frame under the pinned encoded byte limit. */
export function encodeContributionFrame(frame) {
  if (!Number.isSafeInteger(contributionFrameBytes)) {
    throw new Error("The pinned contract declares no contribution frame limit.");
  }
  const json = JSON.stringify(frame);
  if (json === undefined) throw new TypeError("Expected a JSON contribution frame.");
  const encoded = json + "\n";
  if (new TextEncoder().encode(encoded).byteLength > contributionFrameBytes) {
    throw Object.assign(new RangeError("Contribution frame exceeds the encoded byte limit."), {
      code: "contribution_frame_limit",
    });
  }
  return encoded;
}
