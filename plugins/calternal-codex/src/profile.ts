/** Construct the private profile and effective Codex sandbox policy. */
import { lstat, mkdir, readlink, symlink } from 'node:fs/promises';
import { isAbsolute, join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const run = promisify(execFile);

/** Member-selected mode supplied by the Calternal host, never by the prompt. */
export type AgentMode = 'least_access' | 'approve_for_me' | 'full_access';

/** Absolute official binary and environment owned by this sandbox. */
export interface Profile {
  /** The documented executable installed in the immutable image. */
  binary: string;
  /** CLI config overrides for the two confined permission profiles. */
  args: string[];
  /** The CLI environment with provider auth ingress limited to the private home. */
  env: NodeJS.ProcessEnv;
  /** Named permission profile selected for this turn. */
  permissions: string;
  /** Native approval setting selected by the mode. */
  approvalPolicy: 'on-request' | 'never';
}

/** Find the exact executable version, discarding vendor stderr. */
export async function version(binary: string): Promise<string> {
  const { stdout } = await run(binary, ['--version'], {timeout: 3_000, maxBuffer: 4096});
  const match = /^codex-cli ([a-z0-9.+-]+)\s*$/i.exec(stdout);
  if (!match) throw new Error('missing official Codex CLI version');
  return match[1];
}

async function sameLink(target: string, path: string) {
  try {
    await symlink(target, path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST' || await readlink(path) !== target) {
      throw new Error('unsafe provider login path');
    }
  }
}

/** Bind only Codex's own auth and lock files into the current per-space home.
 * Codex resolves the auth link for atomic refresh and locks the shared target.
 * Regular command tools are denied reads of both the link and its destination.
 */
export async function prepareProfile(workspace: string, mode: AgentMode, withBroker = false): Promise<Profile> {
  const login = process.env.CALTERNAL_PROVIDER_LOGIN_DIR;
  const home = process.env.HOME;
  const gateway = process.env.CALTERNAL_GATEWAY_URL;
  if (!home || !isAbsolute(home) || (!gateway && (!login || !isAbsolute(login)))) throw new Error('isolated provider profile is unavailable');
  if (!gateway && login) {
    const loginStat = await lstat(login);
    if (!loginStat.isDirectory() || (loginStat.mode & 0o077) !== 0) throw new Error('provider login directory is not private');
  }
  const codexHome = join(home, '.codex');
  try {
    await mkdir(codexHome, { mode: 0o700 });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
  }
  const homeStat = await lstat(codexHome);
  if (!homeStat.isDirectory() || (homeStat.mode & 0o077) !== 0) throw new Error('unsafe Codex home');
  if (!gateway && login) for (const name of ['auth.json', 'auth.lock']) {
    await sameLink(join(login, name), join(codexHome, name));
  }
  const binary = process.env.CALTERNAL_CODEX_BINARY ?? '/usr/local/bin/codex';
  // Named permission profiles are supported by the pinned App Server schema.
  // Fixed low-level capabilities stay inside the Calternal sandbox.
  const quote = JSON.stringify;
  const cwd = resolve(workspace);
  const files = [
    '":minimal" = "read"',
    `${quote(cwd)} = "${mode === 'least_access' ? 'read' : 'write'}"`,
    `${quote(gateway ? '/run/calternal-broker' : login)} = "deny"`,
    `${quote(codexHome)} = "deny"`,
    '"/usr/local/bin/calternal" = "read"',
    '"/nix/store" = "read"',
  ];
  const env: NodeJS.ProcessEnv = { CODEX_HOME: codexHome };
  for (const key of ['PATH', 'HOME', 'LANG', 'TMPDIR', 'CALTERNAL_URL', 'CALTERNAL_AGENT_TOKEN',
    'SSL_CERT_FILE', 'SSL_CERT_DIR', 'CODEX_CA_CERTIFICATE']) {
    if (process.env[key]) env[key] = process.env[key];
  }
  const permissions = 'calternal-workspace';
  const broker = withBroker && env.CALTERNAL_URL ? new URL(env.CALTERNAL_URL) : undefined;
  if (broker && (!['http:', 'https:'].includes(broker.protocol) || broker.username || broker.password)) {
    throw new Error('invalid Calternal broker URL');
  }
  const network = broker ? [
    `permissions.${permissions}.network.enabled=true`,
    `permissions.${permissions}.network.mode="full"`,
    `permissions.${permissions}.network.domains={${quote(broker.hostname)}="allow"}`,
  ] : [];
  if (gateway) env.CALTERNAL_GATEWAY_TOKEN = process.env.CALTERNAL_GATEWAY_TOKEN;
  const gatewayArgs = gateway ? [
    'model_provider="calternal_shared"',
    'model_providers.calternal_shared.name="Calternal shared gateway"',
    `model_providers.calternal_shared.base_url=${quote(gateway + '/v1')}`,
    'model_providers.calternal_shared.env_key="CALTERNAL_GATEWAY_TOKEN"',
    'model_providers.calternal_shared.wire_api="responses"',
    'features.shell_tool=false',
    'features.unified_exec=false',
    'features.code_mode_host=false',
    'features.api_key_model_discovery=false',
    'features.enable_request_compression=false',
    'mcp_servers.workspace.command="/usr/local/bin/node"',
    'mcp_servers.workspace.args=["/run/calternal-broker/client.mjs","mcp"]',
    'mcp_servers.workspace.enabled_tools=["workspace_command"]',
    'mcp_servers.workspace.env_vars=["CALTERNAL_GATEWAY_TOKEN"]',
    'mcp_servers.workspace.required=true',
  ] : [];
  const args = [
    ...gatewayArgs,
    'cli_auth_credentials_store="file"',
    `default_permissions="${permissions}"`,
    `permissions.${permissions}.filesystem={${files.join(',')}}`,
    `projects.${quote(cwd)}.trust_level="untrusted"`,
    'allow_login_shell=false',
    'shell_environment_policy={inherit="all",ignore_default_excludes=true,include_only=["PATH","HOME","LANG","TMPDIR","CALTERNAL_URL","CALTERNAL_AGENT_TOKEN","SSL_CERT_FILE","SSL_CERT_DIR"]}',
    ...network,
    'features.hooks=false',
    'features.plugins=false',
    'features.apps=false',
    'features.multi_agent=false',
    'web_search="disabled"',
  ].flatMap(value => ['-c', value]);
  return { binary, args, env, permissions, approvalPolicy: mode === 'approve_for_me' ? 'on-request' : 'never' };
}
