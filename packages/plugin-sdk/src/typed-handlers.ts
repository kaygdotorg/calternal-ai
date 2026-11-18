/** Infer the common JSON-schema object input shape from a literal manifest. */
import type { WorkerContextV2, WorkerHandlerV2 } from "./worker-v2.js";

type JsonSchema = {
  readonly type?: string;
  readonly properties?: Readonly<Record<string, JsonSchema>>;
  readonly required?: readonly string[];
  readonly items?: JsonSchema;
};

type Input<S> =
  S extends { readonly type: "string" } ? string :
  S extends { readonly type: "boolean" } ? boolean :
  S extends { readonly type: "integer" | "number" } ? number :
  S extends { readonly type: "array"; readonly items: infer I } ? Input<I>[] :
  S extends { readonly type: "object"; readonly properties: infer P; readonly required: readonly (infer R)[] }
    ? { [K in keyof P as K extends R ? K : never]: Input<P[K]> } &
      { [K in keyof P as K extends R ? never : K]?: Input<P[K]> }
    : unknown;

type Operation = {
  readonly id: string;
  readonly input_schema: JsonSchema | boolean;
  readonly output_schema: JsonSchema | boolean;
};
type Handler<Op extends Operation> = (input: Input<Op["input_schema"]>, context: WorkerContextV2) => Promise<Input<Op["output_schema"]>>;

/** Bind each declared operation to one handler. Literal manifest declarations infer
 * basic input and output properties and require matching handler keys. Runtime validation
 * still comes from the host's compiled schemas. Cost: return the map unchanged.
 */
export function typedHandlers<const O extends readonly Operation[]>(
  manifest: { readonly operations: O },
  handlers: NoInfer<{ [Id in O[number]["id"]]: Handler<Extract<O[number], { readonly id: Id }>> }>,
): Record<string, WorkerHandlerV2> {
  // The host validates against the declared operation before sending a frame.
  void manifest;
  return handlers as Record<string, WorkerHandlerV2>;
}
