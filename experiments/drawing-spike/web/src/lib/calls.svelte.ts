// The spike's stand-in for the host's call table: every action ends here as `call(operation, inputs)`. Nothing is sent;
// the last calls are shown in the shell's foot so a capture proves which operation and inputs a gesture produced.
import { opOf } from "./model";
export const calls = $state<{ log: { op: string; inputs: Record<string, unknown>; title: string }[] }>({ log: [] });
/** Records a call as the host would receive it: an operation name and its inputs. */
export function call(op: string, inputs: Record<string, unknown>): void {
  calls.log = [{ op, inputs, title: opOf(op)?.title ?? op }, ...calls.log].slice(0, 3);
}
