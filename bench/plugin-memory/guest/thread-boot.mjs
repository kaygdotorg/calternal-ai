// Bench-only worker-thread bootstrap. A worker thread's stdin stream does not
// keep its event loop alive the way a process's stdin pipe does, so an
// unchanged worker bundle would exit while waiting for its first frame.
// Keep the thread alive, then load the plugin's own entry with the argv a
// process would see.
import { pathToFileURL } from 'node:url';

setInterval(() => {}, 1 << 30);
process.argv.splice(1, 1);
await import(pathToFileURL(process.argv[1]).href);
