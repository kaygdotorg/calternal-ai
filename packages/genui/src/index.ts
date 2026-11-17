/** Portable GenUI language entry points. Parsing and evaluation are pure and bounded; no renderer is included. */
import type { Program, Statement } from './ast.js';
import { budgets, byteLength, Failure, type Budgets } from './budgets.js';
import { DiagnosticList } from './diagnostics.js';
import { tokenize } from './lexer.js';
import { parse } from './parser.js';
import { validate } from './validate.js';
import type { Diagnostic } from './types.js';
export type { 
/** Public package export of Program; see its defining module for invariants, errors and costs. */
Program, 
/** Public package export of Statement; see its defining module for invariants, errors and costs. */
Statement, 
/** Public package export of Expr; see its defining module for invariants, errors and costs. */
Expr, 
/** Public package export of Decl; see its defining module for invariants, errors and costs. */
Decl, 
/** Public package export of NamedArg; see its defining module for invariants, errors and costs. */
NamedArg, 
/** Public package export of BinaryOperator; see its defining module for invariants, errors and costs. */
BinaryOperator } from './ast.js';
export type { 
/** Public package export of Diagnostic; see its defining module for invariants, errors and costs. */
Diagnostic, 
/** Public package export of Value; see its defining module for invariants, errors and costs. */
Value, 
/** Public package export of Scalar; see its defining module for invariants, errors and costs. */
Scalar, 
/** Public package export of UiNode; see its defining module for invariants, errors and costs. */
UiNode, 
/** Public package export of UiState; see its defining module for invariants, errors and costs. */
UiState } from './types.js';
export type { 
/** Public package export of Pos; see its defining module for invariants, errors and costs. */
Pos } from './diagnostics.js';
export type { 
/** Public package export of Token; see its defining module for invariants, errors and costs. */
Token } from './lexer.js';
export { 
/** Public package export of DEFAULT_BUDGETS; see its defining module for invariants, errors and costs. */
DEFAULT_BUDGETS, 
/** Public package export of budgets; see its defining module for invariants, errors and costs. */
budgets, 
/** Public package export of Budgets; see its defining module for invariants, errors and costs. */
type Budgets } from './budgets.js';
export { 
/** Public package export of COMPONENTS; see its defining module for invariants, errors and costs. */
COMPONENTS, 
/** Public package export of ACTIONS; see its defining module for invariants, errors and costs. */
ACTIONS, 
/** Public package export of FUNCTIONS; see its defining module for invariants, errors and costs. */
FUNCTIONS, 
/** Public package export of METHODS; see its defining module for invariants, errors and costs. */
METHODS, 
/** Public package export of CONSTANTS; see its defining module for invariants, errors and costs. */
CONSTANTS, 
/** Public package export of FORBIDDEN_PROPERTIES; see its defining module for invariants, errors and costs. */
FORBIDDEN_PROPERTIES, 
/** Public package export of ComponentSpec; see its defining module for invariants, errors and costs. */
type ComponentSpec, 
/** Public package export of ParamSpec; see its defining module for invariants, errors and costs. */
type ParamSpec } from './catalog.js';
export { 
/** Public package export of evaluate; see its defining module for invariants, errors and costs. */
evaluate, 
/** Public package export of initialState; see its defining module for invariants, errors and costs. */
initialState, 
/** Public package export of Evaluation; see its defining module for invariants, errors and costs. */
type Evaluation } from './evaluate.js';
/** Parsed/validated program; syntax or validation errors always leave program null. */
export interface Compilation {
    /** Accepted program; compile owns creation. */
    program: Program | null;
    /** Parsed statement AST, including statements with semantic errors. */
    statements: Statement[];
    /** Source-ordered diagnostics, capped at twenty. */
    diagnostics: Diagnostic[];
}
/** Strip token-only fields from AST source positions so both languages emit the same JSON. */
function normalize<T>(value: T): T {
    if (Array.isArray(value))
        return value.map(normalize) as T;
    if (value !== null && typeof value === 'object')
        return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, k === 'pos' ? { line: v.line, column: v.column } : normalize(v)])) as T;
    return value;
}
/**
 * Parse and validate the upstream expression syntax against the native data catalog.
 * Errors prevent activation. Costs O(source bytes + bounded AST/dependency walks); allocates
 * tokens, AST and per-program symbol tables. No locks, I/O, fsyncs or network round trips.
 */
export function compile(source: string, overrides: Partial<Budgets> = {}): Compilation {
    const diagnostics = new DiagnosticList();
    let limits: Budgets;
    try {
        limits = budgets(overrides);
    }
    catch (error) {
        diagnostics.report({ line: 1, column: 1 }, (error as Error).message);
        return { program: null, statements: [], diagnostics: diagnostics.items };
    }
    if (byteLength(source) > limits.serializedBytes) {
        diagnostics.report({ line: 1, column: 1 }, 'budget exceeded: serializedBytes');
        return { program: null, statements: [], diagnostics: diagnostics.items };
    }
    const statements = normalize(parse(tokenize(source, diagnostics), diagnostics));
    let program: Program | null = null;
    if (!diagnostics.hasErrors) {
        if (byteLength(JSON.stringify(statements)) > limits.serializedBytes)
            diagnostics.report({ line: 1, column: 1 }, 'budget exceeded: serializedBytes');
        else
            try {
                program = validate(statements, diagnostics, limits);
            }
            catch (error) {
                if (!(error instanceof Failure))
                    throw error;
                diagnostics.report(error.pos, error.message);
            }
    }
    diagnostics.items.sort((a, b) => a.line - b.line || a.column - b.column);
    return { program: diagnostics.hasErrors ? null : program, statements, diagnostics: diagnostics.items };
}
/**
 * Render diagnostics as source-positioned lines. O(total message bytes), one result string
 * allocation plus line strings; no locks, I/O, fsyncs or network round trips.
 */
export function formatDiagnostics(diagnostics: readonly Diagnostic[]): string { return diagnostics.map(d => `${d.line}:${d.column} ${d.severity}: ${d.message}`).join('\n'); }
