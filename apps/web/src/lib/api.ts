import type { ApiErrorBody, Describe } from './types';

export class ApiError extends Error {
  constructor(public code: string, message: string, public status: number, public details?: Record<string, unknown>) {
    super(message);
  }
  get isConflict() { return this.code === 'conflict'; }
}

/** The bearer token lives only in this closure variable: never in URL, storage or cookies. */
let token: string | null = null;
let generation = 0; // bumped on every token change; responses to an older token are discarded
export function setToken(t: string | null) { generation++; token = t && t.trim() ? t.trim() : null; }
export function hasToken() { return token !== null; }

export function newRequestId(): string {
  return crypto.randomUUID();
}

async function parse(res: Response): Promise<any> {
  let body: any = null;
  try { body = await res.json(); } catch { /* non-JSON */ }
  return body;
}

function toError(res: Response, body: any): ApiError {
  const e: ApiErrorBody | undefined = body?.error;
  if (e && typeof e.code === 'string') return new ApiError(e.code, e.message ?? e.code, res.status, e.details);
  return new ApiError(res.status === 401 ? 'unauthorized' : 'internal', `Unexpected response (HTTP ${res.status})`, res.status);
}

async function send(url: string, init: RequestInit): Promise<{ res: Response; body: any }> {
  if (!token) throw new ApiError('unauthorized', 'Enter a development token to connect.', 401);
  const gen = generation;
  try {
    const res = await fetch(url, {
      ...init,
      credentials: 'omit',
      cache: 'no-store',
      headers: { ...(init.headers as Record<string, string>), authorization: `Bearer ${token}` }
    });
    const body = await parse(res);
    if (gen !== generation) throw new ApiError('unavailable', 'Session changed; response discarded.', 0);
    return { res, body };
  } catch (e) {
    if (e instanceof ApiError) throw e;
    throw new ApiError('unavailable', 'Cannot reach the preview server.', 0);
  }
}

export async function describe(): Promise<Describe> {
  const { res, body } = await send('/api/v1/describe', { method: 'GET' });
  if (!res.ok || body?.ok === false) throw toError(res, body);
  return (body?.data ?? body?.result ?? body) as Describe;
}

/**
 * Invoke an operation. Pass a stable `requestId` when retrying a mutation so the server
 * can deduplicate it (same id + same input => same result).
 */
export async function invoke<T>(operation: string, input: Record<string, unknown>, requestId: string = newRequestId()): Promise<T> {
  const { res, body } = await send('/api/v1/operations', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ request_id: requestId, operation, input })
  });
  if (!res.ok || body?.ok !== true) throw toError(res, body);
  return body.data as T;
}

export function describeError(e: unknown): string {
  if (e instanceof ApiError) {
    switch (e.code) {
      case 'unauthorized': return 'Not authorized. Check the development token.';
      case 'forbidden': return 'Your principal may not do that in this space.';
      case 'not_found': return 'Not found, or not visible to you.';
      case 'conflict': return 'Someone saved a newer revision first.';
      case 'too_large': return 'That is larger than the preview limit (document 1 MiB).';
      case 'unavailable': return e.message || 'The server is unavailable.';
      default: return e.message;
    }
  }
  return e instanceof Error ? e.message : 'Unexpected error';
}
