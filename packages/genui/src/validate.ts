/** Catalog, accessibility and dependency checks before a description can be activated.
 * @module
 */
import type { CallExpr, Decl, Expr, Program, Statement } from './ast.js';
import { ACTIONS, COMPONENTS, CONSTANTS, FORBIDDEN_PROPERTIES, type ComponentSpec } from './catalog.js';
import { DiagnosticList, type Pos } from './diagnostics.js';
import { byteLength, Failure, type Budgets } from './budgets.js';
/** Pure-function argument counts, inclusive; -1 means unbounded. */
export const FUNCTION_ARITY: Readonly<Record<string, readonly [
    number,
    number
]>> = { round: [1, 2], fmt: [1, 2], sum: [1, 1], min: [1, -1], max: [1, -1], abs: [1, 1], floor: [1, 1], ceil: [1, 1], sqrt: [1, 1], pow: [2, 2], sin: [1, 1], cos: [1, 1], tan: [1, 1], asin: [1, 1], acos: [1, 1], atan: [1, 1], atan2: [2, 2], clamp: [3, 3], lerp: [3, 3], range: [2, 3], random: [1, 1], keys: [1, 1], values: [1, 1], entries: [1, 1] };
/** Allowed methods and argument counts; arrow methods receive an item and optional index. */
export const METHOD_ARITY: Readonly<Record<string, readonly [
    number,
    number
]>> = { map: [1, 1], filter: [1, 1], find: [1, 1], some: [1, 1], every: [1, 1], join: [0, 1], slice: [0, 2], includes: [1, 1], indexOf: [1, 1], split: [1, 1], trim: [0, 0], toUpperCase: [0, 0], toLowerCase: [0, 0], startsWith: [1, 1], endsWith: [1, 1], toFixed: [0, 1] };
/** Methods whose argument is an unevaluated arrow expression. */
export const LAMBDA_METHODS: ReadonlySet<string> = new Set(['map', 'filter', 'find', 'some', 'every']);
/** Return immediate children in deterministic traversal order. O(immediate child count),
 * allocates at most one small result array; no locks, I/O, fsyncs or network. */
export function children(expr: Expr): readonly Expr[] {
    switch (expr.type) {
        case 'literal':
        case 'state':
        case 'ident': return [];
        case 'list': return expr.items;
        case 'object': return expr.entries.map(e => e.value);
        case 'template': return expr.parts;
        case 'unary': return [expr.operand];
        case 'binary': return [expr.left, expr.right];
        case 'conditional': return [expr.test, expr.then, expr.else];
        case 'member': return [expr.object];
        case 'index': return [expr.object, expr.index];
        case 'arrow': return [expr.body];
        case 'call': return [...expr.args, ...expr.named.map(a => a.value)];
        case 'method': return [expr.object, ...expr.args, ...expr.named.map(a => a.value)];
    }
}
/** Enforce AST node/depth/list/state/byte budgets before validation or evaluation.
 * O(AST size), allocating a bounded traversal stack; throws Failure on a ceiling.
 * No locks, I/O, fsyncs or network. Accepts only parser-produced acyclic ASTs. */
export function checkProgram(program: Program, limits: Budgets): void {
    const start = { line: 1, column: 1 };
    if (program.serializedBytes > limits.serializedBytes)
        throw new Failure(start, 'budget exceeded: serializedBytes', true);
    if (program.states.length > limits.stateEntries)
        throw new Failure(start, 'budget exceeded: stateEntries', true);
    const stack = [...program.states.map(d => d.value), ...[...program.named.values()].map(d => d.value), ...program.roots].map(expr => ({ expr, depth: 1 }));
    let count = 0;
    while (stack.length) {
        const { expr, depth } = stack.pop()!;
        if (++count > limits.nodes)
            throw new Failure(start, 'budget exceeded: nodes', true);
        if (depth > limits.depth)
            throw new Failure(start, 'budget exceeded: depth', true);
        if (expr.type === 'list' && expr.items.length > limits.listLength)
            throw new Failure(start, 'budget exceeded: listLength', true);
        stack.push(...children(expr).map(expr => ({ expr, depth: depth + 1 })));
    }
}
/** Associate arguments with catalog parameter names; first occurrence wins after static duplicate checks.
 * O(argument count), one bounded Map allocation; no locks, I/O, fsyncs or round trips. */
export function slots(expr: CallExpr, spec: ComponentSpec): Map<string, Expr> {
    const result = new Map<string, Expr>();
    expr.args.forEach((v, i) => {
        if (spec.params[i])
            result.set(spec.params[i].name, v);
    });
    for (const arg of expr.named)
        if (!result.has(arg.name))
            result.set(arg.name, arg.value);
    return result;
}
/** Check parser-produced statements against catalog/accessibility/dependency contracts.
 * Cost grows only with this bounded AST and its dependency walks, including transitive reads per
 * state initializer. Allocates local symbol/reference sets and diagnostics; no locks, I/O, fsyncs
 * or round trips. Reports errors into diagnostics; compile discards any resulting invalid program. */
export function validate(statements: readonly Statement[], diagnostics: DiagnosticList, limits: Budgets): Program {
    const states: Decl[] = [];
    const named = new Map<string, Decl>();
    const roots: Expr[] = [];
    for (const statement of statements) {
        if (statement.kind === 'root') {
            roots.push(statement.value);
            continue;
        }
        const table = statement.kind === 'state' ? states : named;
        if (FORBIDDEN_PROPERTIES.includes(statement.name))
            diagnostics.report(statement.pos, `\`${statement.name}\` is not accessible`);
        if (Object.hasOwn(CONSTANTS, statement.name))
            diagnostics.report(statement.pos, `reserved constant: ${statement.name}`);
        if (table instanceof Map ? table.has(statement.name) : table.some(d => d.name === statement.name))
            diagnostics.report(statement.pos, `duplicate declaration: ${statement.kind === 'state' ? '$' : ''}${statement.name}`);
        else if (table instanceof Map)
            table.set(statement.name, statement);
        else
            table.push(statement);
    }
    if (roots.length === 0 && named.has('root'))
        roots.push({ type: 'ident', name: 'root', pos: named.get('root')!.pos });
    if (!roots.length)
        diagnostics.report({ line: 1, column: 1 }, 'Nothing to render: add a bare component expression such as Card(...)');
    const program = { states, named, roots, serializedBytes: byteLength(JSON.stringify(statements)) };
    try {
        checkProgram(program, limits);
    }
    catch (error) {
        if (error instanceof Failure) {
            diagnostics.report(error.pos, error.message);
            return program;
        }
        throw error;
    }
    const stateNames = new Set(states.map(d => d.name));
    const forbidden = (name: string, pos: Pos) => {
        if (FORBIDDEN_PROPERTIES.includes(name))
            diagnostics.report(pos, `\`${name}\` is not accessible`);
    };
    const refs = new Map<string, Set<string>>();
    const stateRefs = new Map<string, Set<string>>();
    function walk(expr: Expr, params: readonly string[], names: Set<string>, reads: Set<string>, allowArrow = false): void {
        if (expr.type === 'ident') {
            if (!params.includes(expr.name) && !Object.hasOwn(CONSTANTS, expr.name)) {
                if (!named.has(expr.name))
                    diagnostics.report(expr.pos, `unknown name: ${expr.name}`);
                else
                    names.add(expr.name);
            }
        }
        else if (expr.type === 'state') {
            reads.add(expr.name);
            if (!stateNames.has(expr.name))
                diagnostics.report(expr.pos, `unknown state: $${expr.name}`);
        }
        else if (expr.type === 'member')
            forbidden(expr.name, expr.pos);
        else if (expr.type === 'index' && expr.index.type === 'literal' && typeof expr.index.value === 'string')
            forbidden(expr.index.value, expr.index.pos);
        else if (expr.type === 'object') {
            for (const e of expr.entries)
                forbidden(e.key, e.pos);
        }
        if (expr.type === 'arrow') {
            if (!allowArrow)
                diagnostics.report(expr.pos, 'arrow functions are only allowed in named functions and list methods');
            const seen = new Set<string>();
            for (const p of expr.params) {
                forbidden(p, expr.pos);
                if (seen.has(p))
                    diagnostics.report(expr.pos, `duplicate parameter: ${p}`);
                seen.add(p);
            }
            walk(expr.body, [...params, ...expr.params], names, reads);
            return;
        }
        if (expr.type === 'call') {
            const spec = Object.hasOwn(COMPONENTS, expr.name) ? COMPONENTS[expr.name] : Object.hasOwn(ACTIONS, expr.name) ? ACTIONS[expr.name] : undefined;
            if (spec) {
                const mapped = slots(expr, spec);
                const given = new Set(expr.args.slice(0, spec.params.length).map((_, i) => spec.params[i].name));
                if (expr.args.length > spec.params.length)
                    diagnostics.report(expr.pos, `too many arguments: ${expr.name}`);
                for (const arg of expr.named) {
                    if (!spec.params.some(p => p.name === arg.name))
                        diagnostics.report(arg.pos, `unknown argument: ${expr.name}.${arg.name}`);
                    if (given.has(arg.name))
                        diagnostics.report(arg.pos, `duplicate argument: ${arg.name}`);
                    given.add(arg.name);
                }
                for (const p of spec.params) {
                    const v = mapped.get(p.name);
                    const accessible = spec.accessibility?.required.includes(p.name);
                    if (!v) {
                        if (!p.optional)
                            diagnostics.report(expr.pos, accessible ? `accessibility: ${expr.name}.${p.name} is required` : `missing argument: ${expr.name}.${p.name}`);
                        continue;
                    }
                    if (accessible && v.type === 'literal' && (typeof v.value !== 'string' || !v.value.trim()))
                        diagnostics.report(v.pos, `accessibility: ${expr.name}.${p.name} must be nonblank text`);
                    if (p.kind === 'binding' && v.type !== 'state')
                        diagnostics.report(v.pos, `${expr.name}.${p.name} must be a $state reference`);
                    if (v.type === 'literal' && !(v.value === null && (p.optional || p.nullable))) {
                        if ((p.kind === 'number' && typeof v.value !== 'number') || (p.kind === 'boolean' && typeof v.value !== 'boolean') || p.kind === 'list')
                            diagnostics.report(v.pos, `${expr.name}.${p.name} needs ${p.kind}`);
                        if (p.kind === 'enum' && !p.values?.includes(String(v.value)))
                            diagnostics.report(v.pos, `${expr.name}.${p.name} must be one of ${p.values?.join(', ')}`);
                    }
                }
            }
            else {
                const own = named.get(expr.name)?.value;
                const arity = Object.hasOwn(FUNCTION_ARITY, expr.name) ? FUNCTION_ARITY[expr.name] : own?.type === 'arrow' ? [0, own.params.length] : undefined;
                if (!arity)
                    diagnostics.report(expr.pos, `unknown call: ${expr.name}`);
                else {
                    if (own?.type === 'arrow')
                        names.add(expr.name);
                    if (expr.named.length)
                        diagnostics.report(expr.named[0].pos, `${expr.name} takes positional arguments only`);
                    if (expr.args.length < arity[0] || (arity[1] >= 0 && expr.args.length > arity[1]))
                        diagnostics.report(expr.pos, `wrong argument count: ${expr.name}`);
                }
            }
        }
        if (expr.type === 'method') {
            const a = Object.hasOwn(METHOD_ARITY, expr.name) ? METHOD_ARITY[expr.name] : undefined;
            if (!a)
                diagnostics.report(expr.pos, `unknown method: ${expr.name}`);
            else if (expr.args.length < a[0] || expr.args.length > a[1])
                diagnostics.report(expr.pos, `wrong argument count: .${expr.name}`);
            if (expr.named.length)
                diagnostics.report(expr.named[0].pos, 'methods take positional arguments only');
            walk(expr.object, params, names, reads);
            expr.args.forEach((arg, i) => {
                if (i === 0 && LAMBDA_METHODS.has(expr.name)) {
                    if (arg.type !== 'arrow')
                        diagnostics.report(arg.pos, `.${expr.name} requires an arrow function`);
                    else if (arg.params.length > 2)
                        diagnostics.report(arg.pos, `.${expr.name} accepts at most two parameters`);
                    walk(arg, params, names, reads, true);
                }
                else
                    walk(arg, params, names, reads);
            });
            for (const arg of expr.named)
                walk(arg.value, params, names, reads);
            return;
        }
        for (const child of children(expr))
            walk(child, params, names, reads);
    }
    for (const [name, d] of named) {
        const n = new Set<string>(), s = new Set<string>();
        walk(d.value, [], n, s, true);
        refs.set(name, n);
        stateRefs.set(name, s);
    }
    for (const root of roots)
        walk(root, [], new Set(), new Set());
    const visiting = new Set<string>(), done = new Set<string>();
    function cycle(name: string): void {
        if (done.has(name))
            return;
        if (visiting.has(name)) {
            diagnostics.report(named.get(name)!.pos, `cyclic named value: ${name}`);
            return;
        }
        visiting.add(name);
        for (const r of refs.get(name) ?? [])
            cycle(r);
        visiting.delete(name);
        done.add(name);
    }
    for (const name of named.keys())
        cycle(name);
    function reachableState(name: string, out: Set<string>, seen = new Set<string>()): void {
        if (seen.has(name))
            return;
        seen.add(name);
        for (const s of stateRefs.get(name) ?? [])
            out.add(s);
        for (const r of refs.get(name) ?? [])
            reachableState(r, out, seen);
    }
    states.forEach((d, i) => {
        const n = new Set<string>(), s = new Set<string>();
        walk(d.value, [], n, s);
        for (const r of n)
            reachableState(r, s);
        for (const ref of s) {
            if (states.findIndex(d => d.name === ref) >= i)
                diagnostics.report(d.pos, `state initializer reads uninitialized state: $${ref}`);
        }
    });
    return program;
}
