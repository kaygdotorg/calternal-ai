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
appdata. No image rebuild is needed. Package input is a built directory or
one bounded `.calplugin` archive. Run `calternal plugin pack <dir> <archive>`
to generate an archive. The manifest's `api_version` selects the reader.

### Manifest v2

Use `calternal plugin review <source>` to see the declared kinds, Bind reads,
operations, broker calls, network permissions, credential routes, jobs, schedules and inbound webhooks. For a package that requests
neither network access nor anonymous host image fetches, the review shows
**No network access**. The review uses the pinned v2 schemas and verifies the
SHA-256 of each pinned package asset. Use the review checksum with
`calternal plugin install <source> --appdata <dir> --approve <sha256>`.

The installer publishes an installation-local declaration for the kind and
operation registry, with no space activation or worker start. The v2 installer
currently accepts new packages and supports removal. Updates and space
enablement are unavailable. Heavy jobs, scratch disks and conversation kinds
are refused. Revision 13.18 supplies the `host_create` declaration and its checked follow-up shape.

### Manifest v1

Version 1 directories contain `manifest.json` and may include a bundled
`dist/worker.js`.

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

The catalogue reserves each package's operation prefixes. Manifest v2
also reserves its app IDs for that package. An app or package cannot claim
a host operation namespace. The compiled-in namespace owner map assigns
shared prefixes whose exact host operations stay reserved. Operator
distribution catalogues cannot supply this map. Installed
packages keep their original registration order through updates. Startup
registers valid rows in that order, checks reads against the complete
catalogue and then applies skipped-row reservations.
A reservation blocks reassignment, while the current live owner can retain
its existing names on update. If a stored package no longer validates, the
server keeps its recoverable names and folders reserved while its
operations and kinds are unavailable. A dependent v2 package stays
unavailable while its read has no owner. Resolve the unavailable rows
before assigning those names elsewhere. If the server cannot recover the
declarations from the archive, it names the row to remove before accepting
new declarations.

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

### Revision 14 draft packages

Use one reviewed contract checkout for generation and package validation.
For a revision 14.32 checkout, point the SDK at its absolute contract
directory and set the expected revision.

```sh
export CALTERNAL_PLUGIN_CONTRACT="$PWD/contract"
export CALTERNAL_PLUGIN_CONTRACT_REVISION=14.32
npm run build -w @calternal/plugin-sdk
node packages/plugin-sdk/scripts/build-author-package.mjs ./plugin.ts ./dist ./worker.mjs
python3 -B "$CALTERNAL_PLUGIN_CONTRACT/verify-description.py" --package ./dist
```

The [Threads walk](description-contract.md#93-threads) uses two packages.
The app declares the extension point and owns the item. The contributor
names the app as `{package: "calternal-threads", app: "threads"}`. Its
manifest pins an image digest and entry bytes and can declare
`image_source: "registry.example.org/community/harness"`. The builder
takes entry paths relative to the author module and packages verified
bytes under a content-addressed name. Declare `execution_network` for
harness egress. The worker's network approval is separate.

Use `authorContext` from `@calternal/plugin-sdk/author-context` in the
pinned carrier adapter. Supply the validated invoke and a broker call
already bound to it. Hand the returned `RevisionedAuthorContext` to the
operation. Use its host-authored clock, actor and request identity for
deterministic writes and receipts. The app creates an item with
`item_create`, and an operation declaring `creates` returns
`{created: result.item}`. Send starts a job. The job reserves an attempt
and calls the checked contribution. Use top-level `item` for the
checked target on `start_job` and `alarm_set`. The host injects the
declared run-operation field. Use `encodeContributionFrame` from
`@calternal/plugin-sdk/contribution-frame` on the whole frame before
sending it. It serializes UTF-8 JSON with LF and throws
`contribution_frame_limit` when over the catalogue bound. A fake host can return a typed output
frame and then a terminal frame. The real placement supplies the same
frames through the [entry ABI](../contract/host-catalogue-v2.json#/extension_points/harness_abi).
Only the owner commits its own item. Reuse a subeffect key for each
consumed frame, and handle `head_conflict` by rereading and checking the
new head before issuing a different write receipt. An alarm handler checks
host-stamped `missed` and applies the app's chosen behavior. A write run
can publish an authorized snapshot for the saved revision.

The draft package checks validate declarations, types and asset pins.
Live execution and publication qualify through the API's Step 2 gates.
