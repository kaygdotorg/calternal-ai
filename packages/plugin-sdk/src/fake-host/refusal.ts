/** Refusals the fake host returns, shaped like the host's caller-safe errors. */

/**
 * A broker or admission refusal: HTTP-like `status`, stable `code`, a plain
 * message and, for a missing declaration, the manifest path and the change
 * that fixes it.
 */
export class HostRefusal extends Error {
  /** HTTP-like status, such as 403 or 409. */
  readonly status: number;
  /** Stable machine-readable code, such as `not_declared`. */
  readonly code: string;
  /** For a missing declaration, the manifest path to change. */
  readonly path?: string;
  /** For a missing declaration, the declaration to add. */
  readonly change?: string;
  /** Create a refusal; `message` says what happened and what to do next. */
  constructor(status: number, code: string, message: string, path?: string, change?: string) {
    super(message);
    this.name = "HostRefusal";
    this.status = status;
    this.code = code;
    if (path !== undefined) this.path = path;
    if (change !== undefined) this.change = change;
  }
}

/** `403 not_declared` naming the exact manifest path and the declaration to add. */
export function notDeclared(what: string, path: string, change: string): HostRefusal {
  return new HostRefusal(403, "not_declared", `${what} is not declared at ${path}. ${change}`, path, change);
}

/** Throw `refusal` unless `condition` holds. */
export function ensure(condition: unknown, status: number, code: string, message: string): asserts condition {
  if (!condition) throw new HostRefusal(status, code, message);
}
