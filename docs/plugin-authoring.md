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
   Restart the server to load the new catalog.
3. A space manager runs `calternal plugin enable <id> --space <uuid>` with
   their normal server connection and credential. This uses `plugins.enable`
   and provisions its kinds and collections before activation.
4. Run `calternal plugin update` with the same arguments as install and a
   fresh review checksum. Run `calternal plugin review <source> --appdata <dir>`
   first to see changed operation policies and added broker calls or secret names.
   A changed authority request requires the affected space managers to enable
   the package again. Other updates retain existing activations. Changes to kinds and placements
   require a separate migration plan.
5. Use `calternal plugin disable` for immediate space revocation. Use
   `calternal plugin remove <id> --appdata <dir>` to remove the package at the
   next server start. Space content remains under the normal storage rules.

Shipped packages seed the same installation-local SQLite catalog on first
server start. The catalog holds at most 128 packages, each with a 256 KiB
manifest and a 1 MiB bundled worker. Each record holds the reviewed archive and
its checksum. Only the active bundle is materialized on startup. Durable jobs admitted by a superseded worker revision stop at the revision check.
They require a deliberate retry or migration. Preserve appdata
when reinstalling the server.

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
