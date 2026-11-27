#!/usr/bin/env node
import net from "node:net";

const socketPath = process.env.CALTERNAL_EGRESS_SOCKET;
if (!socketPath) process.exit(2);

const server = net.createServer(client => {
  const upstream = net.createConnection({ path: socketPath });
  client.pipe(upstream);
  upstream.pipe(client);
  client.on("error", () => upstream.destroy());
  upstream.on("error", () => client.destroy());
});
server.on("error", () => process.exit(1));
server.listen(17853, "127.0.0.1", () => {
  if (process.send) process.send({ ready: true });
});
process.on("SIGTERM", () => server.close(() => process.exit(0)));
process.on("SIGINT", () => server.close(() => process.exit(0)));
