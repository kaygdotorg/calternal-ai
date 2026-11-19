# Author and install a TypeScript plugin

Build the SDK with `npm ci` and `npm run build:plugins` in the source checkout.
Run `calternal plugin new <name> --sdk-root <checkout>` in the directory where
you want the package. In the created package, run `npm install` and
`calternal plugin test`. The build generates a literal TypeScript manifest
from the checked JSON file, checks handler inputs and bundles one worker. The
fixture drives the same worker protocol and answers only configured callbacks.

`calternal plugin check [dir]` validates the manifest, schemas, capabilities,
risk declarations and README. JSON syntax and decoding errors include the
parser's line and column. Semantic validation currently reports the contract
category. `calternal plugin bench [dir]` prints fixture p50 and p95 for first
response, warm invocation and a warm invocation that makes three broker calls.
These measurements exclude the deployment's container, network and store.

## Live development

Use a server and CLI built with the `dev-plugin-drafts` feature. Start the
server with `--development` on disposable data and enroll a worker machine.
Use a space where your account can manage plugins.

```sh
cargo build -p calternal-server --features dev-plugin-drafts -j4
calternal plugin dev ./my-plugin --appdata "$APPDATA" --space "$SPACE_ID" --account "$ACCOUNT_ID"
```

The watcher checks the package, builds the worker, and publishes a draft once
its sandbox replies to the worker readiness challenge. Edit the source and wait
for `Ready`, then invoke the operation in that space. An invalid draft leaves
the previous revision active. `--once` builds and activates once.
`--json` prints one record per build attempt. A scaffold's watch script runs
manifest validation and esbuild. Run `calternal plugin test` for the full
TypeScript check and fixture tests. A custom package can define a
`calternal:dev` npm script. Otherwise the watcher uses its `build` script.

Draft registrations disappear when the server exits. The regular server
binary has no draft command or draft transport action. Drafts use the same
container and broker as reviewed packages, and are visible in the designated
space only.

## Installation

The server operator runs the package commands with local access to protected
appdata. No image rebuild is needed. Package input is a directory with
`manifest.json` and optional bundled `dist/worker.js`, or one bounded
`.calplugin` archive. Run `calternal plugin pack <dir> <archive>` to generate
an archive.

1. Run `calternal plugin review <source>`. Review the operation capability and
   risk table, broker calls and requested secret names. JSON output includes
   the full manifest. Retain the exact SHA-256 approval.
2. Run `calternal plugin install <source> --appdata <dir> --approve <sha256>`.
   For a package with a worker, also pass `--space <uuid> --account <uuid>`.
   The enrolled account must manage plugins in that space. The running server
   validates the checksum, starts a new sandbox, and publishes the package when
   its worker answers a readiness challenge. The challenge contains no space data.
3. A space manager runs `calternal plugin enable <id> --space <uuid>` with
   their normal server connection and credential. This uses `plugins.enable`
   and provisions its kinds and collections before activation.
4. Run `calternal plugin update` with the same arguments as install and a
   fresh review checksum. Run `calternal plugin review <source> --appdata <dir>`
   first to see changed operation policies and added broker calls or secret names.
   A changed authority request requires the affected space managers to enable
   the package again. Old work keeps its narrower grant during this approval.
   A narrowing change fences old work at its next broker call. Changes to kinds
   or collections require a separate migration plan.
5. Use `calternal plugin disable` for immediate space revocation. Use
   `calternal plugin remove <id> --appdata <dir>` to unpublish the package
   immediately. Removal fences its remaining broker calls. Space content
   remains under the normal storage rules.

To change the bundled distribution, review its entries and defaults in
`crates/server/trusted-plugins.json`. Run `npm run build:plugins` and
`npm run pin:plugins` to derive the worker hashes from the selected bundles.
Use `npm run pin:plugins -- --check` to verify the hashes. The digest test
derives its expected inventory from the seed. Authorization expectations and
behavioral examples remain independent test inputs.

Shipped packages seed the same installation-local SQLite catalog on first
server start. The catalog holds at most 128 packages, each with a 256 KiB
manifest and a 1 MiB bundled worker. Each record holds the reviewed archive and
its checksum. Superseded revisions with unchanged or preserved authority
remain available for jobs admitted under those revisions. Up to 128 retained
revisions are stored across the catalog. Narrowing or removal clears retained
authority for that package. Preserve appdata when reinstalling the server.

A trusted package is one the operator has elected to install at an exact
checksum. This records provenance within the installation. It grants no
ambient access. Normal deployment runs workers in the machine sandbox and
authorizes every host call. The explicit direct development worker is for
disposable local data. Official and community packages use the same store and
worker contract.

Enable `calternal-plugin-guide` in a space. Call
`calternal-plugin-guide.publish` there to write the reference and walkthrough.
Call `calternal-plugin-guide.explain` with `reference`, `walkthrough` or
`skill` to read the packaged Markdown. The package's skill source lives at
`skills/plugin-authoring/SKILL.md`. Agent skill discovery for installed
packages is a separate integration.

See the [plugin API decisions](plugin-api.md) and the
[interface proposal](plugin-interface-proposal.md).


## Gallery submission

Enable `calternal-plugin-submission-guide` and invoke
`calternal-plugin-submission-guide.explain` in your space for the listing
format, screenshot requirements, permissions statement and review checklist.
Run `calternal plugin package . NAME.calsubmission` after the package checks
pass. The submissions repository can run `calternal plugin verify-package`
on the same archive offline.
