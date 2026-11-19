// Package the built SDK and its locked production dependencies for an offline trial.
import { execFileSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const sdk = join(repo, "packages/plugin-sdk");
const trialSources = fileURLToPath(new URL("./", import.meta.url));
const readJson = async file => JSON.parse(await readFile(file, "utf8"));

async function installedDependency(dependency, from) {
  const require = createRequire(join(from, "package.json"));
  for (const directory of require.resolve.paths(dependency) ?? []) {
    const candidate = join(directory, dependency);
    try {
      await stat(join(candidate, "package.json"));
      return realpath(candidate);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  throw new Error(`Install the workspace dependency ${dependency} before preparing the trial.`);
}

async function packSdk(stage, destination) {
  const manifest = await readJson(join(sdk, "package.json"));
  // The offline kit also carries the compiler used by its build command.
  manifest.dependencies.typescript = manifest.devDependencies.typescript;
  const direct = Object.keys(manifest.dependencies);
  const bundled = new Map();
  delete manifest.devDependencies;
  delete manifest.scripts;
  manifest.bundleDependencies = direct;
  manifest.files = ["dist", "generated", "scripts"];

  async function include(dependency, from) {
    const source = await installedDependency(dependency, from);
    const previous = bundled.get(dependency);
    if (previous === source) return;
    if (previous) throw new Error(`Trial needs a nested copy of ${dependency}.`);
    bundled.set(dependency, source);
    const { dependencies = {} } = await readJson(join(source, "package.json"));
    const output = join(stage, "node_modules", dependency);
    await mkdir(output, { recursive: true });
    await cp(source, output, {
      recursive: true,
      filter: file => !["node_modules", ".git"].includes(basename(file)) || file === source,
    });
    for (const child of Object.keys(dependencies)) await include(child, source);
  }

  for (const folder of ["dist", "generated"]) {
    await cp(join(sdk, folder), join(stage, folder), { recursive: true });
  }
  await cp(join(repo, "LICENSE"), join(stage, "LICENSE"));
  await mkdir(join(stage, "scripts"));
  for (const file of ["build-author-package.mjs", "verify-package.mjs"]) {
    await cp(join(sdk, "scripts", file), join(stage, "scripts", file));
  }
  await writeFile(join(stage, "package.json"), JSON.stringify(manifest, null, 2) + "\n");
  for (const dependency of direct) await include(dependency, sdk);

  const metadata = execFileSync("npm", ["pack", "--offline", "--ignore-scripts", "--json", "--pack-destination", destination],
    { cwd: stage, encoding: "utf8" });
  return JSON.parse(metadata)[0].filename;
}

async function prepare(destination) {
  const stage = await mkdtemp(join(tmpdir(), "calternal-trial-pack-"));
  try {
    const archive = await packSdk(stage, destination);
    let guide = (await readFile(join(trialSources, "GUIDE.md"), "utf8"))
      .replace("calternal-plugin-sdk-0.1.0.tgz", archive);
    for (const [filename, language] of [
      ["first-list.ts", "ts"], ["gear-list.ts", "ts"], ["gear-worker.mjs", "js"],
    ]) {
      const example = await readFile(join(repo, "plugins/calternal-plugin-guide/examples", filename), "utf8");
      guide = guide.replace(`<!-- ${filename} -->`, () => `\`\`\`${language}\n${example.trimEnd()}\n\`\`\``);
    }
    await writeFile(join(destination, "GUIDE.md"), guide);
    for (const file of ["TASK.md", "build-and-check.sh", "check.mjs"]) {
      await cp(join(trialSources, file), join(destination, file));
    }
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
}

try {
  if (process.argv.length !== 3) throw new Error("Usage: node prepare.mjs <new-trial-directory>");
  const destination = resolve(process.argv[2]);
  await mkdir(destination);
  try {
    await prepare(destination);
  } catch (error) {
    await rm(destination, { recursive: true, force: true });
    throw error;
  }
  console.log(destination);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
