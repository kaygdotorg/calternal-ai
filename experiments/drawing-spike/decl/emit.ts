// Serialises the four manifests to data/declarations.json: what the host would check at install and give every client.
// Run: node --experimental-strip-types decl/emit.ts (Node 22.6 or later; the default from Node 23.6).
import { writeFileSync } from "node:fs";
import readingList from "./reading-list.ts";
import chat from "./chat.ts";
import chatAi from "./chat-ai.ts";
import composer from "./chat-composer.ts";
const out = new URL("../data/declarations.json", import.meta.url);
writeFileSync(out, JSON.stringify({ plugins: [readingList, chat, chatAi, composer] }, null, 1) + "\n");
console.log(`wrote ${out.pathname}`);
