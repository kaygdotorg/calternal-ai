/** Claude Code adapter: the official CLI owns authentication and session storage. */
import { spawn, type ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import type { ProviderCommand, ProviderEvent, ProviderEventKind, ProviderRequest, ProviderFailureClass, ProviderBilling } from '@calternal/plugin-sdk';

const COMMAND_BYTES = 64 * 1024;
const EVENT_BYTES = 12 * 1024;
const CLI_LINE_BYTES = 1024 * 1024;
const signInCommand = 'claude auth login';
const sessionId = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const bytes = (s: string) => Buffer.byteLength(s, 'utf8');
const clip = (s: unknown, max: number): string => {
  if (typeof s !== 'string') return '';
  let result = ''; let used=0;
  for (const c of s) { const n=bytes(c); if (used+n>max) break; result += c; used+=n; }
  return result;
};

/** Split a byte stream into bounded UTF-8 JSON lines, rejecting partial trailing input. */
export async function* lines(input: AsyncIterable<Uint8Array>, max: number): AsyncGenerator<string> {
  let held = Buffer.alloc(0);
  for await (const chunk of input) {
    held = Buffer.concat([held, chunk]);
    let end: number;
    while ((end = held.indexOf(10)) >= 0) {
      if (end + 1 > max) throw new Error('line limit');
      yield new TextDecoder('utf-8', { fatal: true }).decode(held.subarray(0, end));
      held = held.subarray(end + 1);
    }
    if (held.length >= max) throw new Error('line limit');
  }
  if (held.length) throw new Error('partial line');
}

/** One session; emits sequentially and waits for the consumer before reading more CLI output. */
export class Adapter {
  private sequence = 0;
  private commandSequence = 0;
  private config?: Extract<ProviderCommand, { type: 'start' }>;
  private child?: ChildProcess;
  private active?: Promise<void>;
  private cancelled = false;
  private session?: string;
  private emitTail: Promise<void> = Promise.resolve();
  /** Select the installed official executable and an ordered, back-pressured event sink. */
  constructor(private readonly binary: string, private readonly write: (line: string) => Promise<void>) {}

  private emit(event: ProviderEventKind): Promise<void> {
    const next = async () => {
      const value: ProviderEvent = { version: 1, seq: ++this.sequence, event };
      const line = JSON.stringify(value) + '\n';
      if (bytes(line) > EVENT_BYTES) throw new Error('event limit');
      await this.write(line);
    };
    this.emitTail = this.emitTail.then(next);
    return this.emitTail;
  }

  /** Reject unsupported versions, command gaps, oversized input and unknown command keys. */
  async command(line: string): Promise<void> {
    if (bytes(line) + 1 > COMMAND_BYTES) throw new Error('command limit');
    const request = JSON.parse(line) as ProviderRequest;
    if (Object.keys(request).sort().join() !== 'command,seq,version' || request.version !== 1 ||
        !Number.isSafeInteger(request.seq) || request.seq !== this.commandSequence + 1 || !request.command) throw new Error('protocol');
    const c = request.command;
    const keys: Record<string, string[]> = {check: ['type'], start: ['type','model','options','cwd','instructions','resume'],
      send: ['type','prompt','context'], steer: ['type','prompt'], stop: ['type']};
    if (!keys[c.type] || Object.keys(c).some(k => !keys[c.type].includes(k))) throw new Error('command');
    this.commandSequence = request.seq;
    switch (c.type) {
      case 'check': await this.check(); break;
      case 'start':
        if (this.active || typeof c.model !== 'string' || c.model.length > 128 || c.model.startsWith('-') || /[\u0000-\u001f]/.test(c.model) || typeof c.cwd !== 'string' ||
            typeof c.instructions !== 'string' || (c.resume !== null && (typeof c.resume !== 'string' || !sessionId.test(c.resume)))) throw new Error('start');
        if (!c.options || typeof c.options !== 'object' || Array.isArray(c.options) || Object.keys(c.options).some(k => !['max_turns','effort'].includes(k))) throw new Error('options');
        if (c.options.max_turns !== undefined && (!Number.isInteger(c.options.max_turns) || Number(c.options.max_turns)<1 || Number(c.options.max_turns)>1000)) throw new Error('max turns');
        if (c.options.effort !== undefined && !['low','medium','high'].includes(String(c.options.effort))) throw new Error('effort');
        this.config = c; this.session = c.resume ?? undefined; break;
      case 'send':
        if (!this.config || this.active || typeof c.prompt !== 'string' || !Array.isArray(c.context) || c.context.length > 32 ||
            c.context.some(b => typeof b.source !== 'string' || bytes(b.source) > 128 || typeof b.text !== 'string' || Object.keys(b).sort().join() !== 'source,text')) throw new Error('send');
        this.launch(c.prompt + c.context.map(b => `\n\n<context source=${JSON.stringify(b.source)}>\n${b.text}\n</context>`).join('')); break;
      case 'steer':
        if (!this.config || typeof c.prompt !== 'string') throw new Error('steer');
        await this.stop(); this.launch(c.prompt); break;
      case 'stop': await this.stop(); break;
    }
  }

  private environment(): NodeJS.ProcessEnv {
    // Do not inherit API keys, OAuth environment overrides, or the machine role's login.
    const env: NodeJS.ProcessEnv = {};
    for (const k of ['PATH','HOME','TMPDIR','LANG','CALTERNAL_URL','CALTERNAL_AGENT_TOKEN']) if (process.env[k]) env[k] = process.env[k];
    return env;
  }

  private async check(): Promise<void> {
    let installed = true;
    const child = spawn(this.binary, ['auth', 'status'], { env: this.environment(), stdio: 'ignore' });
    const timer = setTimeout(() => child.kill('SIGKILL'), 15_000);
    const status = await new Promise<number | null>(resolve => {
      child.once('error', () => { installed = false; resolve(null); });
      child.once('close', code => resolve(code));
    });
    clearTimeout(timer);
    await this.emit({ type: 'check', installed, signed_in: installed && status === 0, sign_in_command: signInCommand });
  }

  private launch(prompt: string): void {
    this.cancelled = false;
    this.active = this.run(prompt).finally(() => { this.active = undefined; this.child = undefined; });
  }

  private async run(prompt: string): Promise<void> {
    let outcome = 'outcome_unknown';
    let result = false;
    let billing: ProviderBilling = 'unknown';
    let resetAt: number | undefined;
    try {
      // Read only the official status response, not a login file or token.
      const status = spawn(this.binary, ['auth','status'], { env: this.environment(), stdio: ['ignore','pipe','ignore'] });
      this.child = status;
      let installed = true;
      const statusDone = new Promise<number | null>(resolve => {
        status.once('error', () => { installed = false; resolve(null); });
        status.once('close', resolve);
      });
      const statusTimer = setTimeout(() => status.kill('SIGKILL'), 15_000);
      let statusText = '';
      for await (const chunk of status.stdout) { statusText += chunk.toString('utf8'); if (bytes(statusText) > 32*1024) {status.kill('SIGKILL'); statusText=''; break;} }
      const statusCode = await statusDone;
      clearTimeout(statusTimer);
      await this.emit({type:'check',installed,signed_in:installed && statusCode === 0,sign_in_command:signInCommand});
      try {const method = JSON.parse(statusText).authMethod; billing = method === 'claude.ai' ? 'subscription' : ['api_key','api_key_helper','third_party'].includes(method) ? 'api' : 'unknown';} catch {}
      statusText = '';
      if (this.cancelled) {await this.emit({type:'done',outcome:'cancelled'});return;}
      const c = this.config!;
      const args = ['-p', '--output-format', 'stream-json', '--verbose', '--include-partial-messages',
        '--model', c.model, '--append-system-prompt', c.instructions, '--system-prompt-snapshot', 'off',
        '--disallowedTools', 'AskUserQuestion,Agent,Task',
        '--allowedTools', 'Read,Edit,Write,Bash,Glob,Grep,WebFetch,WebSearch,NotebookEdit',
        '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}'];
      if (c.options.max_turns !== undefined) args.push('--max-turns', String(c.options.max_turns));
      if (c.options.effort !== undefined) args.push('--effort', String(c.options.effort));
      if (this.session) args.push('--resume', this.session);
      const child = spawn(this.binary, args, { cwd: c.cwd, env: this.environment(), stdio: ['pipe','pipe','ignore'] });
      this.child = child;
      const closed = new Promise<number | null>(resolve => {
        child.once('error', () => resolve(null)); child.once('close', code => resolve(code));
      });
      child.stdin.on('error', () => {});
      child.stdin.end(prompt);
      for await (const line of lines(child.stdout, CLI_LINE_BYTES)) {
        const m = JSON.parse(line);
        if (m.type === 'system' && m.subtype === 'init' && typeof m.session_id === 'string') {
          if (!sessionId.test(m.session_id)) throw new Error('session id');
          const id: string = m.session_id; this.session = id;
          await this.emit({ type: 'session', session_id: id });
        }
        if (m.type === 'stream_event' && m.event?.type === 'content_block_delta' && m.event.delta?.type === 'text_delta') {
          const text = m.event.delta.text;
          if (typeof text !== 'string') throw new Error('text');
          let part = ''; let used=0;
          for (const ch of text) {
            const n=bytes(ch);
            if (used+n>1024) { await this.emit({ type: 'text_delta', text: part }); part = ''; used=0; }
            part += ch; used+=n;
          }
          if (part) await this.emit({ type: 'text_delta', text: part });
        }
        if (m.type === 'assistant' && Array.isArray(m.message?.content)) {
          for (const block of m.message.content) if (block.type === 'tool_use') {
            if (block.name === 'AskUserQuestion') await this.emit({ type: 'question', text: 'Claude Code needs your answer. Use Calternal to reply.' });
            await this.emit({ type: 'tool_started', id: clip(block.id,128), name: clip(block.name,128), summary: ({Read:'Read a workspace file',Bash:'Run a workspace command',Edit:'Edit a workspace file',Write:'Write a workspace file',Grep:'Search workspace text',Glob:'Find workspace files',WebSearch:'Search the web',WebFetch:'Read a web page'} as Record<string,string>)[String(block.name)] ?? 'Run a provider tool' });
          }
        }
        if (m.type === 'user' && Array.isArray(m.message?.content)) {
          for (const block of m.message.content) if (block.type === 'tool_result') {
            // Tool output can contain credentials. Keep completion status, not the raw output.
            await this.emit({ type: 'tool_finished', id: clip(block.tool_use_id,128), failed: block.is_error === true, detail: '' });
          }
        }
        if (m.type === 'rate_limit_event' && m.rate_limit_info?.status === 'rejected') {
          const reset=m.rate_limit_info.resetsAt;
          if (typeof reset==='number' && Number.isFinite(reset) && reset>0 && Number.isSafeInteger(Math.ceil(reset*1000))) resetAt=Math.ceil(reset*1000);
          await this.emit({type:'error',code:'rate_limit',class:'usage_limit',billing,...(resetAt ? {reset_at_ms:resetAt}:{}),message:'The provider reported a usage limit.'});
        }
        if (m.type === 'assistant' && typeof m.error === 'string') {
          const code=clip(m.error,128);
          const category: ProviderFailureClass = ['server_error','overloaded'].includes(code) ? 'transient' : code==='rate_limit' ? 'usage_limit' :
            ['authentication_failed','oauth_org_not_allowed','cloud_credential_error'].includes(code) ? 'authentication' :
            ['invalid_request','model_not_found','billing_error','account_on_hold','max_output_tokens'].includes(code) ? 'permanent' : 'unknown';
          await this.emit({type:'error',code,class:category,billing,...(resetAt ? {reset_at_ms:resetAt}:{}),message:category==='authentication'?'Sign in inside your sandbox with claude auth login.':'Claude Code reported a provider error.'});
        }
        if (m.type === 'system' && m.subtype === 'model_refusal_no_fallback') await this.emit({type:'error',code:'refusal',class:'refusal',billing,message:'The provider declined this request.'});
        if (m.type === 'result') {
          if (result) throw new Error('duplicate result');
          result = true; outcome = m.is_error === false && m.subtype === 'success' ? 'done' : 'failed';
          const safeCount = (v: unknown) => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0 ? v : 0;
          await this.emit({ type: 'usage', input_tokens: safeCount(m.usage?.input_tokens), output_tokens: safeCount(m.usage?.output_tokens) });
          if (Array.isArray(m.permission_denials)) for (const denial of m.permission_denials.slice(0,32)) {
            await this.emit({ type: 'approval_request', summary: `Permission required for ${clip(denial.tool_name,128)}` });
          }
        }
      }
      const code = await closed;
      if (!result || (code !== 0 && outcome === 'done')) {
        outcome = 'outcome_unknown';
        if (!this.cancelled) await this.emit({ type: 'error', code: 'cli_exited', class:'unknown', billing, message: 'Claude Code ended before a complete result. Check sign-in with calternal agent check; resume explicitly.' });
      }
    } catch {
      this.child?.kill('SIGKILL');
      if (!this.cancelled) await this.emit({ type: 'error', code: 'provider_stream', class:'unknown', billing, message: 'Claude Code could not start or returned an invalid stream. Check the installation and sign-in inside your sandbox.' });
    }
    await this.emit({ type: 'done', outcome: this.cancelled ? 'cancelled' : outcome });
  }

  /** End active CLI work, escalating after two seconds; waits for the final ordered event. */
  async stop(): Promise<void> {
    this.cancelled = true;
    const child = this.child;
    child?.kill('SIGINT');
    const timer = setTimeout(() => child?.kill('SIGKILL'), 2000);
    await this.active; clearTimeout(timer);
  }

  /** EOF waits for an in-flight turn; no additional work starts after EOF. */
  async finish(): Promise<void> { await this.active; await this.emitTail; }
}

/** Run one adapter over stdio; stderr contains no provider output or input. */
export async function main(): Promise<void> {
  const adapter = new Adapter(process.env.CALTERNAL_CLAUDE_BINARY ?? 'claude', async line => {
    if (!process.stdout.write(line)) await once(process.stdout, 'drain');
  });
  process.once('SIGTERM', () => { void adapter.stop(); });
  process.once('SIGINT', () => { void adapter.stop(); });
  try {
    for await (const line of lines(process.stdin, COMMAND_BYTES)) await adapter.command(line);
    await adapter.finish();
  } catch { await adapter.stop(); process.exitCode = 1; }
}
