# Plugin-delivered interface

2026-11-18 · Interface decision and implementation proposal · [Lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

Keep a protected renderer and mount points in the client. Package the visible
shell and orchestrator as ordinary plugins that use the same slots, tokens,
operations and confinement as community plugins. The owner chose stateful generated views for rich plugin interfaces on 2026-11-18.
Add sealed components where they can stay within a bounded rectangle. This
fits the decision against page-wide JavaScript in the [plugin API](plugin-api.md)
and preserves the recovery surface described in the [architecture](architecture-proposal.md).
Terms follow the [glossary](glossary.md). The foundation cannot be replaced or styled by any plugin. Sign-in, approvals,
secrets and recovery are outside plugin reach. The server lists fixed and package-owned slots. Client mounting follows the renderer plan.

## What a plugin can contribute

| Host slot | Proposed contribution | Host responsibility |
| --- | --- | --- |
| Sidebar section | Generated view or sealed section | Own navigation, selection and the single sidebar |
| Card/window | Generated view or sealed component | Own window placement, chrome and stable address |
| Composer row | Generated view or sealed accessory | Own the editor, send and offline queue |
| Message decoration | Generated view | Virtualize with the transcript and keep one focus order |
| Thread edge | Generated view or sealed rail panel | Own edge placement and resize bounds |
| Settings section | Generated view or sealed section | Keep trusted security forms in the protected host |
| Palette commands | Generated action invoking a declared operation | Own search, shortcut routing and confirmation |
| Toasts | Generated view | Own timing, accessible announcements and queue limits |

A contribution has a plugin-local ID, slot and renderer declaration. An active
plugin contributes within its authorized space. The client selects where a
contribution is relevant using the current address and context. An address
identifies the item rather than its current renderer. Contributions never
authorize an operation. A personal shell resolves contributions separately for
each authorized space, with no shared worker across those spaces.
[Addresses](addresses.md) · [Orchestrator decisions](orchestrator-proposal.md#decided-in-the-grill-2026-11-17)

Owned mounts use a qualified `{plugin_id, id}` target. The owner declares
visibility, renderer types and a maximum number of visible contributions.
The server resolves only active owners and contributors whose corresponding
capabilities the caller holds. Missing or incompatible targets stay dormant.
Policy and capacity changes are reflected in subsequent listings. The next
scoped reactive values contract must publish removals and updated resolved
contributions to the owner under the same space and capability checks.
See the [manifest contract](plugin-api.md#authoring-and-interface-prototype).

### App switcher entries

Put app declarations in a dedicated manifest section. An app introduces a
navigation identity and owns a route prefix. Slot contributions render
inside the selected app, and may come from other enabled packages. Each
app declares an ID, a required display name, an icon, and an internal route
prefix. The host uses the name as the accessible label in the icon-only
switcher. It normalizes local prefixes, rejects external or ambiguous
addresses and refuses overlaps with other apps or protected host routes.
A prefix grants no operation or data access.
[Addresses](addresses.md)

The host accepts an icon name from its own catalogue or the data for one
SVG path. Parse its path commands and numeric operands, including the
required initial move and the grammar for arc flags. Reject markup, extra
attributes, invalid command arity, non-finite or out-of-range coordinates,
and paths that exceed bounded input length or segment count.
The host draws the validated path in its own fixed view box and controls
color and sizing. CSS, scripts and resource references have no place in
the declaration. Set the exact validation bounds with the web and Swift
renderers before freezing the manifest shape.

The switcher appends enabled plugin apps after its built-in entries in
the current space. Every package uses the same declaration, visibility
and routing checks, regardless of publisher. If the active plugin's entry
is removed, the host falls back to Threads.

## Sealed components

Mount packaged assets in an isolated frame with an opaque origin, a restrictive
asset policy and no ambient app credentials, network access, popups, or top-level
navigation. Give each mount a dedicated message port. Bind it to the frame,
plugin, space and activation generation. The host parses bounded messages and
passes minimal context. It routes requested actions through normal dispatch and
rechecks authority. Cancel the port and discard stale messages on navigation,
disable or replacement. Asset serving, policy enforcement and resend/revocation
need isolation tests before enabling this renderer.

The host sends a versioned token snapshot with scheme, type scale, contrast,
reduced motion and material role when a frame mounts or the snapshot changes.
Frames load fonts separately. Serve only public font assets with explicit CORS
headers or deliver font bytes over the bridge. An opaque-origin frame falls
back to system fonts when CORS blocks its font, even if the surrounding page
loaded the same asset. The host still owns adaptive material.

**Placement rule.** A transparent frame can blur the host pixels behind it.
The UI lane reproduced this in Chromium and on iOS Safari in the simulator.
Mount every sealed frame inside a host-drawn opaque `Card` or `Window` surface.
Clip it to that surface and keep it clear of chrome and protected UI. An
opacity, token or theme change must preserve the opaque backing. A visible host
slot may contain a sealed frame only when the host can place this backing
under it. The frame can draw blur inside its own rectangle. An opaque origin
controls authority and leaves compositing to the browser.
The [isolation fixture](../tests/integration/sealed-frame-placement.test.mjs)
checks that changing pixels behind the card does not change the frame's
rendered pixels, and includes a transparent control that samples the backdrop.
Run the same assertion against the actual mount before enabling sealed frames.

The UI bench measured roughly 40 ms to mount one frame at 1x CPU, around
100 ms at 4x and about 0.5 MB of heap per frame. Mount on demand, cap visible
mounts and dispose hidden ones. Keep repeated rows and scrolling transcripts
as generated views. Measure first interaction and memory on target devices
before assigning live budgets. [UI host catalogue and browser measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/1e61298/docs/ui-host-catalogue.md)

The host controls focus entry and restoration and supplies accessible frame
names. The frame handles local Tab order and reports boundary focus movement.
Keystrokes inside the frame do not reach the host shortcut listener. Frame
cooperation is required to forward named actions, which the host validates
against context. Keep foundation shortcuts and recovery accessible from
host-owned surfaces. The host owns drag handles and resize gestures,
coalesces size updates and clamps requested bounds.
Use typed drag payloads with host-side drop authorization. DOM inspection and
styling outside the frame, cross-frame drag previews, capture of app keystrokes
and a frame overlay across the host chrome are outside this mechanism.

## Stateful generated views

The owner selected this model for rich plugin interfaces. A
plugin runs ordinary TypeScript in an isolated worker. It produces a typed
tree of host components, updates that tree via bounded patches and receives
typed events. The same component IDs, property types, accessibility rules and
actions serve both a static GenUI program and a live tree. Keep the current
GenUI source format. Add an independently versioned patch and event protocol
on top of the same component catalog and normalized tree. Reuse the existing
validation where the representations meet.

Shopify introduced this pattern as remote-ui and now develops it as Remote
DOM. Its receiver can render native components and Shopify reports hosts in
JavaScript, Kotlin and Swift. Figma's sandbox is a precedent for separating
plugin logic and editor authority, with a separate frame for custom HTML.
[Shopify engineering](https://shopify.engineering/remote-rendering-ui-extensibility) · [Remote DOM](https://github.com/Shopify/remote-dom) · [Figma runtime](https://developers.figma.com/docs/plugins/how-plugins-run/)

On the web use a worker bootstrapped from a separately isolated environment
with a restrictive resource policy, without app storage, credentials or raw
network access. A same-origin worker by itself can fetch resources. Verify
the isolation and quota boundary before running a package. A server worker can
supply a live tree through its existing container and broker when the logic
needs server authority. That path adds transport latency. On Swift, an
isolated JavaScriptCore instance with no ambient bridges is a candidate for
local UI logic. Apply memory and execution limits and verify termination.
The runtime talks through the same frozen event and operation bridge on each
client. Every action is rebound to the authenticated scope and current
activation. An event ID names a declared callback, with no authority attached.

The shell plugin returns a `Layout` tree containing `Region` nodes. The
current pane arrangement uses cards and `Slot` nodes in those regions. The
canvas uses a `Strip` of `Window` nodes with the same `Slot` body contract.
Those five components are new. Other catalogue additions register existing
host components. The guard for sign-in, approvals, secrets and recovery stays
under the protected mount host. [UI host catalogue and both layout trees](https://git.kayg.org/kayg/calternal-ai/src/commit/1e61298/docs/ui-host-catalogue.md)

The orchestrator places `Transcript`, `Composer` and `Editor` as one host
component each. Plugin patches provide transcript pages and appends, composer
chips and accessories, and editor document and configuration inputs. The
host keeps their existing virtual list, selection, markdown editing, IME,
attachments and send queue. A plugin receives bounded asynchronous semantic
events. Host components supply focus, accessibility, tokens, fonts and
adaptive material. Plugins supply labels for their composition.

The versioned patch and event protocol must enforce these rules alongside
existing GenUI component validation:

- Give nodes stable IDs. Batch bounded updates and replace or invalidate a
  tree when revisions diverge.
- For large lists, the host owns the virtual list and sends a visible
  `range {first, end}` event. Patch only the requested bounded rows page or
  its deltas. Preserve host scroll position and anchoring on page changes.
- Coalesce streaming text into ordered append deltas on a fixed cadence. The
  browser bench exercised 60 Hz. Keep the exact production cadence and limits
  in the protocol budget review.
- Keep typing, scroll, drag and resize in host components. Emit their semantic
  events or settled states, such as `sized {w}`, as asynchronous notifications.

Reserve a sealed frame for custom drawing, a third-party editor or HTML the
catalogue cannot express. Reuse the host components for forms, lists, charts
and chrome.

## The shell decision

| Option | What the shell plugin can do | Cost |
| --- | --- | --- |
| Same trust tier with page JavaScript | Any approved plugin could run in the app page and replace live layout or intercept input | Sign-in, grants, secrets and recovery need a separate trusted origin or native surface. Every same-page plugin can still read or alter other app UI, counterfeit content and act with exposed page state. An approval and checksum identify the code a person agreed to run. They provide no runtime confinement. This would reopen the decision against page-wide JavaScript. |
| Static generated layout with a protected mount host | The visible shell is replaceable through declarative layout, theme and slots. A package can contribute at any supported slot subject to scope and grants. | The foundation still owns drawing boundaries, window and focus policy, and recovery. If “shell” means that privileged mount layer, that layer is host code. Custom global effects need a reviewed host primitive. |
| Stateful generated layout with a protected mount host | Any package can compose and update the host tree, run sandboxed layout logic and receive semantic events through the same interfaces. | Requires a shared catalog, bounded patch protocol and platform runtimes. The renderer retains control over the protected surfaces and fast interaction paths. |

Stateful generated views are the selected model for replaceable shell and orchestrator behavior.
Static GenUI remains the smallest authoring path. Sealed frames cover the
custom visual cases. All three use the same install process and renderer
permissions for official and community packages. Keep recovery, app-level
shortcuts and protected prompts in the foundation. The canvas experiment comes first. Design layout primitives
that support both fixed panes and windows on a scrollable canvas. Wait for the
layout result before implementing the shell renderer.

bb accepts full-trust server plugins and supports page content scripts as well
as named slots. Its Calternal UI plugin uses a content script for the frame and
registers an overlay, settings and sidebar navigation through the slot API.
That explains how it achieves global styling and where Calternal's sealed
model needs explicit host primitives. [bb Plugin Guide](https://getbb.app/plugin-guide)

VS Code separates normal contribution points from iframe-style webviews.
Webviews use message passing and get theme classes and tokens. The extension
host is a separate concern from the webview. [Contribution points](https://code.visualstudio.com/api/references/contribution-points) · [Webview API](https://code.visualstudio.com/api/extension-guides/webview)

Obsidian disables community plugin code in Restricted Mode and asks users to
trust the authors they enable. Its workspace API lets a plugin register a view
inside the app. [Plugin security](https://help.obsidian.md/plugin-security) · [Views](https://docs.obsidian.md/Plugins/User+interface/Views)

Figma runs plugin main code in a sandbox with editor APIs and uses an iframe
for browser UI. The two sides communicate by message passing. This supports the
separation between UI and document authority. [How Figma plugins run](https://developers.figma.com/docs/plugins/how-plugins-run/)

## Transport measurement

A headless Node worker prototype ran on `calternal-perf-02-backup01`
(x86_64, Node v22.23.3) under `/usr/local/bin/perf-lock.sh`. The timer
includes worker computation, structured clone and applying the update to
host state. The process and caches were warm. It excludes browser and Swift
rendering, sandbox bootstrap, GenUI validation, layout and frame time.

| Update | Samples | p50 | p95 |
| --- | ---: | ---: | ---: |
| Full 10,000-row data snapshot | 100 | 7.540 ms | 8.275 ms |
| 60 visible rows from a 10,000-row dataset | 1,000 | 0.079 ms | 0.118 ms |
| 256-character append to one streaming message | 1,000 | 0.034 ms | 0.070 ms |

The full snapshot nearly consumes one 120 Hz frame period at p95 before
rendering. The browser bench used the real host components in Chromium on
dev-02 and compared worker and direct updates. Applying a patch took
0.0–0.1 ms p50 and up to 1.9 ms p95 in the summary at 4x CPU throttling.
The detailed 4x window insertion and removal runs ranged up to 3.1 ms p95.
The worker and direct cases were comparable. The 10,000-row test sent
visible ranges, and streaming appended text at 60 Hz. At 4x, layout-heavy
list measuring and repeated resize dropped frames. Streaming and window
insertion stayed within the frame cadence. These dev-VM observations guide
the protocol. The locked Node results above measure transport only.
[Transport benchmark](../scripts/bench-plugin-view-transport.mjs) · [Browser benchmark](https://git.kayg.org/kayg/calternal-ai/src/commit/1e61298/docs/ui-host-catalogue.md#3-what-a-tree-update-costs-in-the-browser-measured)

### Warm browser-to-worker operation

On locked `calternal-perf-01-backup01`, headless Chromium 153.0.8010.12 called
`notes-insights.summarize` through the server and an enrolled operator
pool runner using rootless Podman. The browser, server and runner were
colocated. The operation reads a three-word note through the real
broker and returns a generated view. Timing begins before the browser
builds the request and ends after it parses the response. Ten browser
calls warmed the path before 100 serial measured calls. The prebuilt
binary was from the live-reload checkpoint (`95e3865`).

| Warm path | Samples | p50 | p95 |
| --- | ---: | ---: | ---: |
| Browser to server to enrolled worker and back | 100 | 49.1 ms | 50.1 ms |
| SDK fixture invocation | 500 | 0.135 ms | 0.199 ms |

The fixture baseline measures the SDK transport without HTTP, a machine
link or a real broker read. The browser measurement excludes page
rendering and uses loopback links, so it measures the application path
before adding network latency between machines.
[SDK fixture benchmark](reviews/plugin-dx-2026-11-18.md)

The follow-up on locked `calternal-perf-02-backup01` used the same operation
and colocated setup. A loopback HTTP relay timed both machine exchanges.
A pipe relay recorded when the runner delivered and received worker frames.
Both relays used the host's monotonic clock. Each column represents 100
serial calls after warm-up. Browser-edge measurements include the relay
overhead. Percentiles for individual hops do not add to the end-to-end percentile.

| Hop with tracing | Before p50 / p95 | After p50 / p95 |
| --- | ---: | ---: |
| Browser sends request | 0.59 / 1.26 ms | 0.56 / 1.29 ms |
| Server dispatch and sync hand-off | 1.67 / 6.26 ms | 1.60 / 3.39 ms |
| Runner delivers invocation to worker | 0.35 / 0.82 ms | 0.34 / 1.07 ms |
| Worker emits host call | 0.71 / 1.36 ms | 0.70 / 1.95 ms |
| Runner uploads host call | 21.54 / 22.94 ms | 0.21 / 0.50 ms |
| Server broker read and sync hand-off | 1.68 / 6.24 ms | 1.40 / 3.34 ms |
| Runner delivers broker reply to worker | 0.30 / 0.94 ms | 0.29 / 0.60 ms |
| Worker completes operation | 0.67 / 1.46 ms | 0.59 / 2.09 ms |
| Runner uploads result | 21.55 / 22.13 ms | 0.20 / 0.58 ms |
| Server rechecks authority and responds | 1.80 / 5.48 ms | 1.43 / 4.98 ms |
| Browser receives and parses response | 1.25 / 2.24 ms | 1.22 / 2.38 ms |
| End to end with tracing | 52.2 / 71.3 ms | 8.6 / 22.9 ms |
| End to end without relays | 49.3 / 63.6 ms | 5.6 / 10.1 ms |

The matched, untraced rerun on locked perf-01 measured 47.8 / 48.8 ms
before the change and 4.8 / 5.2 ms afterward.

The runner waited 20 ms before uploading each burst of exec output. The
worker pauses for the host reply, so the runner paid the timer twice per
operation. Together the uploads took 43.07 ms p50 before the change and
0.42 ms after it. Worker output uploads now start immediately and drain
already-queued frames in the same bounded post. Ordinary command output
keeps its timed batching. The comparison reused the checkpoint server
and plugin and changed only the runner's batching condition.

## Plugin worker placement and network access

The operator's pool runs shared-space plugin workers. Member-owned machines
are ineligible. Use the existing pool grant and fenced machine leases.
When the caller has no eligible container-capable pool machine, return a
caller-safe `unavailable` reason with the hint “Ask an operator to add a
container-capable pool machine.” Distinguish missing capacity from a busy
pool or a machine that needs to reconnect. The UI can point operators to
the pool enrollment steps. Keep machine identifiers and other accounts'
grants out of the caller's error. [Operator pool enrollment](../crates/server/README.md#machines)

Replace the enrolled-machine path's fixed eight-worker limit with the sum
of the authenticated `max_sandboxes` ceilings of eligible enrolled pool
machines. The runner's configured ceiling is the per-machine bound.
Use checked arithmetic. At each machine, subtract the greater of the
server's reservation count and the runner's authenticated occupied count.
Server reservations include other sandbox leases and stopping workers
without teardown proof. Clamp free slots at zero. Disconnection
or a lower report can suspend new admissions while existing reservations
remain fenced and accounted for. Release a stopping reservation on
authenticated teardown proof or durable revocation of its machine. A
revocation fences the session, executions and broker calls, removes the
machine's capacity from admission and releases its server reservations,
including stopping leases. Revocation does not attest that a disconnected
machine physically stopped its containers. Restart must reconcile these
states before reusing capacity.

A placement question remains. The host can reuse the negotiated backend and
first start result, or require a positive engine check cached per machine
session. A proactive check adds readiness traffic and detects a missing
engine before a plugin starts.

### Brokered network capabilities

Every worker has no direct network. Its manifest declares the request classes
it needs. Installation review shows one line per declared class with its
scope. Enabling the package grants only the classes approved for that space.
The broker checks the live space grant and revision declaration before
starting a request and before forwarding additional data on an open handle.
A widening update requires approval. Revocation closes or fences active
handles. The same declarations and checks apply to every publisher.

| Class | Capability and what the approver sees |
| --- | --- |
| A. Named connections (`network.named`) | “Connect to these services” shows purpose, fixed hosts, destination settings, protocols, ports and credential handles. A Manager approves the envelope once. People can add their own hosts within it. Widening the envelope requires another approval. |
| B. Content fetch (`network.content_fetch`) | “Load remote images and fonts referenced by this content” appears at install for any plugin that declares the capability. It shows the content kinds and host profile. Each person chooses Ask, Always or Never for fetches on their behalf. The content can reference arbitrary public hosts, so this class has no list of approved domains. |
| C. Open internet (`network.open_internet`) | “Make arbitrary HTTP requests to any public host. This can send space data off the server.” An Owner or a Manager approves this broad capability separately. |
| D. Private network (`network.private`) | “Reach private destinations from this server or machine” shows the originating network, approved destinations, ports and protocols. A Manager approves the plugin's per-space capability and the origin owner separately approves exposing that origin. |

These are proposed manifest IDs. Each class needs its own grant. A named
connection or content fetch cannot provide open internet access.

**Owner decision, 2026-11-18.** Community and official plugins can request
all four classes. The broker should support integrations whose destinations
are known at install, entered by a person, taken from content, or chosen at
runtime. Scale the review with reusable envelopes, explicit broad grants,
and destination selectors so a plugin can add services within its authority
without sending a Manager back through approval for each host. Enforce the
same network and disclosure checks regardless of who publishes the plugin.
These are design decisions. The broker has yet to be implemented.

#### A. Named connections

A public bank, broker, workout or provider API declares exact hosts and
protocols. For IMAP or SMTP, the manifest can identify a destination setting
and bound its protocol and ports. The grant binds the exact configured host
and port to the setting, package and space. A person may add or change a
host inside the Manager-approved envelope without another Manager approval.
The broker validates the entered host, port and protocol before use.
Endpoint discovery and redirects must stay within the approved service
envelope. Protected credential handles let the host attach credentials
without returning them to plugin code.

The host provides built-in mail handling with verified TLS and safe STARTTLS
transitions. Plugins can also request generic TCP to an approved host and
port. A broker-owned, expiring handle bounds reads and writes. The host owns
connect, backpressure, timeout and teardown. An opaque byte stream can send
arbitrary commands to the approved service, so its approval must describe
that power. Sending mail and making a trade remain separate effect
permissions.

#### B. Content fetch

Port the existing mail content proxy as the default profile. A plugin submits
content for host cleaning. It cannot pass a URL directly to the proxy. The host
stores the cleaned content and its generated references, then fetches eligible
assets by reference after checking the viewer and preference. This works for
HTML mail images and fonts, favicons, preview images and image or font feed
enclosures within the same profile. Fetching a linked HTML page or playing an
arbitrary audio enclosure requires a broader reviewed profile or class C.
[Mail architecture, remote content](https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/plans/mail-architecture.md#11-remote-content-and-network-safety)

- Use GET over HTTPS on port 443. Strip cookies, Referer and credential
  headers, use a generic User-Agent and never attach space data or a person's
  identity. Apply netguard on every hop, refuse IP-literal hosts in any
  spelling, and connect only to public addresses.
  Accept images and fonts by an allow-list, excluding SVG. Bound transfer to
  10 MB, redirects to three, connections to 5 seconds and total time to 10
  seconds. Start with two concurrent fetches per host and 16 globally, and
  enforce a per-space rate cap.
- Store bytes without host decoding. Serve from a per-person proxy URL that
  resolves only through that person's asset reference and rechecks access to
  the source content. Use `nosniff`, a strict CSP and a bounded per-person
  LRU, starting with the inherited 2 GB ceiling. Return a placeholder on
  policy, cache or fetch failure.
- Clean HTML at ingest with an allow-list. Rewrite or remove every URL
  position, including `srcset`, CSS URLs and imports, font declarations,
  SVG URL attributes and inline attachment references. Put the cleaned result
  in a sealed frame without scripts or same-origin access, inside the host's
  opaque card. Its CSP permits only authorized proxied assets. The person can still open an
  ordinary external link through a host-controlled navigation action.
- The ingest queue owns scheduled fetches. Opening the content shows cached
  assets or placeholders. An automatic fetch on open would give the sender
  the reading time. With Ask, wait for the person's
  decision about loading remote content. With Always, fetch at ingest for
  eligible items, dropping known tracker forms. With Never, show placeholders.
  Ask can disclose the time of the person's explicit approval. Backfill,
  out-of-window items and junk need the policy below.

URLs are a data channel. A plugin with access to space data could put text,
identifiers or encoded bytes in a URL inside the content it submits. The
remote server can see that URL and the proxy's source IP and fetch time.
The proxy accepts only host-issued references from stored, cleaned content,
caps URL length and fetch rates, and records the package, source reference
and destination hostname in the audit log. These limits make bulk transfer
harder and attributable. They cannot prove where a URL came from or keep
arbitrary encoded data out of it. Keeping B separate from C and requiring a
named capability makes this residual risk visible at review. Avoid putting
full URL paths and queries in logs.

#### C. Open internet

Use one broad grant when the plugin must construct arbitrary HTTP
requests to public destinations at runtime. Examples include reading a
person's subscriptions, scraping a page or calling an arbitrary API in an
automation. Require the Owner's or a Manager's affirmative approval of the
public-network and data-disclosure scope. Each request still passes netguard,
response and time bounds, and per-space rate and concurrency controls. Host
credential injection stays with a named service grant. Private destinations
need the separate private capability even when C is approved.

#### D. Private network and execution origin

Public egress originates at the server by default. A private grant identifies
the origin, plugin, space, protocol, ports and destination scope. Show whether
the operation can make changes where the host enforces that distinction.

| Origin | Approver and dialer |
| --- | --- |
| Server | A server administrator approves access because the connection exposes the operator's network. The server broker dials from that network. |
| Chosen enrolled machine | The machine owner approves their machine as the origin. An operator approves a pool machine. The server checks the invocation and sends the bounded request to that machine's broker, which validates and dials locally. The worker remains on eligible operator pool machines. |

A grant can name hosts with approved address scopes, CIDRs limited by protocol
and port, or “this space's machines.” The latter resolves to endpoints mapped
to current enrolled and space-authorized machines. Revocation removes an
enrollment from that scope immediately.
The broker checks whether the caller may disclose the input to the chosen
machine. The grant stays bound to its chosen origin and cannot fall back to
another network.

#### Shared guard, limits and inbound routes

Port the existing app's ADR 0071 `netguard` for every outbound connection.
Normalize names, reject ambiguous address representations, resolve once and
check every answer, then dial a checked address with the original TLS server
name and HTTP Host. Repeat for redirects, protocol discovery and reconnects.
Use the inherited checks for embedded IPv4 and public address classes.
The default rejects private destinations, and class B always does. Always
refuse cloud metadata, including aliases and mapped forms, even for an
approved private scope. Keep certificate validation mandatory.
[Calternal ADR 0071](https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/adr/0071-mail-architecture.md)
[Mail security summary](https://git.kayg.org/kayg/calternal/src/commit/ea57b84b9c9e40e8b67b7d1e1f1805915123b040/docs/plans/mail-architecture.md#19-security-summary)

Bound per-space request and connection rates, byte volumes and concurrency.
An idle open stream still consumes capacity. Audit the class, space, actor,
package revision, approval, origin, destination host and port, protocol,
outcome, elapsed time and byte counts. Audit grant creation, widening and
revocation. Keep a bounded retention window and omit credentials, headers,
URL paths and queries, request bodies and response content.

Inbound webhooks have a separate manifest declaration and review line.
The host binds a route to a space, enabled package and operation, verifies
the signature, and bounds request size, rate and replay handling. It
passes the authorized payload to the operation. The worker has no listener.
Registering a callback with a remote service or emitting a subsequent
request uses the appropriate outbound grant.

#### Integration walkthroughs

| Plugin | Classes and behavior |
| --- | --- |
| Mail | A connects to the IMAP, SMTP or JMAP endpoints entered in settings and uses brokered credentials. B renders remote images and fonts from cleaned mail through per-person proxy references without a host list. An internal provider also needs D from its chosen origin. Open internet access is unnecessary for those actions. |
| Money | A connects to approved bank and broker APIs. Reads and transactions use separate host permissions. An on-premises brokerage endpoint also needs D. |
| VPN manager | A reaches a named public NetBird or Tailscale management API. D reaches a controller on an overlay network from an origin that is already on that network. Reconfiguring an enrolled machine's own network requires a separate host operation. |
| Home automation bridge | D reaches an approved LAN controller from an enrolled machine on that LAN. A can cover its public cloud service. Discovery or multicast requires its own reviewed broker contract. |
| Webhook receiver | An inbound declaration owns the route, verification, limits and target operation. A can register the route with a named provider. Reply or follow-up calls need the appropriate outbound class. |
| Feed reader | C fetches subscriptions whose hosts are chosen at runtime. B can load supported feed artwork and media references from stored entries. Hosting over a private network additionally needs D. |
| Link-preview plugin | B fetches an image or favicon referenced by stored content. Producing a title or description by fetching arbitrary linked HTML needs C or a separately reviewed host-owned preview profile. Running on every pasted URL cannot silently broaden B. |
| Workout, orchestrator and shell plugins | A covers an approved fitness service. Orchestrator integrations declare each applicable network class. The shell uses host components and operations, and a mount point carries no network grant. |

These are design walkthroughs. Each needs a fixture before shipment.

#### Open choices

| Choice | Options and costs |
| --- | --- |
| Endpoint and credential ownership | Decide when a configured endpoint and its credential belong to the person or the space. Personal scope isolates connections and credentials. Shared scope supports shared workflows and needs visibility and revocation rules. |
| Content preference and backfill | Ask first exposes the choice early. Always retains the earlier mail default and sends public fetches automatically. Never starts with placeholders. The inherited ingest window uses 90 days or 5,000 threads and excludes junk. Decide whether to load older items only on explicit request or during bounded backfill. Explicit load can disclose reading time. |
| Content fetch profiles | Ship the existing image/font profile first. Review HTML metadata, audio and other enclosures independently. More types cover more view and feed cases and increase what the proxy must validate, cache and serve. |
| Private-origin payloads | Decide which payloads may be sent through a member-owned origin. Wider forwarding covers more local integrations and requires clear disclosure authority. |
| Budgets and visibility | Measure mail sync, finance polling and interactive loads to set per-space caps and retention. Choose how Managers and origin owners see the audit and test revocation with long-lived connections. |
| Inbound verification | Choose supported signature profiles and retry identity rules before exposing routes. A common profile gives one enforcement point. Provider adapters increase compatibility and review cost. |

The grant classes and approval flows above are the accepted design. Delivery
order, benchmark thresholds and the remaining operational details are open.

## Swift clients

Use the same contribution and operation identifiers, address and token
contract. Generated views render with platform controls after validation.
Provide a generated view for any sealed web contribution that must also work
on Swift clients. A separate native component protocol could add richer Apple
behavior later. It would require its own accessibility, versioning and safety
review. Web frames do not translate to AppKit or UIKit.

## Notes and Threads as ordinary plugins

The current broker has live-authorized single-item reads and writes. It
does not expose the Notes and Threads behavior currently wired into core
dispatch. Keep resource identity, journaling, receipts, recovery, membership,
search authorization and machine/provider enforcement in the host. Port from
the existing Calternal engines before exposing a new primitive.

| Missing public contract | Existing code to reuse | Estimated size |
| --- | --- | --- |
| Revision reads, lifecycle transitions and collection placement | Own-kind indexed listing now uses `list_items_v1`. Notes history, Trash and placement still use host paths. | 3–5 engineer-days including isolation fixtures for the remaining calls |
| Conditional patch and block-identity merge, revision conflicts and links/tags | Notes revision, merge and indexing paths | 4–7 days including recovery tests |
| Invalidation and incremental indexing, including search-provider registration | Space journal and Notes index | 2–4 days |
| Typed thread events, atomic append/checkpoint, paging and overflow attachments | Thread document, deltas, persistence and saved transcript | 4–7 days including crash/retry tests |
| Job admission, leases, typed provider output and durable user requests available to a plugin | Existing job/checkpoint code, turn admission, machine transports and requests | 5–9 days including revocation tests |
| Thin TypeScript operation and view port for both apps | SDK worker, existing manifest kinds and GenUI | 4–7 days after the contracts above |

These are implementation estimates for one engineer and exclude native UI work.
First expose versioned service calls over the existing engines, then move
product routing and composition to TypeScript. Direct userdata access would
duplicate the authorization and durability boundaries. Decide separately when
the public engine replacement contract should support alternative formats.

## Can official plugins use the same contract?

**Server today.** Notes Insights and the guide use the same manifest, worker
and broker path that a community package can use. Notes and Threads still
depend on native host handlers. The existing packages cannot run unchanged
as ordinary TypeScript packages. Official packages also seed the installation
catalog, which is a distribution default. Provider executables and images are
machine provisions. The provider manifest describes them and conveys neither
installation nor login authority. The plan is to expose versioned engine
operations behind the same public checks, then move Notes and Threads one
contract at a time. Notes listing and lifecycle go first. Threads wait for
the thread file work in #46.

The owner requested mechanical parity on 2026-11-18. Distribution and
publisher metadata may affect discoverability. Every grant and operation must
use the same rules and dispatch regardless of either field. Current native
exceptions become API requirements.

| Current exception | Contract needed to remove it |
| --- | --- |
| The server matches the orchestrator ID to native thread operations and native turn jobs | A public engine service selection contract with thread append/checkpoint and turn admission, lease, retry and request operations |
| The seed sets some packages enabled by default | A deployment profile selects default packages and activation policy by the same manifest rules available to any installed package |
| Notes calls its Rust store and lifecycle handlers directly | Public own-kind listing is available. Notes still needs lifecycle and data mutation contracts over the existing engine, with the same isolation and receipts. |
| The orchestrator's core skill is embedded by path and reused at startup as a named fallback | Declare packaged instruction and skill sources and distribute defaults through the package seed; keep host security instructions in the protected core |
| Server, CLI and browser recognize a provider package ID when resolving defaults and thread files | Resolve an enabled provider by configuration or service selection and carry provider metadata through the thread protocol |
| Browser defaults and palette entries name particular apps | Discover enabled apps and contributions from the manifest and active service slots |

At cutover, install each official package and a separately published package
with a different ID in parallel disposable installations. Use the same logical
operation, kind and slot names to check behavior. Separate installations avoid
global declaration collisions between the two packages. Assert the same
capability review, space grants, listing, worker outcomes and isolation
denials. A publisher field, if added, is provenance only. Add a source check
that derives the official IDs from the distribution seed and rejects their
use in host, broker, registry and client code. Public operation and kind IDs
remain valid. Run the source check and the renamed-package conformance suite
together, with no exception list for official code paths.

The current [conformance case](../tests/conformance/plugin_parity_cases.rs)
installs renamed copies of both seeded TypeScript worker packages in a separate
installation. It compares grants, slots, operation results and cross-space
denials. Manifest v1 has no publisher identity. A publisher substitution
test requires a provenance contract with no authority-bearing fields.
The strict [source check](../scripts/check-plugin-parity.py) detects the
identity references still present in the host and clients. The native app
and provider cuts above must land before it can pass.

**Interface today.** Every package can declare the same portable view slots.
The current web shell and orchestrator visuals are client code. They cannot
ship unchanged as ordinary plugin contributions. The host component catalog
must gain layout primitives, native editors, virtual transcript and composer
accessories before the visible shell and orchestrator fit. Protected forms
remain foundation features for everyone.

The implementation order is live install and reload, plugin-owned slots,
scoped reactive values, readiness and logs and timing, then durable waits and
continuations. Update the comparison after each step. In parallel with that
sequence, the versioned Notes engine contracts need the existing isolation,
recovery and performance gates. Measure plugin-run Notes save and list against
the existing budgets on perf-01. Report the boundary and host costs separately.

## Acceptance check for the author workflow

Run three independent GPT-6 Luna sessions chosen by the coordinator. Give
each the same version of the packaged authoring skill and one paragraph.
Start from a fresh project directory and one already-running disposable
development server per run. Provide connection details and preinstalled
tooling as environment, without recipes or additional instructions. Record
the server PID, starting space UUID, initial package catalog, command output
and full transcript. Use a separate passkey account without access for the
isolation checks.

Prepare the built SDK with `npm run build:plugins`. The disposable server's
binary needs the `dev-plugin-drafts` feature, `--development` and an enrolled
container-capable pool runner. Supply `CALTERNAL_SDK_ROOT`, `APPDATA`,
`SPACE_ID`, `ACCOUNT_ID` and the normal CLI connection for its Manager.
Offer the bundled plugin authoring skill and the request paragraph.

| Request paragraph | Observable success |
| --- | --- |
| Create a reading-list plugin with a readable Markdown kind and a generated list that shows the items in this space. | The package is active, the authored kind stores a fixture item, and the view operation renders that item. Another space cannot list it. |
| Create a checklist runner with a start operation that launches a durable job, streams its progress and returns a final count. | Starting through the ordinary operation path produces ordered progress and the final count. A foreign account cannot read the job. |
| Create a composer chip with a configurable label, exposed as a plugin setting. | The slot listing resolves the chip in the enabled space. Check the default label, set a distinct value as Manager and invoke the view again. An unrelated space sees no resolved chip. |

Each session must scaffold, implement, run check and tests, and live-load
its package. Invoke through the normal authenticated operations route.
Change one visible output, confirm the updated behavior, and check the same
server PID throughout. Record edit-to-success time from the final file write
through the first valid result. Require every step to succeed without
assistance. A failed check, misleading error or manual repair is a defect
to record against the guide, scaffold, diagnostic or API. The coordinator
runs the model trials after live reload and all three contracts are ready.

## Extension points in bb, bb-lite and pi-durable

The largest authoring gaps are **plugin-owned slots**, **live install and reload**,
**scoped reactive values**, **durable waits and continuations**, and **readiness,
logs and timing**. Together these would let a plugin compose with other plugins,
explain its own failure and resume unattended work. Prioritize the slot and
component contracts before adding individual React-style hooks. Prioritize
typed waits and run continuations before adding one-off scheduling or retry APIs.

The following inventory groups equivalent extension points across the sources.
A declaration without execution is marked as decided. Built includes the
prototype where explicitly indicated. Proposed additions require the owner's
decision. The [November 17 comparison](reviews/extensibility-research-2026-11-17.md)
contains the detailed constraints for the 24 previously adopted capabilities.

| Extension point in the reference systems | Calternal status | What an author can do there today |
| --- | --- | --- |
| bb-lite plugin-declared typed slots and single or multiple selection | Owner declarations, typed contributions, capacity and space-aware listing built | Have a slot owner receive changes through a live value and mount the result in the client. |
| bb and bb-lite local install, hot reload and development inspection | Live install and scoped draft reload built. The enrolled-machine author loop measured 903 ms p95. Development drafts can report the ready revision, phase timings and the caller's latest five errors per operation. | Installed-package inspection for production viewers is not built; it needs a new authorized operation and a retention decision. Candidate failures are reported in development. |
| bb marketplace and bb-lite Git/local catalogs, dependency and engine ranges, upgrade rollback | Decided, unbuilt | Find packages by source, have compatibility checked and restore the last working activation. Local checksum review is built in this lane. |
| bb-lite fork existing server, app or runner entry and profiles | Decided, unbuilt | Fork one part of a default feature and keep updating the other parts. |
| bb status, logs, lifecycle timings, safe mode and bb-lite audit chain | Decided, unbuilt | See why a plugin needs setup, view handler latency and disable scoped plugins together. |
| bb-lite live data, changed notifications and audience-scoped events | Decided, unbuilt | Publish a typed value to one account and refresh its subscribers without polling. Job streams are built. |
| pi-durable typed signals and deadlines, replayable parked tools | Durable job waits | Park a replayable job on a signed webhook, Inbox decision or deadline, then continue from its checkpoint. |
| pi-durable parked approval gates and argument-bound decisions | Job approvals and protected native turn approvals | Park a job for a one-time Inbox decision. Native turns use their existing argument-bound approval gates. |
| pi-durable `afterRun`, `afterFailedRun`, `onYield`, after-tools decisions; bb lifecycle retry | Decided, unbuilt | Schedule follow-up testing and withhold an answer until the child work completes. Calternal's core retries do not expose this as a plugin point. |
| bb workflow scripts and pi-durable Code Mode | Decided, unbuilt | Replay a deterministic tool pipeline without repeating successful calls. The reference Wasm runtime is outside the current platform rule. |
| bb lifecycle events for thread, turn, tools, environment, host and startup | Decided, unbuilt | Subscribe a plugin to changes and run scoped follow-up work without polling. |
| bb dispatch hook, bb-lite operation watch, rewrite and answer policies | Decided, unbuilt | Inspect or rewrite allowed operation inputs with an owner-declared ceiling and field redaction. Calternal accepts some metadata today. |
| bb schedules and cheap pre-checks, one-shot timers and long-lived background services | Decided, unbuilt | Trigger narrow condition checks and enqueue work only when it is needed. |
| bb and bb-lite app registration, home and replaceable navigation | Decided, unbuilt | Replace navigation, home panes or sidebar lists as a package. Stateful GenUI is the recommendation here. |
| bb sidebar footer, header, action menus, row glyphs, pin, drag/split and metadata | Decided, unbuilt | Put live usage, status and commands on individual rows and add split targets. |
| bb contextual fixed/dynamic panel tabs, file openers, Browser toolbar | Decided, unbuilt | Offer an editor panel or toolbar action relevant to the current tab and thread. |
| bb thread header, Info sections, thread context cards and stacked cards | Partly built as slot metadata | Register controls and context that update in place around an open thread. |
| bb and bb-lite message controls, directive renderers, file viewers, code fences and diffs | Decided, unbuilt | Take over one file, code block or message action with a specialized renderer. |
| bb-lite attributed timeline rows | Decided, unbuilt | Insert and update a plugin-owned progress row with one stable ID. |
| bb-lite composer contributions: menus, footer, send menu, banner, popup and text paint | Partly built as slot metadata | Add a conditional chip, send action, banner or syntax highlight around the editor. |
| bb mentions and composer command providers | Decided, unbuilt | Resolve a project document at send time and attach labelled context. |
| bb and bb-lite command shortcuts, palette entries, toasts and settings panels | Partly built as slot metadata | Bind a named shortcut and invoke it from the host command palette. |
| bb reusable app components and hooks for chat, source, diff, Info, model picker, icons, navigation and composer | Decided, unbuilt | Reuse production components with host focus, theme and state. Static GenUI has a smaller catalog. |
| bb browser extension state, query/mutation caching and tab-context hooks | Proposed | Bind a control to typed scoped host state and invalidate its query on change. |
| bb theme tokens, theme preview, code colors and fonts; bb-lite theme bundles and author reload | Decided, unbuilt as a plugin lifecycle | Test a complete theme, including native tokens and a code palette, before enabling it. |
| bb and bb-lite typed settings, pickers, scoped values and on-change handlers | Typed shared settings, worker reads and scoped journal invalidation built. Per-person pickers and subscriptions for slot owners remain unbuilt. | Declare a per-person selector or react to another plugin's contribution through a live subscription. |
| bb and bb-lite install/setup and teardown lifecycle | Decided, unbuilt for authored handlers | Seed package defaults during activation and dispose a watcher when disabled. |
| bb typed RPC and bb-lite operations, client/CLI/agent doors and breaking-change lock | Built for public operations | Expose one schema-checked operation. Typed cross-plugin RPC and consumer compatibility ranges remain unbuilt. |
| bb hand-written plugin CLI commands and bb-lite combined CLI routes | Proposed | Group several operations under a plugin-owned command with help and completion. Currently use `calternal invoke`. |
| bb tools, presentation and conditional offering; bb-lite agent doors; pi-durable caller restrictions, search and split result channels | Partly built | Offer a tailored tool to one agent and keep render-only output out of model context. Calternal already exposes authorized operations. |
| bb agent/provider environment entries; bb-lite runner parts; pi-durable per-conversation environment | Partly built | Supply a narrow executable or environment binding to one authorized machine-run command. Managed machine/provider invocation exists. |
| bb and bb-lite machine and environment providers, enrollment, checkpoint, suspend and reconcile | Partly built | Define a custom environment kind with its own provisioning, suspend and cleanup. Core machine enrollment exists. |
| bb and bb-lite agent provider adapters, capability/model catalog and native skills/commands | Built for the worker/provider protocol, partly unbuilt at discovery | Ship an agent backend, extend its native command and skill roots, and expose its configuration controls. |
| bb and bb-lite small completion, title fallback, usage providers and transcription | Decided, unbuilt | Register a provider for titles, quota feeds or voice without taking over the agent runtime. |
| bb skill layering and import, Honcho instruction contribution; bb-lite instruction sections; pi-durable wrapped system-prompt sections | Partly built | Add a selectable context source or skill with visible provenance. Space standing instructions and the guide asset exist. |
| bb and bb-lite trusted user forms and interactions, pi-durable ask-person example | Built for host forms, plugin entry unbuilt | Ask a typed question from an ordinary plugin and resume when the person answers. |
| bb operator-entered sandbox secrets and bb-lite runner credentials | Partly built | Bind an approved secret to a particular runner environment with no agent-visible value. Calternal has protected storage and brokered handles. |
| bb private stores and bb-lite KV; pi-durable document families, typed state and transactional entries | Partly built | Declare durable extension state and update it as part of a task transition. Calternal has brokered hidden kinds and revision writes. |
| bb-lite app and server contract calls, explicit caller chain | Decided, unbuilt in full | Compose operations across packages while preserving the acting caller and scope. |
| bb and bb-lite thread, project, terminal, file and Git SDK surfaces | Partly built as host operations | Build a plugin-powered tooling view over thread/workspace actions, Git status and a scoped terminal. The general worker broker is narrower. |
| bb and bb-lite OAuth integration, plugin routes and hooks; pi-durable signal ingress | Decided for brokered ingress, unbuilt | Receive an external system event with schema checks and wake only its owning job. |
| bb fixture APIs for app, host, providers and server; bb-lite contract harness; pi-durable scripted models | Partly built | Test UI, provider negotiation and scripted task recovery without the live app. This lane supplies a worker fixture. |
| pi-durable tool wrappers, nested calls, continuation, tool policy and abandonment | Decided, unbuilt as a public extension contract | Wrap a tool with approval and persist nested calls independently through retries. |
| pi-durable conversation configuration, named policies, context views, reset, handoff and compaction | Partly built as host/provider behavior | Install an extension that changes prompt assembly, compaction and handoff for one conversation. |
| pi-durable persistent child work, graph inspection, bounded retries and pinned registry reload | Partly built | Watch the complete child and task graph and keep its code pinned during live reload. Core jobs and durable checkpoints exist. |
| pi-durable conversation forks, transcript projections and watch subscriptions | Partly built | Define a fork or custom projection that updates as stored entries are added. Core thread feeds exist. |
| pi-durable usage, cost and live task telemetry | Proposed as scoped plugin data | Query accumulated model usage and live task state for an extension-owned diagnostic view. |
| bb full-trust server factories, host scripts and unrestricted process or network access | Excluded by worker isolation | Read ambient configuration, launch host programs, or fetch anywhere with server authority. |
| bb app content scripts, bb-lite same-page app parts and app-global overlays | Excluded by page isolation | Inject a stylesheet or change live DOM across other features and windows. |
| bb custom composer and bb-lite plug-in submit handler | Excluded for host-owned input and send | Read every keystroke and decide what a submit does. Chips and semantic requests fit stateful GenUI. |
| bb raw HTTP, WebSockets and broadcast realtime; bb-lite arbitrary server responses | Excluded on the app origin | Serve executable content beside protected sessions or broadcast tenant data to every client. Use scoped operations and subscriptions. |
| bb raw SQLite, bb-lite shared tables, pi-durable in-process storage replacement | Excluded by per-space access and protected recovery | Open another feature's tables or replace its durability substrate. Expose authorized service operations instead. |
| bb-lite pluggable authentication and authority | Excluded by protected identity | Replace the host authenticator. Approved sign-in integration belongs in protected host code. |
| pi-durable arbitrary in-process extensions and custom replay/storage adapters | Excluded for authored packages | Run beside the harness or replace how recovery commits. Versioned broker calls preserve the host's boundary. |

### Sources checked

- bb **0.49.0**, local `bb guide plugins` and authoring skill snapshot
  `478fd091c7850d45f0584dafd50035943b52e76f37d9cb6372bad547b6f1c0ae`.
  Checked the backend and frontend symbol indexes and registration,
  lifecycle, renderer, host, CLI, providers, RPC and slot references. Read
  Calternal UI and CLIProxy Usage app and server entries at local
  `bb-plugins` revision `0c485b1`, and the Honcho app, server and host
  entries at `145816f`. [Public Plugin Guide](https://getbb.app/plugin-guide)
- bb-lite public HEAD `19e1804266d476a89def44d07361e2f992cec4aa`.
  Checked the plugin guide and the server API and core contracts from the
  same commit. [Plugin API](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/docs/plugin-api.md) · [Server API source](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/packages/plugin/src/server-api.ts)
- pi-durable **1.5.2**, pi public HEAD
  `05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94`. Checked the current Durable
  README, package version and root export inventory against the earlier
  `747c217` reference and reviewed the earlier snapshot's example inventory.
  [Durable README](https://github.com/earendil-works/pi/blob/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/README.md) · [Public exports](https://github.com/earendil-works/pi/blob/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/src/index.ts) · [Examples](https://github.com/earendil-works/pi/tree/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/test/examples)
