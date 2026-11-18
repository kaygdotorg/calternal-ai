// A copy of a rendered element that looks the same outside its parents (a list row lifted onto the dimmed page, a long-press preview). Its
// computed styles are copied across element by element once, so no rule that depended on an ancestor is missed. Used by the row press
// (lib/press.ts, components/PressSurface.svelte). Ported from the existing app's packages/ui/src/lib/lift.ts.

// What a look-alike needs to copy: the properties that draw or place a box and its words. (Copying every computed property of every
// element took 80 ms for one task row, which stalls the touch that is still down; these take a sixth of that.) Custom properties and
// everything an element inherits from `:root` come for free; a rule that depends on an ancestor is what the copy is for.
const PROPS = [
  "display", "position", "box-sizing", "width", "height", "min-width", "min-height", "max-width", "max-height", "aspect-ratio",
  "margin-top", "margin-right", "margin-bottom", "margin-left", "padding-top", "padding-right", "padding-bottom", "padding-left",
  "flex-direction", "flex-wrap", "flex-grow", "flex-shrink", "flex-basis", "order", "gap", "row-gap", "column-gap",
  "align-items", "align-self", "align-content", "justify-content", "justify-items", "justify-self", "place-items",
  "grid-template-columns", "grid-template-rows", "grid-column", "grid-row", "grid-area", "inset", "top", "left", "right", "bottom", "z-index",
  "overflow-x", "overflow-y", "text-overflow", "white-space", "word-break", "overflow-wrap", "text-align", "vertical-align",
  "color", "font-family", "font-size", "font-weight", "font-style", "font-variant-numeric", "font-feature-settings", "line-height", "letter-spacing",
  "text-transform", "text-decoration-line", "text-decoration-color", "text-shadow", "text-wrap", "-webkit-line-clamp", "-webkit-box-orient",
  "background-color", "background-image", "background-size", "background-position", "background-repeat", "opacity",
  "border-top-width", "border-right-width", "border-bottom-width", "border-left-width", "border-top-style", "border-right-style", "border-bottom-style", "border-left-style",
  "border-top-color", "border-right-color", "border-bottom-color", "border-left-color",
  "border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius",
  "box-shadow", "outline", "filter", "mask-image", "clip-path", "object-fit", "object-position", "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin",
  "transform", "scale", "translate", "rotate", "transform-origin",
] as const;

/** Copies how `from` looks into `to`, element by element. Both must have the same shape (`to` is a deep clone of `from`). */
export function dress(from: HTMLElement, to: HTMLElement) {
  const a = [from, ...from.querySelectorAll<HTMLElement>("*")];
  const b = [to, ...to.querySelectorAll<HTMLElement>("*")];
  a.forEach((el, i) => {
    const cs = getComputedStyle(el);
    const st = b[i]?.style;
    if (!st) return;
    for (const p of PROPS) st.setProperty(p, cs.getPropertyValue(p));
    // Not `visibility: visible` (that would stop a hidden look-alike hiding what is inside it); only what the original itself hides stays hidden.
    if (cs.visibility === "hidden") st.visibility = "hidden";
  });
  to.style.transform = "";
  to.style.translate = "none";
  to.style.transition = "none";
  to.style.animation = "none";
}

/** A dressed deep copy of `src`, ready to be put anywhere. */
export function lookalike(src: HTMLElement): HTMLElement {
  const copy = src.cloneNode(true) as HTMLElement;
  dress(src, copy);
  copy.style.margin = "0";
  copy.style.pointerEvents = "none";
  copy.removeAttribute("id");
  return copy;
}
