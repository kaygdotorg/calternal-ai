/** ChatGPT device authorization through the official App Server. */
import { CodexRpc } from './rpc.js';
import { prepareProfile } from './profile.js';

/** Relay one browser challenge. Credential storage and polling remain in Codex. */
export async function signIn(): Promise<void> {
  if (process.env.CALTERNAL_PROVIDER_LOGIN !== '1') throw new Error('no private login lease');
  const cwd = process.cwd();
  const profile = await prepareProfile(cwd, 'least_access');
  let complete!: (success: boolean) => void;
  const result = new Promise<boolean>(resolve => { complete = resolve; });
  const rpc = new CodexRpc(profile.binary, profile.args, cwd, profile.env, async frame => {
    if (frame.method === 'account/login/completed') {
      complete((frame.params as { success?: boolean })?.success === true);
    }
  });
  try {
    await rpc.initialize();
    const challenge = await rpc.request('account/login/start', {type: 'chatgptDeviceCode'});
    const {verificationUrl, userCode} = challenge;
    if (challenge.type !== 'chatgptDeviceCode'
      || verificationUrl !== 'https://auth.openai.com/codex/device'
      || typeof userCode !== 'string' || !/^[A-Z0-9-]{1,32}$/i.test(userCode)) {
      throw new Error('invalid device challenge');
    }
    process.stdout.write(JSON.stringify({url: verificationUrl, display_code: userCode, needs_code: false}) + '\n');
    const timer = setTimeout(() => complete(false), 9 * 60_000);
    try {
      if (!await result) throw new Error('device sign-in failed');
      const status = await rpc.request('account/read', {refreshToken: false});
      if ((status.account as {type?: string})?.type !== 'chatgpt') throw new Error('sign-in did not produce a ChatGPT subscription');
    } finally {
      clearTimeout(timer);
    }
  } finally {
    await rpc.close();
  }
}
