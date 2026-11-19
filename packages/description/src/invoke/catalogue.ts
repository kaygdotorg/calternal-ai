/**
 * What the host catalogue says about an operation, read from the catalogue itself (never from the description): whether a sheet-free one-exchange mutation is allowed (`atomic_undo`) and whether a full swipe is
 * (`full_swipe`). A plugin operation is not in the catalogue and gets neither: it always takes the ordinary path. Named import so only the operation table is bundled.
 */
import { operations } from "../../../../contract/host-catalogue-v2.json";

interface Op { atomic_undo?: boolean; full_swipe?: boolean; generic_invoke?: boolean }
const table = operations as unknown as Record<string, Op>;

/** Whether the catalogue permits resolve and dispatch in one exchange for `operation` (a host built-in whose engine commits its effect and Undo together). */
export const oneExchange = (operation: string): boolean => table[operation]?.atomic_undo === true;
/** Whether `operation` may be a full swipe. The verifier rejects it elsewhere; the renderer also refuses to honour it. */
export const fullSwipeAllowed = (operation: string): boolean => table[operation]?.full_swipe === true;
