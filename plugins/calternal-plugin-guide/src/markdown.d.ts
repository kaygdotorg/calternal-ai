/** esbuild embeds packaged Markdown as UTF-8 text in the isolated bundle. */
declare module "*.md" {
  const markdown: string;
  export default markdown;
}
