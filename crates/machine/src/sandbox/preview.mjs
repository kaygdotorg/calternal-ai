// This reader runs in the verified private Podman network namespace.
import http from 'node:http';
import { once } from 'node:events';

const [port, path, limit, timeout] = process.argv.slice(1);
const deadline = setTimeout(() => process.exit(1), Number(timeout));
let request;
try {
  request = http.get({ host: '127.0.0.1', port, path, agent: false, maxHeaderSize: 16_384 });
  const [response] = await once(request, 'response');
  const { statusCode: status, headers } = response;
  if (status < 200 || status > 599 || (status >= 300 && status < 400)) throw new Error('status');
  const content_type = String(headers['content-type'] ?? '').slice(0, 160);
  let remaining = Number(limit);
  const write = async (data) => {
    if (!process.stdout.write(data)) await once(process.stdout, 'drain');
  };
  await write(JSON.stringify({ status, content_type }) + '\n');
  for await (const chunk of response) {
    remaining -= chunk.length;
    if (remaining < 0) throw new Error('size');
    await write(chunk);
  }
} catch {
  process.exitCode = 1;
} finally {
  request?.destroy();
  clearTimeout(deadline);
}
