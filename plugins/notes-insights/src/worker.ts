import type { Stats, WorkerRequest, WorkerResponse } from "@calternal/contracts";
import {
  MAX_WORKER_INPUT_BYTES,
  MAX_WORKER_OUTPUT_BYTES,
  parseWorkerRequest,
  validateWorkerResponse,
} from "@calternal/plugin-sdk";

function summarize(markdown: string): Stats {
  const words = markdown.match(/[\p{L}\p{N}]+(?:['’][\p{L}\p{N}]+)*/gu) ?? [];
  const breaks = markdown.match(/\r\n|\r|\n/gu) ?? [];
  const endsInBreak = /(?:\r\n|\r|\n)$/u.test(markdown);
  return {
    words: words.length,
    characters: Array.from(markdown).length,
    lines: markdown.length === 0 ? 0 : breaks.length + (endsInBreak ? 0 : 1),
  };
}

function makeResponse(request: WorkerRequest): WorkerResponse {
  const stats = summarize(request.resource.markdown);
  return {
    protocol_version: 1,
    invocation_id: request.invocation_id,
    ok: true,
    data: {
      stats,
      view: {
        genui_version: 1,
        root: {
          type: "stack",
          id: "notes-insights.stats",
          children: [
            { type: "stat", id: "notes-insights.words", label: "Words", value: String(stats.words) },
            { type: "stat", id: "notes-insights.characters", label: "Characters", value: String(stats.characters) },
            { type: "stat", id: "notes-insights.lines", label: "Lines", value: String(stats.lines) },
          ],
        },
      },
    },
  };
}

async function readInput(): Promise<Uint8Array> {
  const chunks: Uint8Array[] = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    const bytes = typeof chunk === "string" ? new TextEncoder().encode(chunk) : chunk;
    size += bytes.byteLength;
    if (size > MAX_WORKER_INPUT_BYTES) throw new Error("input limit exceeded");
    chunks.push(bytes);
  }
  const joined = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    joined.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return joined;
}

async function writeResponse(response: WorkerResponse): Promise<void> {
  const line = JSON.stringify(response);
  const bytes = new TextEncoder().encode(line);
  if (bytes.byteLength > MAX_WORKER_OUTPUT_BYTES) throw new Error("output limit exceeded");
  await new Promise<void>((resolve, reject) => {
    process.stdout.write(bytes, (error?: Error | null) => error ? reject(error) : resolve());
  });
}

async function main(): Promise<void> {
  const input = await readInput();
  const request = parseWorkerRequest(input);
  const response = validateWorkerResponse(makeResponse(request), request.invocation_id);
  await writeResponse(response);
}

main().catch(() => {
  // Diagnostics never include request contents or exception details.
  process.stderr.write("notes-insights worker protocol failure\n");
  process.exitCode = 1;
});
