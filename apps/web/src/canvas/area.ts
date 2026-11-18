/** Registers the canvas as the shell's window area (shell/window-area.ts): the one place the shell's window area is connected to the canvas. Imported once, for its effect, by main.ts. */
import { registerWindowArea } from "../shell/window-area";
import { canvasActive } from "./model.svelte";

registerWindowArea({ id: "canvas", active: canvasActive, load: () => import("./Canvas.svelte").then((m) => m.default) });
