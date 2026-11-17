// Copies the self-hosted font files into public/fonts (gitignored). Google Sans Flex (OFL 1.1): weight + optical size for light, plus
// GRAD for dark (made once, kept in packages/ui/fonts/README.md). Maple Mono (OFL 1.1): code. Latin and latin-ext subsets only.
import { copyFileSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "..", "public", "fonts");
mkdirSync(out, { recursive: true });
const req = createRequire(import.meta.url);
const pkgDir = (name) => dirname(req.resolve(`${name}/package.json`));
const gsf = join(pkgDir("@fontsource-variable/google-sans-flex"), "files");
const maple = join(pkgDir("@fontsource/maple-mono"), "files");
const script = join(pkgDir("@fontsource-variable/dancing-script"), "files");
const own = join(here, "../../../packages/ui/fonts");
const files = [
  [gsf, "google-sans-flex-latin-opsz-normal.woff2"],
  [gsf, "google-sans-flex-latin-ext-opsz-normal.woff2"],
  [gsf, "google-sans-flex-symbols-opsz-normal.woff2"],
  [gsf, "google-sans-flex-latin-rond-normal.woff2"], // the rounded variant (app name, avatar initials)
  [own, "google-sans-flex-latin-opsz-grad-normal.woff2"],
  [own, "google-sans-flex-latin-ext-opsz-grad-normal.woff2"],
  [script, "dancing-script-latin-wght-normal.woff2"], // the wordmark (the owner's calternal-ui plugin default)
  [maple, "maple-mono-latin-400-normal.woff2"],
  [maple, "maple-mono-latin-500-normal.woff2"],
];
for (const [dir, name] of files) copyFileSync(join(dir, name), join(out, name));
console.log(`fonts: copied ${files.length} files to public/fonts`);
