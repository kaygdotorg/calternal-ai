/**
 * Small JSON Schema builders whose TypeScript types carry the value type they
 * describe. Each builder returns plain JSON Schema, so the emitted manifest
 * stays explicit for install review; only the static type is extra.
 * @module
 */
import type { CatalogueDefinitions } from "../generated/author-types.js";

/** Any JSON Schema value an operation may declare. */
export type Schema = boolean | object;

type Simplify<T> = { [K in keyof T]: T[K] } & {};
type Required<S> = S extends RequiredKeyword<readonly (infer R)[]> ? R : never;
type Primitive<T> =
  string extends T ? unknown :
  T extends "string" ? string :
  T extends "integer" | "number" ? number :
  T extends "boolean" ? boolean :
  T extends "null" ? null :
  T extends "array" ? unknown[] :
  T extends "object" ? Record<string, unknown> :
  unknown;
type ObjectValue<S, P> = Simplify<
  { -readonly [K in keyof P as K extends Required<S> ? K : never]: SchemaType<P[K]> } &
  { -readonly [K in keyof P as K extends Required<S> ? never : K]?: SchemaType<P[K]> }>;

/** The `required` keyword. */
export interface RequiredKeyword<R> {
  /** Names of the required properties. */
  readonly required: R;
}

/** The `type` keyword. */
export interface TypeKeyword<T> {
  /** The JSON type or types. */
  readonly type: T;
}

/** An object schema's `type` and `properties` keywords. */
export interface PropertiesKeyword<P> {
  /** Always `"object"`. */
  readonly type: "object";
  /** Property schemas by name. */
  readonly properties: P;
}

/** An array schema's `type` and `items` keywords. */
export interface ItemsKeyword<I> {
  /** Always `"array"`. */
  readonly type: "array";
  /** The item schema. */
  readonly items: I;
}

/** Exactly one of several schemas, matched without a discriminator. */
export interface AnyOfSchema<V> {
  /** The alternatives. */
  readonly anyOf: V;
}

/**
 * The value type a JSON Schema literal describes. Resolves host catalogue
 * references to the generated catalogue types, `const`, `enum`, `oneOf` and
 * `anyOf` unions, typed arrays and objects with required and optional
 * properties. Anything it cannot see statically is `unknown`; the host still
 * validates the full schema at runtime.
 */
export type SchemaType<S> =
  S extends CatalogueRefSchema<infer Name> ? Name extends keyof CatalogueDefinitions ? CatalogueDefinitions[Name] : unknown :
  S extends ConstSchema<infer Value> ? Value :
  S extends EnumSchema<readonly (infer Value)[]> ? Value :
  S extends OneOfSchema<readonly (infer Variant)[]> ? SchemaType<Variant> :
  S extends AnyOfSchema<readonly (infer Variant)[]> ? SchemaType<Variant> :
  S extends PropertiesKeyword<infer P> ? ObjectValue<S, P> :
  S extends ItemsKeyword<infer Items> ? SchemaType<Items>[] :
  S extends TypeKeyword<readonly (infer T)[]> ? Primitive<T> :
  S extends TypeKeyword<infer T> ? Primitive<T> :
  unknown;

/** The catalogue definition names a schema may reference. */
export type CatalogueName = keyof CatalogueDefinitions & string;

/** A reference to one host catalogue definition. */
export interface CatalogueRefSchema<Name extends string> {
  /** The catalogue-relative reference the host resolves. */
  readonly $ref: `${string}host-catalogue-v2.json#/$defs/${Name}`;
}

/** A closed JSON Schema object. */
export interface ObjectSchema<P, R> {
  /** Always `"object"`. */
  readonly type: "object";
  /** Always false: unknown fields are refused. */
  readonly additionalProperties: false;
  /** Property schemas by name. */
  readonly properties: P;
  /** Names of the required properties. */
  readonly required: R;
}

/** A JSON Schema of one primitive type plus the given keywords. */
export type TypedSchema<T extends string, K> = TypeKeyword<T> & K;

/** A JSON Schema array of one item schema plus the given keywords. */
export type ArraySchema<I, K> = ItemsKeyword<I> & K;

/** Keywords `string` accepts. */
export interface StringKeywords {
  /** Fewest characters. */
  minLength?: number;
  /** Most characters. */
  maxLength?: number;
  /** ECMAScript regular expression the value must match. */
  pattern?: string;
  /** A JSON Schema format such as `uri` or `date-time`. */
  format?: string;
}

/** Keywords `integer` accepts. */
export interface IntegerKeywords {
  /** Smallest value. */
  minimum?: number;
  /** Largest value. */
  maximum?: number;
}

/** Keywords `array` accepts. */
export interface ArrayKeywords {
  /** Fewest items. */
  minItems?: number;
  /** Most items. */
  maxItems?: number;
  /** Whether items must differ. */
  uniqueItems?: boolean;
}

/** A closed list of literal values. */
export interface EnumSchema<V> {
  /** The allowed values. */
  readonly enum: V;
}

/** Exactly one literal value. */
export interface ConstSchema<V> {
  /** The only allowed value. */
  readonly const: V;
}

/** Exactly one of several schemas. */
export interface OneOfSchema<V> {
  /** The alternatives. */
  readonly oneOf: V;
}

/** Reference one host catalogue definition, for example `ref("ItemRef")`. */
export function ref<const Name extends CatalogueName>(name: Name): CatalogueRefSchema<Name> {
  return { $ref: `host-catalogue-v2.json#/$defs/${name}` };
}

/**
 * A closed object: `additionalProperties: false` with the listed required
 * keys. The property order of the emitted schema follows the argument.
 */
export function object<const P extends Readonly<Record<string, Schema>>, const R extends readonly (keyof P & string)[]>(
  properties: P, ...required: R
): ObjectSchema<P, R> {
  return { type: "object", additionalProperties: false, properties, required };
}

/** A string schema with optional length, pattern and format keywords. */
export function string<const K extends StringKeywords = {}>(
  keywords?: K,
): TypedSchema<"string", K> {
  return { type: "string", ...keywords } as TypedSchema<"string", K>;
}

/** An integer schema with optional bounds. */
export function integer<const K extends IntegerKeywords = {}>(keywords?: K): TypedSchema<"integer", K> {
  return { type: "integer", ...keywords } as TypedSchema<"integer", K>;
}

/** One of a closed list of literal values. */
export function oneOfValues<const V extends readonly (string | number | boolean)[]>(...values: V): EnumSchema<V> {
  return { enum: values };
}

/** Exactly one literal value. */
export function literal<const V extends string | number | boolean>(value: V): ConstSchema<V> {
  return { const: value };
}

/** An array of one item schema with optional bounds. */
export function array<const I extends Schema, const K extends ArrayKeywords = {}>(
  items: I, keywords?: K,
): ArraySchema<I, K> {
  return { type: "array", items, ...keywords } as ArraySchema<I, K>;
}

/** Exactly one of several schemas. */
export function oneOf<const V extends readonly Schema[]>(...variants: V): OneOfSchema<V> {
  return { oneOf: variants };
}
