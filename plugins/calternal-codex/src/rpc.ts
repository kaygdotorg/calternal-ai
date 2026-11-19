/** Bounded stdio transport for the pinned Codex App Server. */
import { spawn } from 'node:child_process';
import { setTimeout } from 'node:timers/promises';
import type { Readable } from 'node:stream';

/** One decoded message with provider-controlled contents. */
export type Frame = Record<string, unknown>;

function object(data: unknown): Frame {
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('invalid frame');
  return data as Frame;
}

/** Decode complete JSON lines without retaining an unbounded CLI line. */
export async function* jsonLines(input: Readable, maxBytes: number): AsyncGenerator<Frame> {
  let pending = Buffer.alloc(0);
  for await (const chunk of input) {
    const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    let start = 0;
    while (start < bytes.length) {
      const newline = bytes.indexOf(10, start);
      const end = newline < 0 ? bytes.length : newline;
      if (pending.length + end - start + 1 > maxBytes) throw new Error('line too long');
      const part = bytes.subarray(start, end);
      if (newline < 0) {
        pending = Buffer.concat([pending, part]);
        break;
      }
      const line = pending.length ? Buffer.concat([pending, part]) : part;
      pending = Buffer.alloc(0);
      start = newline + 1;
      yield object(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(line)));
    }
  }
  if (pending.length) throw new Error('incomplete frame');
}

interface Pending {
  resolve: (result: Frame) => void;
  reject: (error: Error) => void;
  timer: NodeJS.Timeout;
}

/** One official App Server child with bounded pending requests and private stderr. */
export class CodexRpc {
  private readonly child: ReturnType<typeof spawn>;
  private readonly pending = new Map<number, Pending>();
  private readonly exited: Promise<void>;
  private next = 1;
  private closed = false;

  /** Spawn the version-pinned executable with pipes in the selected sandbox. */
  constructor(binary: string, args: string[], cwd: string, env: NodeJS.ProcessEnv,
    private readonly receive: (frame: Frame) => Promise<void>) {
    this.child = spawn(binary, [...args, 'app-server', '--listen', 'stdio://'], {
      cwd, env, stdio: ['pipe', 'pipe', 'ignore'],
    });
    this.exited = new Promise(resolve => {
      this.child.once('close', () => resolve());
    });
    this.child.on('error', () => this.fail());
    this.child.stdin?.on('error', () => this.fail());
    void this.read();
  }

  private async read(): Promise<void> {
    try {
      if (!this.child.stdout) throw new Error('missing stdout');
      for await (const frame of jsonLines(this.child.stdout, 1024 * 1024)) {
        if (typeof frame.method === 'string') {
          await this.receive(frame);
          continue;
        }
        const item = typeof frame.id === 'number' ? this.pending.get(frame.id) : undefined;
        if (!item || typeof frame.id !== 'number') throw new Error('unmatched response');
        this.pending.delete(frame.id);
        clearTimeout(item.timer);
        if (frame.error !== undefined) item.reject(new Error('Codex request failed'));
        else item.resolve(frame.result === null ? {} : object(frame.result));
      }
    } catch {
      this.child.kill('SIGKILL');
    } finally {
      this.fail();
    }
  }

  private fail(): void {
    this.closed = true;
    for (const item of this.pending.values()) {
      clearTimeout(item.timer);
      item.reject(new Error('Codex process ended'));
    }
    this.pending.clear();
  }

  private write(frame: Frame): void {
    const line = JSON.stringify(frame) + '\n';
    if (this.closed || !this.child.stdin || this.child.stdin.destroyed
      || this.child.stdin.writableLength + Buffer.byteLength(line) > 256 * 1024) {
      throw new Error('Codex input is unavailable');
    }
    this.child.stdin.write(line);
  }

  /** Perform the documented initialize handshake before using any endpoint. */
  async initialize(): Promise<void> {
    await this.request('initialize', {
      clientInfo: { name: 'calternal', title: 'Calternal', version: '0.1.0' },
      capabilities: { experimentalApi: true },
    });
    this.write({ method: 'initialized' });
  }

  /** Send a capped JSON-RPC request and wait for its matching result. */
  request(method: string, params: Frame, timeoutMs = 10_000): Promise<Frame> {
    if (this.pending.size >= 16) return Promise.reject(new Error('Codex request budget exceeded'));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeoutCallback(() => {
        this.pending.delete(id);
        reject(new Error('Codex request timed out'));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      try {
        this.write({ id, method, params });
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
  }

  /** Resolve a native server request without writing a second user message. */
  reply(id: string | number, result: Frame): void {
    this.write({ id, result });
  }

  /** Resolve when the official subprocess exits, including a transport failure. */
  whenClosed(): Promise<void> { return this.exited; }

  /** Shut down the owned subprocess, bounding the wait if it ignores EOF. */
  async close(): Promise<void> {
    this.child.stdin?.end();
    await Promise.race([this.exited, setTimeout(400, null, { ref: false })]);
    if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill('SIGKILL');
    await this.exited;
  }
}

const setTimeoutCallback = globalThis.setTimeout;
