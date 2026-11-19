/** This machine-owned bridge is mounted only into the trusted harness. */
import http from 'node:http';
import readline from 'node:readline';

const socketPath = '/run/calternal-broker/gateway.sock';
const auth = () => `Bearer ${process.env.CALTERNAL_GATEWAY_TOKEN ?? ''}`;

/** Forward the provider protocol over the private socket, replacing caller auth. */
export async function startGateway() {
  const server = http.createServer((req, res) => {
    const forward = http.request({
      socketPath, method: req.method, path: req.url,
      headers: {
        ...req.headers, authorization: auth(),
      },
    }, reply => {
      res.writeHead(reply.statusCode, reply.headers);
      reply.pipe(res);
    });
    forward.on('error', () => res.writeHead(502).end());
    req.on('aborted', () => forward.destroy());
    res.on('close', () => forward.destroy());
    req.pipe(forward);
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  return {
    baseUrl: `http://127.0.0.1:${server.address().port}`,
    close: () => { server.closeAllConnections(); server.close(); },
  };
}

function callTool(params) {
  return new Promise((resolve) => {
    const data = JSON.stringify(params);
    const req = http.request({
      socketPath, method: 'POST', path: '/tool',
      headers: { authorization: auth(), 'Content-Length': Buffer.byteLength(data) },
    }, async res => {
      const chunks = [];
      for await (const chunk of res) chunks.push(chunk);
      const isError = res.statusCode !== 200;
      resolve({content:[{type:'text', text:isError ? 'Isolated tool failed.' : Buffer.concat(chunks).toString('utf8')}], isError});
    });
    req.once('error', () => resolve({content:[{type:'text', text:'Tool broker unavailable.'}],isError:true}));
    req.end(data);
  });
}

if (process.argv[2] === 'mcp') {
  const tool = {
    name: 'workspace_command',
    description: 'Run a bounded shell command in a separate, network-disabled workspace sandbox.',
    inputSchema: {
      type: 'object',
      required: ['command'],
      properties: { command: { type: 'string', maxLength: 4096 } },
      additionalProperties: false,
    },
  };
  const send = (id, result) => process.stdout.write(JSON.stringify({jsonrpc:'2.0',id,result})+'\n');
  for await (const line of readline.createInterface({input:process.stdin})) {
    if (Buffer.byteLength(line) > 16384) break;
    const {id, method, params} = JSON.parse(line);
    if (id === undefined) continue;
    if (method === 'initialize') {
      send(id, {protocolVersion:'2024-11-05',capabilities:{tools:{}},serverInfo:{name:'workspace',version:'1'}});
    } else if (method === 'tools/list') {
      send(id, {tools:[tool]});
    } else if (method === 'tools/call' && params?.name === tool.name) {
      send(id, await callTool(params.arguments));
    } else send(id, {});
  }
}
