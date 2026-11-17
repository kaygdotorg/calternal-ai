/** Small capped diagnostic collector with stable source locations and deterministic ordering.
 * @module
 */
import type { Diagnostic } from "./types.js";
/** A 1-based source position. Tokens and AST nodes carry one. */
export interface Pos {
    /** One-based source line. */
    readonly line: number;
    /** One-based UTF-16 source column. */
    readonly column: number;
}
const MAX_DIAGNOSTICS = 20;
/** Collects up to 20 diagnostics, dropping exact duplicates. */
export class DiagnosticList {
    /** Positioned unique diagnostics in reporting order, capped at twenty. */
    readonly items: Diagnostic[] = [];
    /** True once any error has been reported, including after the item cap. */
    hasErrors = false;
    private readonly seen = new Set<string>();
    /** Record a unique diagnostic in O(message bytes), bounded to twenty stored items; no I/O or locks. */
    report(pos: Pos, message: string, severity: Diagnostic["severity"] = "error"): void {
        if (severity === "error")
            this.hasErrors = true;
        const id = `${pos.line}:${pos.column}:${message}`;
        if (this.items.length >= MAX_DIAGNOSTICS || this.seen.has(id))
            return;
        this.seen.add(id);
        this.items.push({ severity, line: pos.line, column: pos.column, message });
    }
}
