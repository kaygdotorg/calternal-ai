/** Generate and validate public TypeScript reference docs for the portable GenUI package. */
import { readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
const require = createRequire(new URL("../../../scripts/docs-tools/package.json", import.meta.url));
const { Application, TSConfigReader } = await import(require.resolve("typedoc"));

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const check = true;
const packages = ["packages/genui"];

/** Discover source modules recursively so new files cannot bypass the export gate. */
async function sources(directory) {
  const result = [];
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error.code === "ENOENT") return result;
    throw error;
  }
  for (const entry of entries) {
    const filename = path.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await sources(filename));
    else if (/\.[cm]?tsx?$/.test(entry.name) && !/\.d\.[cm]?ts$/.test(entry.name)) result.push(filename);
  }
  return result.sort();
}

for (const location of packages) {
  const directory = path.join(root, location);
  const entryPoints = await sources(path.join(directory, "src"));
  if (!entryPoints.length) {
    console.log(`${location}: no TypeScript source exports (manifest-only package)`);
    continue;
  }
  const app = await Application.bootstrap({
    entryPoints,
    tsconfig: path.join(directory, "tsconfig.json"),
    name: location,
    // External contract/library symbols have their own Rust source of truth.
    excludeExternals: true,
    excludePrivate: true,
    excludeProtected: true,
    excludeInternal: false,
    readme: "none",
    requiredToBeDocumented: [
      "Namespace", "Enum", "EnumMember", "Variable", "Function", "Class",
      "Interface", "Constructor", "Property", "Method", "CallSignature",
      "IndexSignature", "ConstructorSignature", "Accessor", "TypeAlias", "Reference",
    ],
    validation: { notDocumented: true, invalidLink: true, notExported: false },
    treatWarningsAsErrors: true,
    treatValidationWarningsAsErrors: true,
    disableSources: false,
  }, [new TSConfigReader()]);
  const project = await app.convert();
  if (!project) throw new Error(`${location}: TypeScript conversion failed`);
  // TypeDoc models package-barrel aliases as References. Forward the target's full
  // reference comment instead of requiring duplicate documentation that can drift.
  for (const reflection of Object.values(project.reflections)) {
    if (!reflection.comment && typeof reflection.tryGetTargetReflection === "function") {
      const target = reflection.tryGetTargetReflection();
      const comment = target?.comment ?? target?.signatures?.find(signature => signature.comment)?.comment;
      if (comment) reflection.comment = comment.clone();
    }
  }
  app.validate(project);
  if (app.logger.hasErrors() || app.logger.hasWarnings()) {
    throw new Error(`${location}: documentation validation failed`);
  }
  if (!check) {
    await app.generateDocs(project, path.join(root, "reference-docs/typescript", path.basename(directory)));
    if (app.logger.hasErrors() || app.logger.hasWarnings()) throw new Error(`${location}: documentation generation failed`);
  }
  console.log(`${location}: documentation ${check ? "gate passed" : "built"}`);
}
