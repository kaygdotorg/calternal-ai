/** Pure bounded interpreter. Values crossing this boundary are JSON, never executable callbacks. */
import type { CallExpr, Expr, Program } from './ast.js';
import { ACTIONS, COMPONENTS, CONSTANTS, FORBIDDEN_PROPERTIES, type ComponentSpec, type ParamSpec } from './catalog.js';
import { budgets, byteLength, checkState, checkValue, Failure, type Budgets } from './budgets.js';
import { DiagnosticList, type Pos } from './diagnostics.js';
import { checkProgram, LAMBDA_METHODS, slots } from './validate.js';
import type { Diagnostic, UiNode, UiState, Value } from './types.js';
const START = { line: 1, column: 1 };
const truthy = (v: Value): boolean => v !== null && v !== false && v !== 0 && v !== '';
const kind = (v: Value): string => v === null ? 'null' : Array.isArray(v) ? 'list' : typeof v;
function text(v: Value, pos: Pos): string {
    if (v !== null && typeof v === 'object')
        throw new Failure(pos, 'cannot convert a structured value to text');
    return v === null ? '' : String(v);
}
function number(v: Value, pos: Pos): number {
    if (typeof v !== 'number')
        throw new Failure(pos, 'expected a number');
    return v;
}
function finite(v: number): Value { return Number.isFinite(v) ? (Object.is(v, -0) ? 0 : v) : null; }
function record(v: Value): v is Record<string, Value> { return v !== null && !Array.isArray(v) && typeof v === 'object'; }
function same(a: Value, b: Value): boolean { return a === b; }
function round(value: number, digits: number): number {
    const n = Math.abs(value);
    let r = Number(`${Math.round(Number(`${n}e${digits}`))}e${-digits}`);
    if (!Number.isFinite(r))
        r = Math.round(n * 10 ** digits) / 10 ** digits;
    return value < 0 ? -r : r;
}
function random(i: number): number {
    const bits = new DataView(new ArrayBuffer(8));
    bits.setFloat64(0, i === 0 ? 0 : i);
    let h = bits.getUint32(0) ^ Math.imul(bits.getUint32(4) ^ 0x9e3779b9, 0x85ebca6b);
    h = Math.imul(h ^ (h >>> 16), 0x85ebca6b);
    h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35);
    h ^= h >>> 16;
    return (h >>> 0) / 4294967296;
}
class Interpreter {
    steps = 0;
    nodes = 0;
    depth = 0;
    suffix = '';
    readonly diagnostics = new DiagnosticList();
    private readonly cache = new Map<string, Value>();
    constructor(readonly program: Program, readonly state: Record<string, Value>, readonly limits: Budgets) { }
    expr(e: Expr, scope: ReadonlyMap<string, Value> = new Map()): Value {
        if (++this.steps > this.limits.steps)
            throw new Failure(e.pos, 'budget exceeded: steps', true);
        if (++this.depth > this.limits.depth)
            throw new Failure(e.pos, 'budget exceeded: depth', true);
        try {
            const v = this.compute(e, scope);
            checkValue(v, this.limits, e.pos);
            return v;
        }
        finally {
            this.depth--;
        }
    }
    private compute(e: Expr, scope: ReadonlyMap<string, Value>): Value {
        switch (e.type) {
            case 'literal': return e.value;
            case 'state': return Object.hasOwn(this.state, e.name) ? this.state[e.name] : null;
            case 'ident': {
                if (scope.has(e.name))
                    return scope.get(e.name)!;
                if (Object.hasOwn(CONSTANTS, e.name))
                    return CONSTANTS[e.name];
                if (this.cache.has(e.name))
                    return this.cache.get(e.name)!;
                const d = this.program.named.get(e.name);
                if (!d)
                    throw new Failure(e.pos, `unknown name: ${e.name}`);
                const suffix = this.suffix;
                this.suffix = '';
                try {
                    const v = this.expr(d.value);
                    this.cache.set(e.name, v);
                    return v;
                }
                finally {
                    this.suffix = suffix;
                }
            }
            case 'list': return e.items.map(v => this.expr(v, scope));
            case 'object': {
                const v: Record<string, Value> = {};
                for (const entry of e.entries) {
                    this.forbidden(entry.key, entry.pos);
                    const item = this.expr(entry.value, scope);
                    if (entry.key === 'kind' && ['node', 'action', 'binding', 'function'].includes(String(item)))
                        throw new Failure(entry.pos, 'reserved interpreter value kind');
                    v[entry.key] = item;
                }
                return v;
            }
            case 'template': {
                let s = e.quasis[0];
                for (let i = 0; i < e.parts.length; i++)
                    s += text(this.expr(e.parts[i], scope), e.parts[i].pos) + e.quasis[i + 1];
                return s;
            }
            case 'unary': {
                const v = this.expr(e.operand, scope);
                return e.op === '!' ? !truthy(v) : finite(-number(v, e.pos));
            }
            case 'conditional': return this.expr(truthy(this.expr(e.test, scope)) ? e.then : e.else, scope);
            case 'binary': {
                const a = this.expr(e.left, scope);
                if (e.op === '&&')
                    return truthy(a) ? this.expr(e.right, scope) : a;
                if (e.op === '||')
                    return truthy(a) ? a : this.expr(e.right, scope);
                if (e.op === '??')
                    return a === null ? this.expr(e.right, scope) : a;
                const b = this.expr(e.right, scope);
                if (['==', '==='].includes(e.op))
                    return same(a, b);
                if (['!=', '!=='].includes(e.op))
                    return !same(a, b);
                if (['<', '<=', '>', '>='].includes(e.op)) {
                    if (!((typeof a === 'number' && typeof b === 'number') || (typeof a === 'string' && typeof b === 'string')))
                        throw new Failure(e.pos, 'comparison needs two numbers or two strings');
                    return e.op === '<' ? a < b : e.op === '<=' ? a <= b : e.op === '>' ? a > b : a >= b;
                }
                if (e.op === '+' && (typeof a === 'string' || typeof b === 'string'))
                    return text(a, e.pos) + text(b, e.pos);
                const x = number(a, e.pos), y = number(b, e.pos);
                return finite(e.op === '+' ? x + y : e.op === '-' ? x - y : e.op === '*' ? x * y : e.op === '/' ? x / y : e.op === '%' ? x % y : x ** y);
            }
            case 'member': return this.member(this.expr(e.object, scope), e.name, e.pos);
            case 'index': {
                const object = this.expr(e.object, scope), index = this.expr(e.index, scope);
                if (object === null)
                    return null;
                if (Array.isArray(object) || typeof object === 'string') {
                    const i = number(index, e.pos);
                    return Number.isInteger(i) && i >= 0 && i < object.length ? (typeof object === 'string' ? object[i].toWellFormed() : object[i]) : null;
                }
                if (record(object) && typeof index === 'string')
                    return this.member(object, index, e.pos);
                throw new Failure(e.pos, 'invalid index receiver or key');
            }
            case 'arrow': throw new Failure(e.pos, 'arrow functions are only allowed in named functions and list methods');
            case 'method': return this.method(e, scope);
            case 'call': {
                if (Object.hasOwn(COMPONENTS, e.name))
                    return this.component(e, COMPONENTS[e.name], scope);
                if (Object.hasOwn(ACTIONS, e.name))
                    return this.action(e, scope);
                const own = this.program.named.get(e.name)?.value;
                if (own?.type === 'arrow') {
                    const args = e.args.map(a => this.expr(a, scope));
                    const inner = new Map<string, Value>();
                    own.params.forEach((p, i) => inner.set(p, args[i] ?? null));
                    const suffix = this.suffix;
                    this.suffix += `@${e.pos.line}:${e.pos.column}`;
                    try {
                        return this.expr(own.body, inner);
                    }
                    finally {
                        this.suffix = suffix;
                    }
                }
                return this.call(e, e.args.map(a => this.expr(a, scope)));
            }
        }
    }
    private forbidden(name: string, pos: Pos): void {
        if (FORBIDDEN_PROPERTIES.includes(name))
            throw new Failure(pos, `\`${name}\` is not accessible`);
    }
    private member(v: Value, name: string, pos: Pos): Value {
        this.forbidden(name, pos);
        if (v === null)
            return null;
        if (record(v))
            return Object.hasOwn(v, name) ? v[name] : null;
        if ((Array.isArray(v) || typeof v === 'string') && name === 'length')
            return v.length;
        throw new Failure(pos, `cannot read property: ${name}`);
    }
    private node(type: string, key: string, props: Record<string, Value>, pos: Pos): UiNode {
        if (++this.nodes > this.limits.nodes)
            throw new Failure(pos, 'budget exceeded: nodes', true);
        return { kind: 'node', type, key, props };
    }
    children(v: Value, pos: Pos, key: string): UiNode[] {
        const out: UiNode[] = [];
        const visit = (v: Value): void => {
            if (v === null || v === false)
                return;
            if (typeof v === 'string' || typeof v === 'number')
                out.push(this.node('Text', `${key}.${out.length}`, { text: String(v) }, pos));
            else if (Array.isArray(v))
                v.forEach(visit);
            else if (record(v) && v.kind === 'node')
                out.push(v as UiNode);
            else
                throw new Failure(pos, 'children must be components or text');
        };
        visit(v);
        return out;
    }
    private actions(v: Value, pos: Pos): Value[] {
        if (v === null || v === false)
            return [];
        if (Array.isArray(v))
            return v.flatMap(i => this.actions(i, pos));
        if (record(v) && v.kind === 'action')
            return [v];
        throw new Failure(pos, 'expected action descriptions');
    }
    private param(owner: string, p: ParamSpec, e: Expr, scope: ReadonlyMap<string, Value>, key: string): Value | undefined {
        if (p.kind === 'binding') {
            if (e.type !== 'state')
                throw new Failure(e.pos, `${owner}.${p.name} must be a $state reference`);
            let v = Object.hasOwn(this.state, e.name) ? this.state[e.name] : null;
            const fits = v === null || !p.holds || p.holds === 'scalar' && ['string', 'number', 'boolean'].includes(typeof v) || p.holds === 'list' && Array.isArray(v) || p.holds === 'text' && typeof v === 'string' || p.holds === kind(v);
            if (!fits) {
                if (p.holds === 'list' && v !== null && typeof v !== 'object')
                    v = v === '' ? [] : [v];
                else if (p.holds === 'number' && typeof v === 'string' && /^\s*-?\d+(\.\d+)?\s*$/.test(v))
                    v = Number(v);
                else if (p.holds === 'text' && (typeof v === 'number' || typeof v === 'boolean'))
                    v = String(v);
                else
                    throw new Failure(e.pos, `${owner}.${p.name} state needs ${p.holds}`);
            }
            return { kind: 'binding', name: e.name, value: v };
        }
        const v = this.expr(e, scope);
        if (p.kind === 'nodes')
            return this.children(v, e.pos, key);
        if (p.kind === 'actions')
            return this.actions(v, e.pos);
        if (v === null) {
            if (p.optional)
                return undefined;
            if (p.nullable || p.kind === 'any')
                return null;
        }
        if ((p.kind === 'string' || p.kind === 'text') && v !== null && typeof v !== 'object')
            return String(v);
        if (p.kind === 'number' && typeof v === 'number' || p.kind === 'boolean' && typeof v === 'boolean' || p.kind === 'list' && Array.isArray(v) || p.kind === 'any')
            return v;
        if (p.kind === 'enum' && typeof v === 'string' && p.values?.includes(v))
            return v;
        if (p.kind === 'series') {
            const nums = (v: Value) => Array.isArray(v) && v.every(n => n === null || typeof n === 'number');
            if (nums(v) || record(v) && Object.values(v).every(s => nums(s) || record(s) && nums(s.values)))
                return v;
        }
        throw new Failure(e.pos, `${owner}.${p.name} needs ${p.kind}`);
    }
    private component(e: CallExpr, spec: ComponentSpec, scope: ReadonlyMap<string, Value>): Value {
        const key = `${e.pos.line}:${e.pos.column}${this.suffix}`;
        try {
            const args = slots(e, spec), props: Record<string, Value> = {};
            for (const p of spec.params) {
                const arg = args.get(p.name);
                if (arg) {
                    const v = this.param(e.name, p, arg, scope, key);
                    if (v !== undefined)
                        props[p.name] = v;
                }
                else if (!p.optional)
                    throw new Failure(e.pos, `missing argument: ${e.name}.${p.name}`);
            }
            for (const field of spec.accessibility?.required ?? []) {
                const v = props[field];
                if (typeof v !== 'string' || !v.trim())
                    throw new Failure(e.pos, `accessibility: ${e.name}.${field} must be nonblank text`);
            }
            return this.node(e.name, key, props, e.pos);
        }
        catch (error) {
            if (!(error instanceof Failure) || error.limit)
                throw error;
            this.diagnostics.report(error.pos, error.message);
            return this.node('Error', key, { message: error.message }, e.pos);
        }
    }
    private action(e: CallExpr, scope: ReadonlyMap<string, Value>): Value {
        const args = slots(e, ACTIONS[e.name]);
        if (e.name === 'Set') {
            const target = args.get('state');
            if (target?.type !== 'state')
                throw new Failure(e.pos, 'Set.state must be a $state reference');
            return { kind: 'action', type: 'set', name: target.name, value: this.expr(args.get('value')!, scope) };
        }
        const value = this.expr(args.get(e.name === 'OpenUrl' ? 'url' : 'text')!, scope);
        const t = text(value, e.pos);
        if (e.name === 'OpenUrl') {
            if (!/^https?:\/\//i.test(t))
                throw new Failure(e.pos, 'OpenUrl needs an http:// or https:// URL');
            return { kind: 'action', type: 'openUrl', url: t };
        }
        return { kind: 'action', type: e.name === 'Send' ? 'send' : 'copy', text: t };
    }
    private digits(v: Value | undefined, pos: Pos): number {
        const n = v === undefined ? 0 : number(v, pos);
        if (!Number.isInteger(n) || n < 0 || n > 10)
            throw new Failure(pos, 'digits must be an integer from 0 to 10');
        return n;
    }
    private call(e: CallExpr, a: Value[]): Value {
        const n = (i: number) => number(a[i], e.pos), all = (v: Value): number[] => {
            if (!Array.isArray(v))
                throw new Failure(e.pos, 'expected a list of numbers');
            return v.filter(x => x !== null).map(x => number(x, e.pos));
        };
        switch (e.name) {
            case 'round': return finite(round(n(0), this.digits(a[1], e.pos)));
            case 'fmt': {
                const digits = a[1] === undefined ? undefined : this.digits(a[1], e.pos);
                let fixed = round(n(0), digits ?? 2).toFixed(digits ?? 2);
                if (digits === undefined)
                    fixed = fixed.replace(/\.0+$|(\.\d*?)0+$/, '$1');
                const [whole, fraction] = fixed.split('.');
                const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
                return fraction === undefined ? grouped : `${grouped}.${fraction}`;
            }
            case 'abs': return Math.abs(n(0));
            case 'floor': return Math.floor(n(0));
            case 'ceil': return Math.ceil(n(0));
            case 'sqrt': return finite(Math.sqrt(n(0)));
            case 'pow': return finite(n(0) ** n(1));
            case 'sin':
            case 'cos':
            case 'tan':
            case 'asin':
            case 'acos':
            case 'atan': return finite(Math[e.name](n(0)));
            case 'atan2': return finite(Math.atan2(n(0), n(1)));
            case 'clamp': return Math.min(n(2), Math.max(n(1), n(0)));
            case 'lerp': return finite(n(0) + (n(1) - n(0)) * n(2));
            case 'random': return random(n(0));
            case 'sum': return finite(all(a[0]).reduce((s, n) => s + n, 0));
            case 'min':
            case 'max': {
                const v = a.length === 1 && Array.isArray(a[0]) ? all(a[0]) : a.map(x => number(x, e.pos));
                return v.length ? finite(e.name === 'min' ? Math.min(...v) : Math.max(...v)) : null;
            }
            case 'range': {
                const start = n(0), end = n(1), step = a.length > 2 ? n(2) : 1;
                if (step === 0)
                    throw new Failure(e.pos, 'range step cannot be zero');
                const count = Math.max(0, Math.ceil((end - start) / step));
                if (count > this.limits.listLength)
                    throw new Failure(e.pos, 'budget exceeded: listLength', true);
                return Array.from({ length: count }, (_, i) => finite(start + i * step));
            }
            case 'keys':
            case 'values':
            case 'entries': {
                if (!record(a[0]))
                    throw new Failure(e.pos, 'expected an object');
                return e.name === 'keys' ? Object.keys(a[0]) : e.name === 'values' ? Object.values(a[0]) : Object.entries(a[0]);
            }
            default: throw new Failure(e.pos, `unknown call: ${e.name}`);
        }
    }
    private method(e: Extract<Expr, {
        /** Syntax or interpreter-value discriminator; determines the other fields. */
        type: 'method';
    }>, scope: ReadonlyMap<string, Value>): Value {
        const v = this.expr(e.object, scope);
        if (Array.isArray(v) && LAMBDA_METHODS.has(e.name)) {
            const fn = e.args[0];
            if (fn?.type !== 'arrow')
                throw new Failure(e.pos, `.${e.name} requires an arrow function`);
            const base = this.suffix;
            const out: Value[] = [];
            try {
                for (let i = 0; i < v.length; i++) {
                    const inner = new Map(scope);
                    fn.params.forEach((p, k) => inner.set(p, k === 0 ? v[i] : i));
                    this.suffix = e.name === 'map' ? `${base}#${i}` : base;
                    const item = this.expr(fn.body, inner), yes = truthy(item);
                    if (e.name === 'map')
                        out.push(item);
                    else if (e.name === 'filter' && yes)
                        out.push(v[i]);
                    else if (e.name === 'find' && yes)
                        return v[i];
                    else if (e.name === 'some' && yes)
                        return true;
                    else if (e.name === 'every' && !yes)
                        return false;
                }
                return e.name === 'find' ? null : e.name === 'some' ? false : e.name === 'every' ? true : out;
            }
            finally {
                this.suffix = base;
            }
        }
        const a = e.args.map(x => this.expr(x, scope));
        const str = (i: number) => {
            if (typeof a[i] !== 'string')
                throw new Failure(e.pos, 'expected text');
            return a[i] as string;
        };
        const bounds = () => a.map(x => number(x, e.pos));
        if (Array.isArray(v)) {
            if (e.name === 'join')
                return v.map(x => text(x, e.pos)).join(a.length ? str(0) : ',');
            if (e.name === 'slice')
                return v.slice(...bounds());
            if (e.name === 'includes')
                return v.includes(a[0]);
            if (e.name === 'indexOf')
                return v.indexOf(a[0]);
        }
        else if (typeof v === 'string') {
            switch (e.name) {
                case 'includes': return v.includes(str(0));
                case 'startsWith': return v.startsWith(str(0));
                case 'endsWith': return v.endsWith(str(0));
                case 'slice': return v.slice(...bounds()).toWellFormed();
                case 'split': return v.split(str(0)).map(s => s.toWellFormed());
                case 'trim': return v.trim();
                case 'toUpperCase': return v.toUpperCase();
                case 'toLowerCase': return v.toLowerCase();
            }
        }
        else if (typeof v === 'number' && e.name === 'toFixed') {
            const d = this.digits(a[0], e.pos);
            return round(v, d).toFixed(d);
        }
        throw new Failure(e.pos, `method unavailable: ${e.name}`);
    }
}
/** One complete evaluation snapshot, safe to hand to a native tree walker. */
export interface Evaluation {
    /** Initialized state, or the supplied state snapshot. */
    state: Record<string, Value>;
    /** Root native descriptions; resource failures discard the entire output. */
    roots: UiNode[];
    /** Deterministic errors in evaluation order. */
    diagnostics: Diagnostic[];
}
/**
 * Initialize/evaluate a validated program, with optional external state. Limits are rechecked on
 * every call, including AST, state, produced nodes, steps, value depth and serialized output.
 * Costs O(expression steps × bounded value bytes + local name lookups), bounded by Budgets; allocates a local
 * state copy, named-value cache and output tree. No locks, I/O, fsyncs, network or effects.
 */
export function evaluate(program: Program, state?: UiState, overrides: Partial<Budgets> = {}): Evaluation {
    return run(program, state, overrides, false);
}
function run(program: Program, state: UiState | undefined, overrides: Partial<Budgets>, initialOnly: boolean): Evaluation {
    let limits: Budgets;
    try {
        limits = budgets(overrides);
    }
    catch (error) {
        return { state: {}, roots: [], diagnostics: [{ severity: 'error', line: 1, column: 1, message: (error as Error).message }] };
    }
    const values: Record<string, Value> = {};
    const interpreter = new Interpreter(program, values, limits);
    try {
        checkProgram(program, limits);
        if (state !== undefined) {
            if (Object.keys(state).length > limits.stateEntries)
                throw new Failure(START, 'budget exceeded: stateEntries', true);
            checkValue(state, limits, START);
            checkState(state, START);
            Object.assign(values, state);
        }
        else
            for (const decl of program.states)
                values[decl.name] = interpreter.expr(decl.value);
        const roots: UiNode[] = [];
        if (!initialOnly)
            for (const e of program.roots)
                roots.push(...interpreter.children(interpreter.expr(e), e.pos, `${e.pos.line}:${e.pos.column}`));
        let nodeCount = 0;
        const pending: Value[] = [roots];
        while (pending.length) {
            const v = pending.pop();
            if (v !== null && typeof v === 'object') {
                if (!Array.isArray(v) && v.kind === 'node' && ++nodeCount > limits.nodes)
                    throw new Failure(START, 'budget exceeded: nodes', true);
                pending.push(...Object.values(v));
            }
        }
        checkValue({ state: values, roots }, limits, START);
        return { state: values, roots, diagnostics: interpreter.diagnostics.items };
    }
    catch (error) {
        if (!(error instanceof Failure))
            throw error;
        interpreter.diagnostics.report(error.pos, error.message);
        return { state: {}, roots: [], diagnostics: interpreter.diagnostics.items };
    }
}
/**
 * Initialize local state without running roots. Costs O(initializer steps × bounded value bytes + local name lookups),
 * bounded by the supplied budgets; allocates state/cache only. No locks, I/O or effects.
 * A failed initializer discards state and returns positioned diagnostics.
 */
export function initialState(program: Program, overrides: Partial<Budgets> = {}): {
    /** Local state snapshot; names exclude the dollar prefix. */
    state: Record<string, Value>;
    /** Positioned errors collected in deterministic order. */
    diagnostics: Diagnostic[];
} {
    const result = run(program, undefined, overrides, true);
    return { state: result.state, diagnostics: result.diagnostics };
}
