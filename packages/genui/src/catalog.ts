/** Native component and action schema data shared verbatim with Rust; no renderer or host integrations.
 * @module
 */
import data from './catalog-data.json' with { type: 'json' };
/** Argument kinds understood by both evaluators. */
export type ParamKind = 'string' | 'text' | 'number' | 'boolean' | 'enum' | 'nodes' | 'list' | 'series' | 'binding' | 'actions' | 'function' | 'any';
/** The JSON kind a native input binding accepts; null means empty for every kind. */
export type StateKind = 'text' | 'number' | 'boolean' | 'scalar' | 'list';
/** One positional or named argument in a native component/action signature. */
export interface ParamSpec {
    /** Argument key in evaluated properties. */
    name: string;
    /** Validation/normalization rule. */
    kind: ParamKind;
    /** Omitted or null means absent when true. */
    optional?: boolean;
    /** Required arguments that explicitly accept null. */
    nullable?: boolean;
    /** Allowed strings for enum arguments. */
    values?: readonly string[];
    /** State kind required for binding arguments. */
    holds?: StateKind;
}
/** Pure-data component/action contract shared with the Rust evaluator. */
export interface ComponentSpec {
    /** Parameters in positional order. */
    params: readonly ParamSpec[];
    /** Native catalog grouping; absent for actions. */
    group?: string;
    /** Explanation of the component's data shape. */
    summary?: string;
    /** Mandatory nonblank accessible names/descriptions; checked again after evaluation. */
    accessibility?: {
        /** Accessible argument names that must be present and resolve to nonblank text. */
        required: readonly string[];
    };
}
/** Native component vocabulary, adapted from the credited upstream catalog. */
export const COMPONENTS: Readonly<Record<string, ComponentSpec>> = data.components as Record<string, ComponentSpec>;
/** Descriptions of actions; interpreting them never executes their effects. */
export const ACTIONS: Readonly<Record<string, ComponentSpec>> = data.actions as Record<string, ComponentSpec>;
/** Available pure functions and their signatures. */
export const FUNCTIONS: Readonly<Record<string, string>> = data.functions;
/** Permitted receiver methods, grouped by JSON receiver kind. */
export const METHODS: Readonly<MethodCatalog> = data.methods;
/** Built-in numerical constants. */
export const CONSTANTS: Readonly<Record<string, number>> = data.constants;
/** Prototype-related names forbidden in declarations, access and external state. */
export const FORBIDDEN_PROPERTIES: readonly string[] = data.forbiddenProperties;
/** The name of a component drawn by a native client. Unknown names are rejected at validation. */
export type ComponentName = string;
/** Allowed receiver methods, as pure data; platform or prototype methods are never inherited. */
export interface MethodCatalog {
    /** Whitelisted list operations; callbacks are interpreted within the current budget. */
    list: readonly string[];
    /** Whitelisted text operations; positions use normalized UTF-16 strings. */
    string: readonly string[];
    /** Whitelisted numeric formatting operations. */
    number: readonly string[];
}
