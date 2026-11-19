// The one bar: the component, its registered modes, and the rule that picks one. Its look is styles/bar.css; its glass is the shared glass material (styles/glass.css), its spring the bar tokens.
export { default as Bar } from "./Bar.svelte";
export { barModes, type BarMode, type BarRole } from "./modes";
export { pickBarKey, pickBase, type BarState } from "./pick";
