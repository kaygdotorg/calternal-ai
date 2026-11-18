// Disposable dev-01 proxy. Its HTTPS listener is available only through the host.
import { readFileSync } from "node:fs";
import { createServer, request } from "node:http";
import { createServer as createTlsServer } from "node:https";

const [publicName, backendAddress, backendPort, tlsPort, machinePort, certDir, machineAddress = backendAddress] = process.argv.slice(2);
if (!certDir) throw new Error("Usage: rehearsal-proxy.mjs AUTHORITY BACKEND_IP BACKEND_PORT TLS_PORT MACHINE_PORT CERT_DIR [MACHINE_IP]");

function forward(req, res) {
  const tls = Boolean(req.socket.encrypted);
  if ((tls && req.headers.host !== publicName)
      || (!tls && !req.url.startsWith("/api/v1/machine/"))) {
    res.writeHead(404).end();
    return;
  }
  const headers = Object.fromEntries(Object.entries(req.headers)
    .filter(([key]) => key !== "forwarded" && !key.startsWith("x-forwarded-")));
  headers.host = publicName;
  headers["x-forwarded-host"] = publicName;
  headers["x-forwarded-proto"] = tls ? "https" : "http";
  headers["x-forwarded-for"] = req.socket.remoteAddress;
  const upstream = request({
    host: backendAddress, localAddress: backendAddress, port: backendPort,
    path: req.url, method: req.method, headers,
  }, reply => {
    res.writeHead(reply.statusCode, reply.headers);
    reply.on("aborted", () => res.destroy());
    reply.pipe(res);
  });
  upstream.on("error", () => {
    if (res.headersSent) res.destroy();
    else res.writeHead(502).end();
  });
  res.on("close", () => upstream.destroy());
  req.pipe(upstream);
}

createTlsServer({
  key: readFileSync(`${certDir}/proxy.key`),
  cert: readFileSync(`${certDir}/proxy.crt`),
}, forward).listen(Number(tlsPort), "127.0.0.1");
createServer(forward).listen(Number(machinePort), machineAddress);
