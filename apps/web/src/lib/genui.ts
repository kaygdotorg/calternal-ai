import type { GenUiNode, GenUiView } from './types';

export const LIMITS = { maxNodes: 128, maxDepth: 8, maxText: 8 * 1024, maxTotalBytes: 32 * 1024 };
const ID_RE = /^[a-zA-Z0-9_.:-]{1,80}$/;
export const KNOWN = ['stack', 'text', 'button', 'stat', 'resource', 'document-editor'] as const;

const enc = new TextEncoder();

/**
 * Client-side defensive validation of a GenUI view. The server is authoritative;
 * this only prevents the renderer from consuming a malformed/hostile tree.
 * Unknown node types are NOT errors: they render an explicit unsupported fallback.
 */
export function validateView(view: unknown): string[] {
  const errors: string[] = [];
  const v = view as GenUiView | null;
  if (!v || typeof v !== 'object') return ['view is not an object'];
  if (v.genui_version !== 1) errors.push(`unsupported genui_version ${String((v as any).genui_version)}`);
  const seen = new Set<string>();
  let nodes = 0, bytes = 0;
  const walk = (n: any, depth: number) => {
    if (errors.length > 5) return;
    if (!n || typeof n !== 'object' || typeof n.type !== 'string') { errors.push('node is not an object with a type'); return; }
    nodes++;
    if (nodes > LIMITS.maxNodes) { errors.push(`more than ${LIMITS.maxNodes} nodes`); return; }
    if (depth > LIMITS.maxDepth) { errors.push(`deeper than ${LIMITS.maxDepth} levels`); return; }
    if (typeof n.id !== 'string' || !ID_RE.test(n.id)) { errors.push('node has an invalid id'); return; }
    if (seen.has(n.id)) { errors.push(`duplicate id "${n.id}"`); return; }
    seen.add(n.id);
    if (n.type === 'button') {
      if (typeof n.label !== 'string' || !n.action || typeof n.action.operation !== 'string' || !n.action.input || typeof n.action.input !== 'object' || Array.isArray(n.action.input)) {
        errors.push(`button "${n.id}" is malformed`); return;
      }
    }
    if ((n.type === 'button' || n.type === 'resource' || n.type === 'stat') && typeof n.label === 'string' && [...n.label].length > 256) { errors.push(`label in "${n.id}" is too long`); return; }
    if (n.type === 'resource' || n.type === 'document-editor') {
      if (typeof n.resource_id !== 'string' || !ID_RE.test(n.resource_id) || typeof n.space_id !== 'string' || !ID_RE.test(n.space_id)) { errors.push(`reference "${n.id}" is malformed`); return; }
    }
    for (const k of ['text', 'label', 'value']) {
      if (typeof n[k] === 'string') {
        const b = enc.encode(n[k]).length;
        bytes += b;
        if (b > LIMITS.maxText) errors.push(`text in "${n.id}" is too long`);
      }
    }
    if (bytes > LIMITS.maxTotalBytes) errors.push('combined text is too large');
    if (n.type === 'stack') {
      if (!Array.isArray(n.children)) { errors.push(`stack "${n.id}" has no children array`); return; }
      for (const c of n.children) walk(c, depth + 1);
    }
  };
  walk(v.root, 1);
  return errors;
}

export function isKnownNode(n: GenUiNode | { type: string }): n is GenUiNode {
  return (KNOWN as readonly string[]).includes(n.type);
}

/** Truthy only for operation ids that a plugin declared (read-only). */
export interface ActionPolicy {
  /** operations the owning plugin declared as read-only; null = no actions allowed */
  allowed: ReadonlySet<string> | null;
  /** disable all actions (e.g. preview mode) with this reason */
  disabledReason?: string;
}

export function checkAction(policy: ActionPolicy, operation: string): string | null {
  if (policy.disabledReason) return policy.disabledReason;
  if (!policy.allowed || !policy.allowed.has(operation)) return 'Blocked: this plugin may only run its own declared read-only operations.';
  return null;
}
