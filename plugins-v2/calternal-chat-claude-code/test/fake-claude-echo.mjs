#!/usr/bin/env node
import net from "node:net";
import { spawn } from "node:child_process";

const args = process.argv.slice(2);
if (args[0] === "auth" && args[1] === "status") process.exit(1);
if (!args.includes("-p")) process.exit(2);

let prompt = "";
for await (const chunk of process.stdin) prompt += chunk;

function egressProbe(host = "api.anthropic.com") {
  return new Promise((resolve, reject) => {
    const socket = net.connect(17853, "127.0.0.1");
    let response = Buffer.alloc(0);
    let tunneled = false;
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("egress timeout"));
    }, 2000);
    socket.once("connect", () => {
      socket.write(`CONNECT ${host}:443 HTTP/1.1\r\nHost: ${host}:443\r\n\r\n`);
    });
    socket.on("data", chunk => {
      if (tunneled) {
        response = Buffer.concat([response, chunk]);
        if (response.includes("world")) {
          clearTimeout(timer);
          socket.end();
          resolve("world");
        }
        return;
      }
      response = Buffer.concat([response, chunk]);
      const end = response.indexOf("\r\n\r\n");
      if (end < 0) return;
      const status = response.subarray(0, end).toString("ascii");
      if (!status.startsWith("HTTP/1.1 200")) {
        clearTimeout(timer);
        socket.destroy();
        reject(new Error("proxy refused"));
        return;
      }
      tunneled = true;
      const early = response.subarray(end + 4);
      response = Buffer.alloc(0);
      socket.write("hello");
      if (early.length) response = Buffer.concat([response, early]);
    });
    socket.once("error", error => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

function directProbe() {
  return new Promise(resolve => {
    const socket = net.connect(443, "1.1.1.1");
    const timer = setTimeout(() => {
      socket.destroy();
      resolve("blocked");
    }, 700);
    socket.once("connect", () => {
      clearTimeout(timer);
      socket.destroy();
      resolve("open");
    });
    socket.once("error", () => {
      clearTimeout(timer);
      resolve("blocked");
    });
  });
}

let text = "Echo: " + prompt;
if (prompt === "egress-check" || prompt === "egress-denied") {
  try {
    text = "Egress: " + await egressProbe(prompt === "egress-check" ? undefined : "undeclared.invalid");
  } catch {
    text = "Egress: refused";
  }
} else if (prompt === "direct-check") {
  text = "Direct: " + await directProbe();
}

let heldChild = null;
if (prompt.startsWith("hold:")) {
  process.on("SIGINT", () => {});
  heldChild = spawn(
    process.execPath,
    [
      "-e",
      "process.on('SIGINT',()=>{});const {spawn}=require('node:child_process');spawn('sh',['-c','trap \"\" INT; sleep 60'],{stdio:'ignore'});setInterval(()=>{},60000)",
    ],
    { stdio: "ignore" },
  );
}

const emit = value => process.stdout.write(JSON.stringify(value) + "\n");
emit({
  type: "system",
  subtype: "init",
  session_id: "22222222-2222-4222-8222-222222222222",
});
emit({
  type: "stream_event",
  event: {
    type: "content_block_delta",
    delta: { type: "text_delta", text },
  },
});

if (heldChild) await new Promise(resolve => heldChild.once("exit", resolve));

emit({
  type: "result",
  subtype: "success",
  is_error: false,
  usage: { input_tokens: 0, output_tokens: 0 },
});
