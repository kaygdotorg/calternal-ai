# Prior art for a primitives-only host with powerful, bounded plugins

Lane `design-priorart`, night of 2026-11-20 (for the 2026-11-21 morning report).
Status: research and recommendations. Nothing here is decided. Real choices are in
section 7 as questions.

## How to read this file

Every claim carries a link and one label:

| Label | Meaning |
| --- | --- |
| **[V]** | Documented by the vendor or the project (the linked page says it). |
| **[R]** | Reported by developers or researchers (a third party says it). |
| **[I]** | My inference. Not stated by the source. Treat as a hypothesis. |

I fetched each linked page on 2026-11-20 (Firecrawl and Context7). Where a page was
not fetched, the file says so. I did not read any secrets. For bb I read the plugin
documentation on this machine (`bb guide plugins`, the bb plugin-authoring skill and a
copy of bb's plugin API guide at `/tmp/calternal-bb-lite-plugin-api.md`).

## 0. The three findings that matter most

1. **Safe systems enforce a declared grant at one broker.** Deno, wasmtime, Figma, Slack
   and Android all work this way. Obsidian and Raycast do not, and say so. They depend on
   review and trust. We must not copy that, because our plugins are strangers' code.
2. **A grant that can start a process or load code is no grant.** Deno documents that
   `--allow-run` leaves its own sandbox. Android shows that a callable token a plugin can
   change, or a call aimed at "whoever answers", turns one plugin into a deputy for
   another. The call table must name its target and the delegated rights must be fixed.
3. **Taking power away later costs the ecosystem.** Chrome's Manifest V3 replaced a
   code hook with declarative rules, and the argument has not ended. So offer declarative
   forms first, but keep one bounded code path from day one and never remove it.

## 1. The systems studied

Each entry answers: what the core owns; what an extension can and cannot do; sandbox and
grants; how extensions talk; how the interface is contributed; what went wrong; one
lesson for us.

### 1.1 Obsidian: files are the database, plugins are fully trusted

- **Core owns:** the vault (a folder of files) and the app shell. Installed plugins live
  in the vault and are ignored in Restricted Mode [V]
  ([Plugin security](https://help.obsidian.md/plugin-security)).
- **Plugin can:** read files, connect to the internet, install other programs [V]
  (same page).
- **Plugin cannot:** nothing is enforced. The vendor says "Obsidian cannot reliably
  restrict plugins to specific permissions or access levels" and plugins inherit its
  access [V] (same page).
- **Sandbox and grants:** one switch. Restricted Mode (default) runs no third-party code;
  turning it off trusts every plugin [V] (same page).
- **Talk to each other:** not covered by the security page. I did not verify it.
- **Interface:** not verified from the pages I fetched.
- **What went wrong:** the vendor itself advises "an independent security audit" before
  using a plugin with sensitive data, and relies on automatic scans of every version plus
  manual review of popular or flagged plugins [V] (same page).
- **Lesson:** files as the database gives a great ecosystem, and also a plugin that can
  do anything. Our files stay strictly files, but only the host writes them. Review is a
  second layer, never the first.

### 1.2 VS Code: extension host, contribution points, activation events, proposed API

- **Core owns:** the editor, the UI and the extension host process. Extensions run in a
  separate extension host, which "prevents extensions from impacting startup performance,
  slowing UI operations, modifying the UI" [V]
  ([Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host)).
  The protection described is about performance and UI, not about secrets [I].
- **Plugin can:** declare static **contribution points** in `package.json` to extend the
  editor, and call the API [V]
  ([Extension anatomy](https://code.visualstudio.com/api/get-started/extension-anatomy)).
  Extensions load lazily on **activation events** such as opening a language or running a
  command [V] ([Activation events](https://code.visualstudio.com/api/references/activation-events)).
- **Plugin cannot:** change the UI directly; it goes through contributions and the API [V]
  (Extension Host page above).
- **Sandbox and grants:** the editor window is sandboxed but "the extension host is not
  inside that sandbox"; Node.js stays fully supported there [R]
  ([Stack Overflow quoting the VS Code sandbox blog](https://stackoverflow.com/questions/67493012/how-safe-are-extensions-in-visual-studio-code)).
  The one grant is **Workspace Trust**: an extension declares `supported: true | false |
  limited` with a written reason, and in an untrusted workspace the default is
  disabled [V] ([Workspace Trust guide](https://code.visualstudio.com/api/extension-guides/workspace-trust)).
- **Talk to each other:** not covered in the pages I fetched.
- **Interface:** declared statically (contribution points), drawn by the editor [V].
- **Stable versus proposed API:** a proposed API is unstable, Insiders-only and "should
  not be used in published extensions", so the stable API can stay small and
  compatible [V] ([Using proposed API](https://code.visualstudio.com/api/advanced-topics/using-proposed-api)).
- **What went wrong:** researchers report that "all an adversary needs to host a
  malicious extension is a valid Microsoft account" to publish to the marketplace [R]
  ([Control Plane](https://control-plane.io/blog/abusing-vscode-from-malicious-extensions-to-stolen-credentials-part-1)).
  Credential theft via extensions is reported [R] (Stack Overflow link above).
- **Lesson:** contribution points plus lazy activation make a rich, fast extension model.
  Our "call, time, change" triggers are the same idea. The missing piece in VS Code is a
  broker: once the extension runs, it is trusted. We add the broker.

### 1.3 bb (this tool): contracts with operations, events, slots; full trust

I read the plugin guide and skill on this machine.

- **Core owns:** a kernel with "no private API"; built-in plugins use exactly the same
  verbs [V] (bb plugin API guide, rule 6, at `/tmp/calternal-bb-lite-plugin-api.md`).
  `bb guide plugins` states: "Plugins are full-trust code" [V].
- **Plugin can:**
  - declare a **contract**: `op` (call), `event`, `live` value, `slot`, `service`, `table`;
  - **implement** an operation (one implementation), **call** any operation (attributed
    to the calling plugin), wrap another plugin's operation with `on(op, hook)` (a hook
    may `observe`, `rewrite` or `answer`, and `.failClosed()` makes a crash refuse),
    fill a **slot** with `add`, change what others added with `wrap`;
  - keep private storage (`kv`, a private SQLite file), and read a **shared** database
    including tables other plugins publish [V] (guide, "What every part has" and
    "Storage").
- **Plugin cannot:** reach another plugin except through its contract [V]. Operations can
  be limited by `allow` (`member`, `user`, `owner`); write operations are audited; secret
  input fields are kept out of the audit log [V].
- **Sandbox and grants:** none in the kernel; the guide describes trust, not isolation.
  Install asks for confirmation; safe mode stops all installed plugins [V]
  (`bb guide plugins`).
- **Talk to each other:** `bb.call` over one table of operations; events with an optional
  per-user audience; a slot is declared by the plugin that reads it [V].
- **Interface:** every slot is filled with `bb.add`; an entry's component is `view`; a
  crash in one entry is contained by a mount wrapper [V] (guide, "Slots").
- **What went wrong:** I found no incident report in my sources. By design, `bb.db`
  lets any plugin read tables others published, and `rewrite` hooks let one plugin
  change another's input. Both are fine under full trust and unacceptable under ours [I].
- **Lesson:** bb's shape is very close to our seven primitives (one call table, events,
  slots, one validator for contracts). It proves the shape is learnable. What we add is
  enforcement: one check at the call table, no shared tables, no rewrite hooks.

### 1.4 Figma plugins: sealed sandbox plus a UI frame

- **Core owns:** the document ("scene"). Plugin code runs on the main thread in "a
  minimal JavaScript environment" with no browser APIs: no `fetch`, no DOM, no
  timers [V] ([How plugins run](https://www.figma.com/plugin-docs/how-plugins-run/)).
- **Plugin can:** read and write the scene; show an `<iframe>` UI that has browser APIs
  but cannot see the scene; the two talk only by **message passing** [V] (same page).
- **Plugin cannot:** reach the network from the sandbox; if the manifest limits network
  access, a request to an undeclared domain gets a CSP error [V] (same page).
- **Lifecycle:** the plugin must call `closePlugin()`; otherwise a "Running" notice stays
  and the user can cancel at any time [V] (same page).
- **Talk to each other:** not covered; plugins are one at a time [I].
- **What went wrong:** not found in my sources. The page itself notes that network limits
  apply to the plugin's own requests, not to what a website in its iframe loads [V], so
  the frame is a leak path if it is given a wide domain list.
- **Lesson:** this is the model for our sealed frame: two halves, one with data and no
  network, one with a screen and no data, joined by messages. Make network for the frame
  none by default.

### 1.5 Raycast extensions

- **Core owns:** the launcher, a managed Node.js runtime, and an RPC protocol "that only
  exposes a defined set of APIs" [V]
  ([Security](https://developers.raycast.com/information/security)).
- **Plugin can:** run in its own V8 isolate (worker thread) with limited heap [V]; store
  secrets and data through APIs that keep them readable only by that extension [V] (same
  page). It is started by a command, an alias, another command (`launchCommand`), a
  background refresh or a saved draft [V]
  ([Lifecycle](https://developers.raycast.com/information/lifecycle)).
- **Plugin cannot:** use more than the exposed Raycast API for Raycast operations. But
  extensions are "not further sandboxed" for file I/O and networking, only the macOS
  permission prompts apply [V] (Security page).
- **Review:** all extensions are open source and reviewed before merge, then validated by
  CI; automatic updates are treated as a security feature [V] (Security page).
- **Interface:** React components from a closed set (`Detail`, `List` and so on) or a
  no-view command [V] (Lifecycle page).
- **What went wrong:** not found in my sources; the page admits the sandbox "might change
  in the future".
- **Lesson:** a closed component set plus one RPC channel is exactly our "catalogue
  plus calls". The weak part is file and network access. Source review is the control
  there, and review does not scale to strangers.

### 1.5b What Raycast and VS Code share (and we copy)

Both start extension code only when something happens (a command, an event) and both draw
the interface from declarations. This keeps idle cost near zero [V]
(pages above). It matches our three triggers.

### 1.6 Home Assistant integrations

- **Core owns:** the instance object `hass`: an event bus, a state machine, an action
  (service) registry, config entries, auth and a shared data dictionary [V]
  ([hass object](https://developers.home-assistant.io/docs/dev_101_index)).
- **Plugin can:** declare a manifest with `requirements`, `dependencies`,
  `after_dependencies`, `config_flow`, `quality_scale` and an owner [V]
  ([Integration manifest](https://developers.home-assistant.io/docs/creating_integration_manifest)).
  `dependencies` must load first. `after_dependencies` are "can use but does not need";
  if configured they load first, otherwise the integration still loads [V] (same page).
- **Talk to each other:** the event bus and registered actions [V] (hass page). A shared
  mutable `hass.data` dictionary exists [V] (hass page).
- **Sandbox:** none is described in the pages I fetched; integrations run in the core
  process on one event loop [V] (hass page: "`hass.loop`: the event loop everything runs
  on").
- **What went wrong:** not verified from my sources. A shared dictionary and one loop mean
  one broken integration can slow or corrupt the others [I].
- **Lesson:** two dependency kinds (must and may) are exactly "a missing part falls back
  or shows an error". A quality scale with tiers is a cheap way to tell people which
  plugins to trust. Do not copy the shared bag.

### 1.7 Deno and WASI: capability models

- **Deno:** a program has no access to files, network, environment, subprocess or FFI
  unless granted; grants can be scoped (`--allow-read=./data`,
  `--allow-net=example.com`); a deny flag beats an allow flag [V]
  ([Deno permissions](https://github.com/denoland/docs/blob/main/runtime/reference/permissions.md)).
- **What Deno cannot sandbox:** `--allow-run` lets the program spawn subprocesses that
  run outside the sandbox, and `--allow-all` "disables the security sandbox entirely" [V]
  ([Deno security](https://github.com/denoland/docs/blob/main/runtime/fundamentals/security.md),
  section "Permissions that bypass the sandbox").
- **WASI (wasmtime):** by default a guest has no filesystem; the host maps a "preopened
  directory" to a guest path with directory and file permissions [V]
  ([wasmtime-wasi `preopened_dir`](https://docs.rs/wasmtime-wasi/latest/src/wasmtime_wasi/ctx.rs.html)).
  The guest cannot name anything it was not handed [I].
- **Talk to each other:** not part of either model; they are about one program and its
  host [I].
- **What went wrong:** the escape hatches above. The more convenient the "run anything"
  grant, the more it is used [I].
- **Lesson:** grants must be scoped (a path, a host), deny-by-default, and there must be
  no grant that means "everything". Our "machine run" must be started by the host with
  mounts, never by a plugin spawning a process.

### 1.8 Android: intents, content providers, runtime permissions

- **Core owns:** the system, which routes **intents** between components and enforces
  permissions.
- **Implicit intents:** if an app does not name the target component or package, "a
  malicious application can register an intent filter to intercept the intent" [V]
  ([Implicit intent hijacking](https://developer.android.com/privacy-and-security/risks/implicit-intent-hijacking)).
- **PendingIntent (a delegated right):** app A hands app B a token to run a predefined
  action "on behalf of" A. A mutable one lets B fill in the unfilled fields and reach
  components that were not exported. The fix is `FLAG_IMMUTABLE` and setting action,
  component and package [V]
  ([Pending intents](https://developer.android.com/privacy-and-security/risks/pending-intent)).
  This is the confused-deputy problem in the vendor's own words [I: the page does not
  use that term].
- **Content providers:** a provider with no declared permission is closed to others
  unless "exported"; its own components always have full access regardless of declared
  permissions [V]
  ([Content provider basics](https://developer.android.com/guide/topics/providers/content-provider-basics)).
- **Runtime permissions:** apps request permissions at runtime; the page index lists
  "Auto-reset permissions of unused apps" [V]
  ([Request runtime permissions](https://developer.android.com/training/permissions/requesting),
  see its index entry "Auto-reset permissions of unused apps").
- **What went wrong:** the two risk pages above are the vendor's own catalogue of
  real failures: hijacked implicit intents and mutable delegated tokens [V].
- **Lesson:** (a) calls must name the callee, never "whoever answers"; (b) a delegated
  right must be fixed when issued; (c) a store of data needs its own permission check,
  not "the owner's code is trusted"; (d) permissions that are not used should expire.

### 1.9 Plan 9 and 9P: everything is a file, the only interface

- **Core owns:** a tiny protocol, 9P, between a client and a server of a **file tree**.
  The server answers requests to navigate the hierarchy and create, remove, read and
  write files; each request has a tag and each reply is either the matching reply or an
  error with a string [V]
  ([9P intro](https://9p.io/magic/man2html/5/intro)).
- **Extensions:** any program can serve a file tree. "Each client is given a private
  name space" with its own set of files, so what a client can name is what it was
  given [V] ([Plan 9 design](https://9p.io/sys/doc/9.html)).
- **Plugin can / cannot:** it can do whatever the files it can name allow; it cannot
  name anything else [I].
- **Interface:** the same: windows, devices and services are files [V] (design page).
- **What went wrong:** not covered by the sources; Plan 9 did not become a mass
  ecosystem [I]. The small message set (about a dozen request and reply pairs, as I count them) is the point [I].
- **Lesson:** a per-plugin **name space** is a stronger idea than a path filter: the
  plugin simply cannot name what it was not given. A small message set is auditable.

### 1.10 Solid pods, Anytype, Notion: data owned by the person, typed objects, views

- **Solid:** a "pod" is storage the person controls; a **WebID** names "an agent, for
  example a person, organisation, or software"; access control is an auxiliary resource
  (`acl`) next to each resource [V]
  ([Solid Protocol](https://solidproject.org/TR/protocol)). The protocol also says that
  browser cross-origin protection is "an obstacle rather than a feature" because access
  control is in the storage [V] (same page). So the server must authorize the app and
  the person together, or a stolen app acts as the person [I].
- **Anytype:** every Object has one **Type**; a Type defines a fixed set of
  properties (a Task has due date, priority, status); you can add custom Types [V]
  ([Types](https://doc.anytype.io/anytype-docs/getting-started/types)). This is our "kind".
- **Notion:** a database contains data sources and views; a data source holds the schema
  and is the parent of its pages; a table view shows a property as a column and a page
  as a row [V]
  ([Working with databases](https://developers.notion.com/docs/working-with-databases)).
  So a "database" is a set of typed pages plus saved views [I].
- **What went wrong:** nothing in my sources. Notion's API splits one idea into three
  objects (database, data source, page), which is more to learn than "kind plus
  collection" [I].
- **Lesson:** typed objects plus saved views is a proven way to get lists, boards and
  calendars from one store. Keep the view as data (a collection file) and let it grant
  nothing. Authorize the app and the person together.

### 1.11 Slack and Discord apps: consent to read messages

- **Slack:** an app "has no access by default"; **scopes** are chosen at install via
  OAuth; bot scopes act with the bot's identity, **user scopes** act "on behalf of the
  user ... accessing only what that user can" [V]
  ([Scopes](https://api.slack.com/scopes)). Reading message history is four
  separate scopes by conversation kind: `channels:history`, `groups:history`,
  `im:history`, `mpim:history` [V] (same page).
- **Interface:** not covered in the pages I fetched.
- **What went wrong:** not found in my sources.
- **Discord:** I did not fetch Discord documents; no claim is made.
- **Lesson:** "acts only as the person" is our intersection rule. Consent by kind of
  conversation is coarser than ours (per author); ours is stricter and the host draws the
  prompt, not the app.

### 1.12 macOS App Sandbox and XPC

- **App Sandbox:** limits an app's access to resources "requested through entitlements"
  (network in or out, camera, contacts, file access) to "contain damage if an app becomes
  compromised" [V]
  ([App Sandbox](https://developer.apple.com/documentation/security/app-sandbox)).
- **XPC services:** small helper processes managed by `launchd`; launched on demand, shut
  down when idle, restarted on crash; the stated benefits include "privilege isolation to
  narrow the scope of access for different functionality". An XPC service runs one process
  per client, tied to that client's life [V]
  ([XPC](https://developer.apple.com/documentation/xpc)).
- **What went wrong:** not covered in the sources.
- **Lesson:** split by privilege: the part that needs the network or the login is a
  separate process with its own entitlements. Our "machine run" and "the outside" should
  be separate, narrow processes, started on demand and ended with their client.

### 1.13 Browser extensions, Manifest V3: what removing power cost

- **What changed:** background pages became service workers that "run only when needed";
  remotely hosted code is removed; blocking `webRequest` is deprecated in favour of
  declarative rules (`declarativeNetRequest`) [V]
  ([What is Manifest V3](https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3)).
  Chrome says this needs "rewrite your code in terms of use cases", and policy-installed
  extensions keep `webRequestBlocking` [V]
  ([Blocking web requests](https://developer.chrome.com/docs/extensions/develop/migrate/blocking-web-requests)).
- **What it cost:** critics (EFF) argue the change limits ad-blocking and does little for
  security, and quote a Firefox official saying malicious add-ons were "at a manageable
  level" [R] ([EFF](https://www.eff.org/deeplinks/2021/12/chrome-users-beware-manifest-v3-deceitful-and-threatening)).
  Chrome's own stated reasons are performance and privacy cost of proxying all traffic [V]
  (blocking page).
- **Lesson:** declarative rules are safer and faster, and they remove expressiveness that
  some of the most loved extensions used. For us: declare first (queries, declarative
  operations), but never delete the code path once plugins depend on it.

## 2. Patterns table

### 2.1 What recurs in systems people love to extend

| Pattern | Seen in | Source |
| --- | --- | --- |
| Extensions declared in a static manifest (contributions, scopes, requirements) | VS Code, Raycast, HA, Slack, Android | [VS Code anatomy](https://code.visualstudio.com/api/get-started/extension-anatomy), [HA manifest](https://developers.home-assistant.io/docs/creating_integration_manifest), [Slack scopes](https://api.slack.com/scopes) |
| Code starts only when something happens | VS Code, Raycast, MV3 | [Activation events](https://code.visualstudio.com/api/references/activation-events), [Lifecycle](https://developers.raycast.com/information/lifecycle), [MV3](https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3) |
| One RPC channel with a defined API set | Raycast, Figma, bb | [Raycast](https://developers.raycast.com/information/security), [Figma](https://www.figma.com/plugin-docs/how-plugins-run/), bb guide |
| A closed component set draws the UI | Raycast, bb slots, Figma frame | [Raycast](https://developers.raycast.com/information/lifecycle), bb guide |
| Core uses the same API as everyone | bb ("no private API"), VS Code built-ins [I] | bb guide |
| Optional versus required dependencies | Home Assistant | [HA manifest](https://developers.home-assistant.io/docs/creating_integration_manifest) |
| Typed objects, saved views | Anytype, Notion, Obsidian Bases [I] | [Anytype](https://doc.anytype.io/anytype-docs/getting-started/types), [Notion](https://developers.notion.com/docs/working-with-databases) |
| Stable core API, unstable tier kept apart | VS Code proposed API, bb `experimental_` | [Proposed API](https://code.visualstudio.com/api/advanced-topics/using-proposed-api), bb plugin-authoring skill |
| Data stays as plain files the person can read | Obsidian, Plan 9, Solid | [Obsidian](https://help.obsidian.md/plugin-security), [9P](https://9p.io/magic/man2html/5/intro), [Solid](https://solidproject.org/TR/protocol) |
| Automatic scan plus human review for popular plugins | Obsidian, Raycast | [Obsidian](https://help.obsidian.md/plugin-security), [Raycast](https://developers.raycast.com/information/security) |

### 2.2 What caused security incidents or ecosystem death

| Pattern | Seen in | Source |
| --- | --- | --- |
| Full trust, no enforced limits | Obsidian, VS Code extension host, Raycast files and network | [Obsidian](https://help.obsidian.md/plugin-security), [Stack Overflow](https://stackoverflow.com/questions/67493012/how-safe-are-extensions-in-visual-studio-code), [Raycast](https://developers.raycast.com/information/security) |
| Publishing needs only an account | VS Code marketplace | [Control Plane](https://control-plane.io/blog/abusing-vscode-from-malicious-extensions-to-stolen-credentials-part-1) |
| A grant that escapes the sandbox (run, all) | Deno | [Deno security](https://github.com/denoland/docs/blob/main/runtime/fundamentals/security.md) |
| Calls aimed at "whoever answers" | Android implicit intents | [Android](https://developer.android.com/privacy-and-security/risks/implicit-intent-hijacking) |
| Delegated rights the receiver can change | Android mutable PendingIntent | [Android](https://developer.android.com/privacy-and-security/risks/pending-intent) |
| Shared mutable state across extensions | Home Assistant `hass.data`, bb shared tables | [hass](https://developers.home-assistant.io/docs/dev_101_index), bb guide |
| Removing a power hook with no equal | Chrome MV3 (contested) | [MV3](https://developer.chrome.com/docs/extensions/develop/migrate/blocking-web-requests), [EFF](https://www.eff.org/deeplinks/2021/12/chrome-users-beware-manifest-v3-deceitful-and-threatening) |
| A frame with a wide network list leaks | Figma note on iframes | [Figma](https://www.figma.com/plugin-docs/how-plugins-run/) |

Ecosystem death: I found no source in this research that documents a plugin ecosystem
dying from over-restriction beyond the MV3 dispute. I do not claim one.

## 3. How the prior art maps to our seven primitives

| Our primitive | Closest prior art | What to copy | What to avoid |
| --- | --- | --- | --- |
| 1 Files | Obsidian, Plan 9, Solid | Plain files; a per-plugin name space | A plugin that writes files directly |
| 2 Questions | Notion, Anytype, Android providers | Typed kinds, saved views | A provider "owner" that bypasses checks (Android: own components always have full access) |
| 3 People and permissions | Slack scopes, Android runtime permissions | Install-time scopes, per-person consent, expiry of unused | Scopes that mean "everything" (Deno `-A`) |
| 4 Running code | Deno, wasmtime, Figma, XPC, MV3 | No ambient authority, mounts, on-demand start | Any grant to spawn processes |
| 5 Calls | bb, Raycast RPC, Android intents | One table; explicit targets; fixed delegation | Implicit targets; mutable tokens; rewrite hooks |
| 6 Drawing | Figma frame, Raycast components | Closed catalogue; frame with no data and no network | Wide frame network lists |
| 7 The outside | Deno `--allow-net`, Figma `networkAccess`, XPC entitlements | Declared hosts; separate narrow process | Plugin code reading secrets |

## 4. Ten recommendations for our seven primitives

Each is tied to a source and to a primitive.

| # | Recommendation | Primitive | Source |
| --- | --- | --- | --- |
| R1 | **One check at the call table: caller's grant ∩ callee's grant ∩ the person's rights, for every hop.** A user-scoped call acts "only as that user". | 5, 3 | [Slack user scopes](https://api.slack.com/scopes), [Android pending intents](https://developer.android.com/privacy-and-security/risks/pending-intent) |
| R2 | **Calls name the callee (plugin id plus operation).** There is no "whoever handles X" resolution at call time. A pick between plugins is resolved by the host when the plugin set changes, and recorded in a file the person can read. A second plugin trying to register an existing operation name fails. | 5 | [Implicit intent hijacking](https://developer.android.com/privacy-and-security/risks/implicit-intent-hijacking), bb ("the operation's one implementation") |
| R3 | **A delegated right is fixed when issued.** A run's grant is a value stored with the work unit. The run cannot widen it, and the host never accepts "fill in" fields from the receiver. | 4, 3 | [Android mutable PendingIntent](https://developer.android.com/privacy-and-security/risks/pending-intent) |
| R4 | **No right to start a process or load native code.** Machine runs are started by the host with mounts and a deadline; a plugin that asks for "run" gets a work unit, not a shell. | 4 | [Deno security](https://github.com/denoland/docs/blob/main/runtime/fundamentals/security.md), [wasmtime preopens](https://docs.rs/wasmtime-wasi/latest/src/wasmtime_wasi/ctx.rs.html) |
| R5 | **Grants are declared in the manifest, scoped (a kind, a folder, a host) and denied by default.** An update that adds a grant waits for approval; the old version keeps running. Grants unused for a long time lapse and are asked again. | 3, 7 | [Deno permissions](https://github.com/denoland/docs/blob/main/runtime/reference/permissions.md), [Android runtime permissions](https://developer.android.com/training/permissions/requesting), [VS Code Workspace Trust](https://code.visualstudio.com/api/extension-guides/workspace-trust) |
| R6 | **A plugin sees a private name space built from its grants, not the whole tree with a filter.** What it was not given has no name. | 1, 2 | [Plan 9 name spaces](https://9p.io/sys/doc/9.html) |
| R7 | **Code starts on three triggers (call, time, change) and nothing else; contributions (kinds, collections, components, slot entries) are static files, read without running plugin code.** | 4, 6 | [VS Code activation events and contribution points](https://code.visualstudio.com/api/references/activation-events), [MV3 service workers](https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3) |
| R8 | **Declarative first, one bounded code path always.** Operations like "create a file from these fields" need no code; anything else is a sandboxed worker. Never later remove the worker. Keep an `experimental` tier, off for published plugins. | 4, 5 | [MV3 and EFF critique](https://www.eff.org/deeplinks/2021/12/chrome-users-beware-manifest-v3-deceitful-and-threatening), [VS Code proposed API](https://code.visualstudio.com/api/advanced-topics/using-proposed-api) |
| R9 | **Official plugins use the same API; dependencies come in two kinds (required, optional with fallback); disabling a required part warns.** No shared mutable bag between plugins and no direct reads of another plugin's tables. | 3, 5, host apps | bb ("no private API"), [HA `after_dependencies` and `hass.data`](https://developers.home-assistant.io/docs/creating_integration_manifest) |
| R10 | **The frame has no network and no data except what a host call returns; consent for risky effects is drawn by the host, never by the plugin.** Bounds on every run: heap, time, calls and wake-ups; Stop always works. | 6, 7, 4 | [Figma sealed sandbox and `closePlugin`](https://www.figma.com/plugin-docs/how-plugins-run/), [Raycast limited heap](https://developers.raycast.com/information/security) |

## 5. Security: how prior-art attacks apply to us

The brief's section 3 (security) is covered by the table below instead of one per
primitive. Primitive owners own the detailed tables.

| Attack seen in prior art | Does it apply to us? | What stops it in this design | Not sure it holds |
| --- | --- | --- | --- |
| Plugin reads or writes outside its grant (Obsidian, Raycast) | Yes | R6 name space; check at commit | Whether every host method (index, change feed, export) applies the same check |
| Confused deputy through a call (Android) | Yes | R1, R2, R3 | A plugin A calling B with data A could read but the person cannot; the intersection handles it, but the *index* must be queried as the person, not as A |
| Call hijack by registering the same operation (Android implicit intents) | Yes | R2: names are plugin-qualified and unique | Slot "pick one" resolution (bb picks by tier, then later added) must not be a way to steal a role |
| Mutable delegation (Android) | Yes, if a run's grant can change | R3 | Machine-side runs offline: the deadline and the grant must travel with the run |
| Process or native-code escape (Deno `--allow-run`) | Yes | R4 | Agent harnesses are programs that run other programs; the mount list and no-network rule are the only containment |
| Wide frame network list (Figma) | Yes | R10 | Sealed-frame CSP is not something I could verify in our code |
| Account-only publishing (VS Code marketplace) | Yes if we have a marketplace | Operator-pinned install (question 3) | Open |
| Shared state between plugins (HA, bb) | Yes | R9, no shared tables | None found |
| Hook that rewrites another plugin's call (bb `rewrite`) | Only if we add hooks | Thesis has no hook primitive; keep it so | If hooks are added later |
| Plugin storms the host on change triggers | Yes [I] | Quotas on wake-ups (thesis) | The cost of the change feed is not measured |

## 6. Where the thesis is wrong or incomplete (my area)

The thesis (`docs/proposals/primitives-only-thesis-2026-11-21.md`) is close to what the
safe systems do. These are the gaps the prior art shows. I changed nothing in the thesis
file; these are proposals.

1. **Calls need named targets, not only a table.** The thesis says "one table from name
   to handler". Android's implicit-intent problem appears when a name can be claimed by
   more than one plugin. Add: a name is plugin-qualified, registered once, and a second
   registration fails (R2).
2. **Delegation is not described.** The thesis says a callee runs with an intersection.
   It does not say the intersection is fixed when a run starts, and travels with the run
   to a machine. Add R3.
3. **Updates and expiry are missing.** The thesis grants at install only. Add: an update
   that widens a grant waits for approval; unused grants lapse (R5).
4. **"The outside" is a family of calls, not an eighth kind of thing.** Network, secrets
   and notifications are host methods in the call table with a declared-destination
   grant. This agrees with the host-base draft. The thesis lists seven primitives; I would
   say six primitives plus a set of outside methods in the call table. This is a naming
   choice (question 6).
5. **The sealed frame needs a rule.** The thesis says a sealed frame covers what the
   catalogue cannot express. Figma shows the frame needs: no network by default, no
   direct data, messages only (R10).
6. **Dependency kinds.** The thesis says apps have declared dependencies and a missing
   part falls back. Home Assistant's two kinds (`dependencies`, `after_dependencies`) give
   a precise meaning to "falls back" versus "shows an error".
7. **No shared mutable state between plugins.** bb allows reads of other plugins' tables
   and HA has a shared dictionary. Say explicitly: no such thing; the index is the only
   shared read and it runs as the asker.
8. **The thesis lists no unstable tier.** An `experimental` tier lets the SDK grow without
   breaking published plugins (VS Code, bb).
9. **Per-author consent is stricter than the prior art.** Slack scopes by conversation
   kind. The thesis rule (a run for an agent may not read bytes whose author has not
   granted that plugin) is finer. Keep it. I could not find prior art that does the same;
   treat it as new and test it hard [I].

## 7. Questions for the owner

Most consequential first. Each stands on its own.

### Q1. May any plugin ever run outside the sandbox?

- **What it is:** Obsidian and VS Code run plugin code with the full rights of the
  program. Figma and Deno run it in a box that only has what it was handed.
- **Why now:** it decides the whole security story and the SDK. If one trusted tier
  exists, every "must not get weaker" rule has an exception, and each exception tends to
  grow.
- **Example:** `calternal-chat-claude` needs to run an agent harness. Is that a box with
  mounts, or trusted code on the server?
- **Options:** (a) Every plugin, official or not, always runs in the sandbox. Cost: more
  host work for each official plugin; some things need a host method written first.
  (b) An operator-marked trusted tier for official plugins. Cost: a second security story;
  a stolen or replaced official plugin gets full power. (c) A trusted tier for the
  operator only, for debugging. Cost: a switch that can be left on.
- **Recommendation:** (a). It matches your decision that first-party plugins are not
  special, and Obsidian and VS Code show where full trust ends ([Obsidian](https://help.obsidian.md/plugin-security),
  [Stack Overflow](https://stackoverflow.com/questions/67493012/how-safe-are-extensions-in-visual-studio-code)).

### Q2. What happens when a plugin update asks for more rights?

- **What it is:** a plugin version 2 may need a grant version 1 did not.
- **Why now:** it decides whether an update is safe to apply automatically. Raycast calls
  automatic updates a security feature; they are also how a good plugin turns bad.
- **Example:** a Notes plugin v2 asks to call the network for a new "web clipper".
- **Options:** (a) The old version keeps running until the space owner approves the new
  grants; no auto-apply. Cost: a person must approve, and old versions linger. (b) Auto
  update always, re-ask only for new grants after the fact. Cost: a window where the new
  code runs with rights the owner never saw. (c) Operator pins exact versions; nothing
  updates unless the operator moves the pin. Cost: slow security fixes.
- **Recommendation:** (a) for grants, with the operator able to choose (c). See
  [Raycast](https://developers.raycast.com/information/security) for the argument for
  quick updates; the grant check is what makes (a) safe.

### Q3. Who is allowed to publish and install plugins?

- **What it is:** where plugins come from and who vouches for them.
- **Why now:** it is the line between a private tool and a marketplace. You asked earlier
  (morning questions, item 13) about operator-approved replacement of bundled plugins;
  this is the same fork seen from outside.
- **Example:** a stranger uploads `calternal-notes-pro`.
- **Options:** (a) Operator-pinned only: the operator installs from a list and a signature
  is checked. Cost: no open marketplace. (b) Signed publishers with a review step, like
  Obsidian's automatic scan plus manual review of popular plugins ([Obsidian](https://help.obsidian.md/plugin-security)).
  Cost: someone runs the review. (c) Anyone with an account. Cost: the VS Code situation
  ([Control Plane](https://control-plane.io/blog/abusing-vscode-from-malicious-extensions-to-stolen-credentials-part-1)).
- **Recommendation:** (a) now, with the manifest and grants designed so (b) can follow.
  The sandbox contains a hostile plugin, but approval fatigue is real, so fewer plugins
  is safer.

### Q4. Does each person approve a plugin's use of their own data, even if the space owner approved the plugin?

- **What it is:** the space owner approves a plugin at install. Android and Slack also
  separate what the installer approves from what each user allows ([Android](https://developer.android.com/training/permissions/requesting),
  [Slack user scopes](https://api.slack.com/scopes)).
- **Why now:** it decides whether `People/<me>/` files and your own writing are open to a
  plugin the owner chose.
- **Example:** a space owner installs a summarizer agent; Lee never agreed to it reading
  Lee's private notes.
- **Options:** (a) Owner approval is a ceiling; each person's own data needs their own
  approval on first use, drawn by the host. Cost: one extra prompt per person per plugin.
  (b) Owner approval covers everything in the space. Cost: a plugin the owner chose can
  read everyone's private state.
- **Recommendation:** (a). It is the thesis rule for agents, applied to every plugin.

### Q5. Must a plugin list the other plugins it calls?

- **What it is:** Android makes an app name the exact component it calls, because open
  names get hijacked ([Android](https://developer.android.com/privacy-and-security/risks/implicit-intent-hijacking)).
- **Why now:** it decides whether a call by name can find any plugin, or only the
  listed ones.
- **Example:** `calternal-chat` calls `calternal-notes.create` to save a message as a note.
- **Options:** (a) The caller's manifest lists each callee operation it may call; others
  fail. Cost: a longer manifest, and a person sees which plugins talk to each other.
  (b) Any call that the person's rights allow. Cost: a plugin can probe and call anything
  that is installed.
- **Recommendation:** (a). It makes the "plugin calls plugin" decision visible at install.

### Q6. Six primitives or seven?

- **What it is:** Network, secrets and notifications could be listed as methods of the
  call table instead of a separate primitive. The host-base draft already does this.
- **Why now:** it changes what a plain app author learns and what the security review
  lists.
- **Example:** "send a push notification" is `host.push.send` in the call table with a
  grant naming the destination.
- **Options:** (a) Six primitives, with "outside" as call-table methods. Cost: a longer
  call table; the network rule has to be seen there. (b) Keep seven. Cost: one more thing
  to teach; two paths to check.
- **Recommendation:** (a). Fewer concepts, one check.

## 8. Simplification opportunities

- Merge "The outside" into the call table (question 6).
- Do not add hooks. One call table and a change feed cover what bb's `on(op, hook)` does
  for us, and avoid the rewrite power (bb guide, "What every part has").
- Do not copy Notion's three objects (database, data source, page): "kind plus collection"
  is enough.
- Two dependency kinds replace the separate "fallback" and "error" rules.
- Skip a custom permission-prompt UI per plugin: one host-drawn challenge shape serves
  consent, widened sharing and grant changes.

## Sources not read

I did not fetch Discord's documents, Home Assistant's custom-integration security notes,
the `hass` or Chrome `declarativeNetRequest` limits, or any Obsidian page other than
plugin security. Claims about them are marked [I] or omitted.
