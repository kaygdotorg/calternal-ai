/** JSON-only client boundary records for native descriptions, local state and positioned diagnostics.
 * @module
 */
/** JSON scalar supported by the portable expression language. */
export type Scalar = string | number | boolean | null;
/** JSON-only evaluator value; tagged nodes, bindings and actions are ordinary records. */
export type Value = Scalar | Value[] | {
    /** JSON object entries keyed by literal property names. */
    [key: string]: Value;
};
/** Current local description state; names exclude the `$` prefix. */
export type UiState = Readonly<Record<string, Value>>;
/** Positioned diagnostic; positions count UTF-16 code units, starting at one. */
export interface Diagnostic {
    /** Error prevents activation; warning is informational. */
    severity: 'error' | 'warning';
    /** One-based source line. */
    line: number;
    /** One-based UTF-16 source column. */
    column: number;
    /** Stable language-neutral diagnostic text. */
    message: string;
}
/** Evaluated native component description; this package never draws views or performs actions. */
export interface UiNode {
    /** Interpreter-owned value discriminator. */
    kind: 'node';
    /** Catalog name, or Error when a component expression fails. */
    type: string;
    /** Source/iteration-derived identity, stable across state changes. */
    key: string;
    /** Normalized arguments keyed by catalog parameter names. */
    props: Record<string, Value>;
    /** Structural compatibility with JSON values. */
    [key: string]: Value;
}
