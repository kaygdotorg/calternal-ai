/** Translate a bounded Calternal turn to a persistent official Codex session. */
import { randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { CodexRpc, jsonLines, type Frame } from './rpc.js';
import { prepareProfile, version, type AgentMode, type Profile } from './profile.js';

const signInCommand = 'codex login --device-auth';
const toolNames: Record<string, string> = { commandExecution: 'command', fileChange: 'file', mcpToolCall: 'MCP', webSearch: 'web search' };

function object(value: unknown): Frame {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Frame : {};
}

function short(value: unknown, max: number): string {
  if (typeof value !== 'string') return '';
  let result = '';
  let bytes = 0;
  for (const character of value) {
    bytes += Buffer.byteLength(character);
    if (bytes > max) break;
    result += character;
  }
  return result;
}

function safeCount(n: unknown): number {
  return typeof n === 'number' && Number.isSafeInteger(n) && n >= 0 ? n : 0;
}

function mode(): AgentMode {
  const selected = process.env.CALTERNAL_AGENT_MODE;
  if (selected === 'least_access' || selected === 'approve_for_me' || selected === 'full_access') return selected;
  throw new Error('missing host permission mode');
}

class Adapter {
  private seq = 0;
  private commandSeq = 0;
  private start: Frame | undefined;
  private profile: Profile | undefined;
  private rpc: CodexRpc | undefined;
  private threadId: string | undefined;
  private turnId: string | undefined;
  private waiting = false;
  private terminal = false;
  private startedText = new Set<string>();
  private tools = new Map<string, { name: string; summary: string }>();
  private approvals = new Map<string, { id: string | number; timer: NodeJS.Timeout }>();
  private reasoningStart: number | undefined;
  private resetAt: number | undefined;
  private finish!: () => void;
  private readonly settled = new Promise<void>(resolve => { this.finish = resolve; });
  private writes = Promise.resolve();

  private emit(event: Frame): Promise<void> {
    const line = JSON.stringify({version: 1, seq: ++this.seq, event}) + '\n';
    if (Buffer.byteLength(line) > 12 * 1024) throw new Error('event too large');
    this.writes = this.writes.then(async () => {
      if (!process.stdout.write(line)) await once(process.stdout, 'drain');
    });
    return this.writes;
  }

  private async emitText(type: 'text_delta' | 'reasoning_delta', text: string) {
    while (text) {
      const chunk = short(text, 4096);
      await this.emit({type, text: chunk});
      text = text.slice(chunk.length);
    }
  }

  private async connect(): Promise<CodexRpc> {
    if (this.rpc) return this.rpc;
    const profile = await prepareProfile(process.cwd(), this.start ? mode() : 'least_access', this.start !== undefined);
    if (await version(profile.binary) !== process.env.CALTERNAL_HARNESS_VERSION) {
      throw new Error('installed Codex version does not match the active pin');
    }
    this.profile = profile;
    this.rpc = new CodexRpc(profile.binary, profile.args, process.cwd(), profile.env, frame => this.event(frame));
    await this.rpc.initialize();
    void this.rpc.whenClosed().then(() => {
      if (!this.terminal) void this.error('codex_disconnected', 'unknown', 'The official Codex process ended.');
    });
    return this.rpc;
  }

  private async check() {
    let rpc: CodexRpc;
    try {
      rpc = await this.connect();
    } catch {
      await this.emit({type: 'check', installed: false, signed_in: false, sign_in_command: signInCommand});
      return 'missing' as const;
    }
    const result = await rpc.request('account/read', {refreshToken: false});
    const signed = object(result.account).type === 'chatgpt';
    await this.emit({type: 'check', installed: true, signed_in: signed, sign_in_command: signInCommand});
    return signed ? 'ready' as const : 'signed_out' as const;
  }

  private async send(command: Frame) {
    const start = this.start;
    if (this.waiting || !start || typeof command.prompt !== 'string' || !Array.isArray(command.context)) {
      throw new Error('unexpected send');
    }
    this.waiting = true;
    const status = await this.check();
    if (status !== 'ready') {
      if (status === 'missing') {
        await this.error('provider_version_unavailable', 'permanent', 'The pinned Codex CLI or its sandbox is unavailable.');
      } else {
        await this.error('provider_signed_out', 'authentication', 'Sign in to Codex with ChatGPT.');
      }
      return;
    }
    const rpc = this.rpc!;
    const profile = this.profile!;
    const options = object(start.options);
    if (typeof start.model !== 'string' || !start.model || typeof start.instructions !== 'string'
      || Object.keys(options).some(key => key !== 'reasoning')
      || (options.reasoning !== undefined && typeof options.reasoning !== 'string')
      || (start.resume !== null && start.resume !== undefined && typeof start.resume !== 'string')) {
      throw new Error('invalid turn configuration');
    }
    const method = start.resume ? 'thread/resume' : 'thread/start';
    const result = await rpc.request(method, {
      ...(start.resume ? {threadId: start.resume} : {}),
      cwd: process.cwd(), model: start.model, developerInstructions: start.instructions,
      permissions: profile.permissions, approvalPolicy: profile.approvalPolicy, approvalsReviewer: 'user',
    });
    const id = object(result.thread).id;
    if (typeof id !== 'string' || !/^[0-9a-f-]{36}$/i.test(id)) throw new Error('invalid Codex session');
    this.threadId = id;
    await this.emit({type: 'session', session_id: id});
    const inputs = command.context.map(block => {
      const entry = object(block);
      if (typeof entry.source !== 'string' || typeof entry.text !== 'string') throw new Error('invalid context');
      return {type: 'text', text: `Context from ${entry.source}:\n${entry.text}`};
    });
    const started = await rpc.request('turn/start', {
      threadId: id, input: [{type: 'text', text: command.prompt}, ...inputs],
      model: start.model, effort: options.reasoning ?? null, permissions: profile.permissions,
      approvalPolicy: profile.approvalPolicy,
    });
    this.turnId = short(object(started.turn).id, 128) || this.turnId;
  }

  private async stop() {
    if (this.threadId && this.turnId && this.rpc) {
      await this.rpc.request('turn/interrupt', {threadId: this.threadId, turnId: this.turnId}, 1500);
    } else {
      await this.done('cancelled');
    }
  }

  private async steer(command: Frame) {
    if (!this.threadId || !this.turnId || typeof command.prompt !== 'string') return;
    await this.rpc!.request('turn/steer', {
      threadId: this.threadId, expectedTurnId: this.turnId,
      input: [{type: 'text', text: command.prompt}],
    });
  }

  private async approval(frame: Frame, item: Frame) {
    const rpc = this.rpc!;
    const id = frame.id;
    if (typeof id !== 'number' && typeof id !== 'string') throw new Error('invalid approval ID');
    // Native escalations can bypass Codex's protected read roots. Their accept
    // decision is unavailable; the person can adjust a workspace action instead.
    const method = frame.method;
    if (mode() !== 'approve_for_me' || item.threadId !== this.threadId
      || (method !== 'item/commandExecution/requestApproval' && method !== 'item/fileChange/requestApproval')
      || this.approvals.size >= 16) {
      if (method === 'item/permissions/requestApproval') rpc.reply(id, {permissions: {}, scope: 'turn'});
      else if (method === 'item/tool/requestUserInput') rpc.reply(id, {answers: {}});
      else rpc.reply(id, {decision: 'decline'});
      return;
    }
    const key = randomUUID();
    const timer = setTimeout(() => { this.resolveApproval(key); }, 85_000);
    this.approvals.set(key, {id, timer});
    await this.emit({
      type: 'approval_request', request_id: key,
      summary: 'Codex requested permission outside its tool sandbox. Calternal will keep the existing boundary.',
    });
  }

  private resolveApproval(key: string) {
    const pending = this.approvals.get(key);
    if (!pending) return;
    clearTimeout(pending.timer);
    this.approvals.delete(key);
    try { this.rpc?.reply(pending.id, {decision: 'decline'}); } catch { /* the turn ended */ }
  }

  private async event(frame: Frame) {
    if (this.terminal) return;
    const params = object(frame.params);
    if (frame.id !== undefined) {
      await this.approval(frame, params);
      return;
    }
    if (frame.method === 'account/rateLimits/updated') {
      const limits = object(params.rateLimits);
      const reset = [object(limits.primary).resetsAt, object(limits.secondary).resetsAt]
        .map(safeCount).filter(Boolean);
      if (reset.length) this.resetAt = Math.max(...reset) * 1000;
      return;
    }
    if (!this.threadId || params.threadId !== this.threadId) return;
    if (frame.method === 'turn/started') {
      this.turnId = short(object(params.turn).id, 128);
      return;
    }
    const itemId = short(params.itemId, 128);
    if (frame.method === 'item/agentMessage/delta' && typeof params.delta === 'string') {
      this.startedText.add(itemId);
      if (this.startedText.size > 128) throw new Error('too many message items');
      await this.emitText('text_delta', params.delta);
    } else if (frame.method === 'item/reasoning/summaryTextDelta' && typeof params.delta === 'string') {
      if (this.reasoningStart === undefined) {
        this.reasoningStart = performance.now();
        await this.emit({type: 'reasoning_started'});
      }
      await this.emitText('reasoning_delta', params.delta);
    } else if (frame.method === 'item/started' || frame.method === 'item/completed') {
      await this.item(object(params.item), frame.method === 'item/completed');
    } else if (frame.method === 'thread/tokenUsage/updated') {
      const usage = object(object(params.tokenUsage).last);
      await this.emit({type: 'usage', input_tokens: safeCount(usage.inputTokens), output_tokens: safeCount(usage.outputTokens)});
    } else if (frame.method === 'turn/completed') {
      const turn = object(params.turn);
      if (turn.status === 'completed') await this.done('done');
      else if (turn.status === 'interrupted') await this.done('cancelled');
      else {
        const info = JSON.stringify(object(turn.error).codexErrorInfo ?? '');
        if (/usageLimitExceeded|rateLimitExceeded/.test(info)) {
          await this.error('subscription_limit', 'usage_limit', 'Codex reported a subscription limit.', this.resetAt);
        } else if (/unauthorized/.test(info)) {
          await this.error('provider_signed_out', 'authentication', 'Sign in to Codex again.');
        } else {
          await this.error('codex_turn_failed', 'unknown', 'Codex could not finish the turn.');
        }
      }
    }
  }

  private async item(item: Frame, completed: boolean) {
    const id = short(item.id, 128);
    if (!id || typeof item.type !== 'string') return;
    if (item.type === 'agentMessage' && completed) {
      if (!this.startedText.delete(id) && typeof item.text === 'string') await this.emitText('text_delta', item.text);
      return;
    }
    if (item.type === 'reasoning' && completed) {
      await this.finishReasoning();
      return;
    }
    const name = toolNames[item.type];
    if (!name) return;
    if (!completed) {
      if (this.tools.size >= 128 || this.tools.has(id)) throw new Error('too many tool items');
      const step = {name, summary: `Codex ${name}`};
      this.tools.set(id, step);
      await this.emit({type: 'tool_started', id, ...step});
    } else {
      const step = this.tools.get(id);
      if (!step) return;
      this.tools.delete(id);
      if (typeof item.aggregatedOutput === 'string' && item.aggregatedOutput) {
        await this.emit({type: 'tool_output', id, ...step, bytes: Buffer.byteLength(item.aggregatedOutput)});
      }
      await this.emit({
        type: 'tool_finished', id, ...step,
        failed: ['failed', 'declined'].includes(String(item.status)), detail: '',
      });
    }
  }

  private async finishReasoning() {
    if (this.reasoningStart === undefined) return;
    const duration_ms = Math.round(performance.now() - this.reasoningStart);
    this.reasoningStart = undefined;
    await this.emit({type: 'reasoning_finished', duration_ms});
  }

  private async error(code: string, failureClass: string, message: string, reset_at_ms?: number) {
    if (this.terminal) return;
    await this.emit({type: 'error', code, class: failureClass, billing: 'subscription', message, reset_at_ms});
    await this.done('failed');
  }

  private async done(outcome: string) {
    if (this.terminal) return;
    this.terminal = true;
    await this.finishReasoning();
    await this.emit({type: 'done', outcome});
    this.finish();
  }

  private async input() {
    for await (const frame of jsonLines(process.stdin, 64 * 1024)) {
      if (frame.version !== 1 || frame.seq !== ++this.commandSeq || this.terminal) break;
      const command = object(frame.command);
      switch (command.type) {
        case 'check': await this.check(); await this.done('done'); return;
        case 'start': if (this.start) throw new Error('duplicate start'); this.start = command; break;
        case 'send':
          void this.send(command).catch(() => this.error('codex_setup_failed', 'permanent', 'Codex could not start the configured turn.'));
          break;
        case 'steer': await this.steer(command); break;
        case 'stop': await this.stop(); break;
        case 'approve': if (typeof command.request_id === 'string') this.resolveApproval(command.request_id); break;
        default: throw new Error('invalid command');
      }
    }
    if (!this.waiting) await this.done('outcome_unknown');
    else await this.settled;
  }

  async run() {
    try {
      await Promise.race([this.settled, this.input()]);
    } catch {
      await this.error('codex_protocol', 'unknown', 'The Codex turn could not finish its protocol.');
    } finally {
      for (const key of this.approvals.keys()) this.resolveApproval(key);
      await this.rpc?.close();
      process.stdin.destroy();
    }
  }
}

/** Run the adapter until the one admitted turn reaches an outcome. */
export async function main(): Promise<void> {
  await new Adapter().run();
}
