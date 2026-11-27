#!/usr/bin/env node
import { spawn } from "node:child_process";

const args = process.argv.slice(2);
if (args[0] === "auth" && args[1] === "status") {
  process.exit(1);
}
if (!args.includes("-p")) {
  process.exit(2);
}

let prompt = "";
for await (const chunk of process.stdin) prompt += chunk;

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
    delta: { type: "text_delta", text: `Echo: ${prompt}` },
  },
});

if (prompt.startsWith("hold:")) {
  const child = spawn("sh", ["-c", "sleep 60"], { stdio: "ignore" });
  await new Promise(resolve => child.once("exit", resolve));
}

emit({
  type: "result",
  subtype: "success",
  is_error: false,
  usage: { input_tokens: 0, output_tokens: 0 },
});
