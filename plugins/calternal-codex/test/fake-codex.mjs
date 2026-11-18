#!/usr/bin/env node
import {createInterface} from 'node:readline';
const threadId = '01234567-89ab-4cde-8fab-0123456789ab';
const turnId = '01234567-89ab-4cde-8fab-0123456789ac';
if (process.argv.includes('--version')) {
  console.log('codex-cli 0.170.0');
  process.exit(0);
}
const write = value => process.stdout.write(JSON.stringify(value) + '\n');
const notify = (method, params) => write({method, params: {threadId, ...params}});
for await (const line of createInterface({input: process.stdin})) {
  const {id, method, params} = JSON.parse(line);
  if (id === undefined) continue;
  let result = {};
  if (method === 'account/read') result = {account: {type: 'chatgpt'}};
  if (method === 'account/login/start') {
    if (params.type !== 'chatgptDeviceCode') process.exit(2);
    result = {type: 'chatgptDeviceCode', verificationUrl: 'https://auth.openai.com/codex/device', userCode: 'FIXTURE-0123'};
  }
  if (method === 'thread/start' || method === 'thread/resume') {
    if (params.model !== 'gpt-5.5' || params.permissions !== 'calternal-workspace' || params.approvalPolicy !== 'on-request') process.exit(2);
    if (method === 'thread/resume' && params.threadId !== threadId) process.exit(2);
    result = {thread: {id: threadId}};
  }
  if (method === 'turn/start') {
    if (params.effort !== 'high' || params.threadId !== threadId) process.exit(2);
    result = {turn: {id: turnId}};
  }
  if (method === 'turn/steer' && (params.expectedTurnId !== turnId || params.input[0].text !== 'steer')) process.exit(2);
  if (method === 'turn/interrupt' && params.turnId !== turnId) process.exit(2);
  write({id, result});
  if (method === 'account/login/start') notify('account/login/completed', {success: true});
  if (method === 'turn/start') notify('turn/started', {turn: {id: turnId}});
  if (method === 'turn/steer') notify('item/agentMessage/delta', {delta: 'Steered.', itemId: 'm1'});
  if (method === 'turn/interrupt') notify('turn/completed', {turn: {id: turnId, status: 'interrupted'}});
}
