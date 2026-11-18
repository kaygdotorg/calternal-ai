/** esbuild embeds packaged Markdown as UTF-8 text in the isolated bundle. */
declare module "*.md" {
  const markdown: string;
  export default markdown;
}

/** esbuild embeds the same question worker text used by the CLI scaffold. */
declare module "*.txt" {
  const text: string;
  export default text;
}
