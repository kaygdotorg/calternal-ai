# Plugin SDK

The v2 authoring builders describe a package without a running space. Run
`npm run build -w @calternal/plugin-sdk` from the checkout root to generate
TypeScript types from the contract schemas, bundle the validation inputs,
and compile the SDK. Import
`definePlugin`, `defineApp`, `defineKind`, `defineOperation`, `defineView` and
`defineChrome` from `@calternal/plugin-sdk`.

In a workspace package, use `calternal-build-plugin src/plugin.ts dist` to
validate and emit the package. The source module must default-export the result
of `definePlugin`. Pass a bundled worker as the third argument if the manifest
declares worker operations. Node 24 loads the TypeScript declaration module.
Run `tsc --noEmit` first for TypeScript diagnostics. The build checks the
serialized manifest against its schema and each description against the
checked-in Python verifier using that manifest. It publishes `dist/manifest.json`
with SHA-256 references to the validated template and bundled worker bytes.
Build output is disposable and can be deleted between builds.

The [author walkthrough](../../plugins/calternal-plugin-guide/walkthrough.md)
has a complete first list. The [reading-list package](../../plugins/calternal-reading-list/)
shows search, forms and host-owned row actions. Package validity at build time
covers authoring. Installation, authorization and runtime effects belong to the host.

For an already emitted package, import `verifyPackage` from
`@calternal/plugin-sdk/verify-package` and pass the output directory. It
checks the manifest, template and worker pins, and descriptions without
loading the plugin source.
