/** The accent colour the page is using now (`--accent-base`, set on the root by the appearance preferences), as `#rrggbb`, for the native chrome's tint; undefined when it is not a plain hex. */
export function accentOf(): string | undefined {
  const v = typeof document === "undefined" ? "" : document.documentElement.style.getPropertyValue("--accent-base").trim();
  return /^#[0-9a-f]{6}$/i.test(v) ? v : undefined;
}
