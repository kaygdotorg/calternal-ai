/** Hard per-description ceilings and iterative JSON preflight checks; callers can lower deployment ceilings.
 * @module
 */
import type { Pos } from './diagnostics.js';
/** Hard ceilings for one description and each evaluation; callers may lower these limits. */
export interface Budgets {
    /** Maximum UTF-8 bytes in source, external state, AST or evaluated output separately. */
    serializedBytes: number;
    /** Maximum AST expressions and maximum produced native nodes separately. */
    nodes: number;
    /** Maximum source/AST/value and evaluation dependency depth. */
    depth: number;
    /** Maximum declared or supplied top-level state entries. */
    stateEntries: number;
    /** Maximum expression visits in a complete initialization/evaluation pass. */
    steps: number;
    /** Maximum list length, including range/map outputs. */
    listLength: number;
}
/** Conservative engineering ceilings, not a measured device performance budget. */
export const DEFAULT_BUDGETS: Readonly<Budgets> = Object.freeze({ serializedBytes: 262144, nodes: 2000, depth: 64, stateEntries: 256, steps: 200000, listLength: 1000 });
/** Validate host-lowered limits; invalid fields throw Error. O(number of supplied fields),
 * one bounded record allocation; no locks, I/O, fsyncs or round trips. */
export function budgets(overrides: Partial<Budgets> = {}): Budgets {
    const out = { ...DEFAULT_BUDGETS };
    for (const [key, value] of Object.entries(overrides)) {
        if (!Object.hasOwn(out, key) || !Number.isSafeInteger(value) || value < 0 || value > out[key as keyof Budgets])
            throw new Error(`invalid budget: ${key}`);
        out[key as keyof Budgets] = value;
    }
    return out;
}
/** Count UTF-8 bytes rather than UTF-16 positions. O(text length), allocates an encoded
 * byte buffer; no locks, I/O, fsyncs or round trips. */
export function byteLength(text: string): number { return new TextEncoder().encode(text).length; }
/** A deterministic interpreter/validation failure with a source position. */
export class Failure extends Error {
    /** True for a resource ceiling: terminate the complete evaluation. */
    readonly limit: boolean;
    /** Position of the expression that failed. */
    readonly pos: Pos;
    /** Construct a source-positioned failure. O(message bytes) error/stack allocation,
     * no locks, I/O, fsyncs or network. Resource failures terminate the complete pass. */
    constructor(pos: Pos, message: string, limit = false) { super(message); this.pos = pos; this.limit = limit; }
}
/** Validate JSON before recursive evaluation. O(visited JSON bytes), with a traversal stack
 * and active-object set; throws Failure for ceilings, cycles, unsupported values or forbidden keys.
 * No locks, I/O, fsyncs or network. Shared acyclic values are visited once per occurrence. */
export function checkValue(value: unknown, limits: Budgets, pos: Pos): void {
    const stack: {
        /** Literal value or unevaluated expression belonging to this syntax item. */
        value: unknown;
        /** The depth value described by this contract. */
        depth: number;
        /** The exit value described by this contract. */
        exit?: boolean;
    }[] = [{ value, depth: 0 }];
    const active = new Set<object>();
    let bytes = 0;
    while (stack.length) {
        const { value: v, depth, exit } = stack.pop()!;
        if (exit) {
            active.delete(v as object);
            continue;
        }
        if (depth > limits.depth)
            throw new Failure(pos, 'budget exceeded: depth', true);
        if (v === null || typeof v === 'boolean' || typeof v === 'number' || typeof v === 'string') {
            if (typeof v === 'number' && !Number.isFinite(v))
                throw new Failure(pos, 'state must contain finite JSON values');
            bytes += byteLength(JSON.stringify(v));
        }
        else {
            if (typeof v !== 'object')
                throw new Failure(pos, 'state must contain finite JSON values');
            if (active.has(v))
                throw new Failure(pos, 'state must be an acyclic JSON tree');
            active.add(v);
            if (Array.isArray(v) && v.length > limits.listLength)
                throw new Failure(pos, 'budget exceeded: listLength', true);
            if (!Array.isArray(v) && Object.getPrototypeOf(v) !== Object.prototype && Object.getPrototypeOf(v) !== null)
                throw new Failure(pos, 'state must contain plain JSON objects');
            const entries = Object.entries(v);
            bytes += 2 + Math.max(0, entries.length - 1);
            stack.push({ value: v, depth, exit: true });
            for (const [key, item] of entries) {
                if (!Array.isArray(v)) {
                    if (['__proto__', 'prototype', 'constructor'].includes(key))
                        throw new Failure(pos, `\`${key}\` is not accessible`);
                    bytes += byteLength(JSON.stringify(key)) + 1;
                }
                stack.push({ value: item, depth: depth + 1 });
            }
        }
        if (bytes > limits.serializedBytes)
            throw new Failure(pos, 'budget exceeded: serializedBytes', true);
    }
}
/** Reject interpreter-tagged external state after checkValue has established an acyclic JSON tree.
 * O(visited state entries), allocates a local traversal stack; throws Failure on a reserved tag.
 * No I/O, locks, fsyncs, round trips or effects. */
export function checkState(state: unknown, pos: Pos): void {
    const pending: unknown[] = [state];
    while (pending.length) {
        const v = pending.pop();
        if (v !== null && typeof v === 'object') {
            if (Object.hasOwn(v, 'kind') && ['node', 'action', 'binding', 'function'].includes(String((v as Record<string, unknown>).kind)))
                throw new Failure(pos, 'reserved interpreter value kind');
            pending.push(...Object.values(v));
        }
    }
}
