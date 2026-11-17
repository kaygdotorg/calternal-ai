// The phrase plugins the palette and the composer read with. Tasks only for now (dates, times, repeats, deadlines, priority, #tags, @people); the contacts
// phrases of the existing app stay behind their switched-off seam and are not ported. A plugin's own phrases arrive as data (manifest.ts), not as code here.
import "./phrases-tasks";
export { recognise } from "./recognise";
