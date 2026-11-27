# Format engine findings · 2026-11-27

The lane targets revision 9 from `dev` at `3b15b606` and the coordinator's
eleventh set. The public interface is in [the API sketch](format-engine-api-2026-11-27.md).
[Forgejo issue #113](https://git.kayg.org/kayg/calternal-ai/issues/113) was already
open for this branch. Issue updates now use the signed-in Forgejo CLI on hd01.

## Decisions and fixture changes

- **§1.2 and §4.2.** The same `<` character opens an angle group or compares
  two values. The line scanner recognizes an attached angle link target and
  the formula scanner reads comparison operators in expressions. Physical
  continuation positions are tracked in the line scanner.
- **§4.1 and §4.3, eleventh set.** Chat's unread formula should become
  `count(records after state.read through)`, and the state key should become
  `read through: record anchor`. Lane B supplies records in ledger order with
  host-owned anchors. The existing timestamp formula remains a fixture.
- **§2.20 and §7.4, eleventh set.** Chat's coded send needs typed inputs
  for the draft, chat reference and chosen harness. Claude Code's start
  needs a reference identifying the newly appended record. The place
  and caller both use the declared operation input schema. The process
  lane owns send-and-start sequencing and its request key.
- **§7.1, eleventh set.** Claude Code's program ID needs a binding under
  `programs:` once the adapter path is available in its plugin folder.
  The existing flat fixture declaration supplies an ID only.
- **§3.1 and §4.1.** The spike used binary floats. The engine needs base-ten
  arithmetic to preserve values such as 0.1 + 0.2. The implementation
  bounds coefficients to 38 digits and fractional scale to 18 places.
  Division truncates at 18 places. Explicit `round` rounds halfway
  values away from zero. The specification gives no rounding convention.

## Questions for the coordinator

- **§1.1a.** Which asset encodings and file types must the HTML/script
  classification inspect beyond textual files? The host must attest paths
  and file identity and provide content for inspection or classify opaque
  files conservatively. A path list alone cannot prove a file is data-only.


## Scope checks and remaining boundaries

- **§1.2, §2.10, §2.18, §4.6, §5.4 and §§11–12.** The active grammar
  rejects definition features deferred by the build plan. Appendix B's
  Calendar and Tasks are exercised twice, first with the expected refusal
  on their `lines:` declaration, then with line and DAV mappings stripped
  from the test input. Workouts and Reading List compile directly.
- **§3.2.** The item parser handles the single-document map, nested block
  maps and sequences, flow sequences, single and double quoted strings,
  exact scalars and literal and folded blocks. Unknown and untouched values
  and standalone comments preserve their source ranges. Multiline quoted
  and plain YAML scalars, explicit keys and complex keys are outside the
  currently accepted subset. An empty map has no representation when both
  flow mappings and tags are prohibited.
- **§4.4 and §5.5.** The implementation uses 50,000 formula steps per item
  and 1,000,000 steps per query. The host supplies per-space indexing quotas,
  manual order, authorized sources and lookup candidates. Time or repeat
  invalidation and the host's index accounting are outside the pure calls.
- **§4.3 and §4.5.** The repeat reader currently supports daily, weekly,
  monthly and yearly frequencies, intervals, day lists, month days and an
  until date. The example weekly rule works. Calendar-window expansion,
  full iCalendar clauses and the start date for interval phase need more
  vectors. The engine searches at most ten years for `next()`.
- **§2.8, thirteenth set.** The record reader ends the anchor at the first
  space. It accepts a trailing HTML comment as opaque bytes and includes it
  in the source range. Updating frontmatter preserves both tagged and
  untagged record headers. Lane B verifies the token against the ledger.
- **§4.3 and the eleventh set.** Lane B retains the ledger position of a
  removed record. The host may supply that position as the count of visible
  records at or before the marked anchor. The evaluator uses it to count
  only later records. An anchor absent from both sources counts all visible
  records as unread.
- **§4.7.** Viewer dependence is transitive. The compiler also treats link
  traversals and unknown setting scopes conservatively. Computed columns
  belonging to the collection are per-query. A viewer-dependent field
  in the shared search declaration is refused.
- **§7.3, §7.5 and §7.6.** Sign-ins, outbound secrets, outside addresses and
  after-operation listeners are refused at install in this Chat-first path.
  The fixture uses only declared destination hosts. The host owns resolved
  address checks and authority.
- **§7.4, §7.9 and the eleventh set.** Inputs live under a coded operation's
  `inputs:` block. A slot's `inputs <operation>` suffix references this
  schema. The compiler validates available filler references and refusal
  scopes. A refusal may use checked target fields and typed inputs, with
  no linked data or viewer state. The coordinator should confirm if typed
  receivers need optional extra inputs or versioned schema negotiation.

## Example coverage

| Examples | Result |
| --- | --- |
| Chat and Claude Code from the merged revision 9 vectors | Both plugin folders compile. Chat add, write, check and query work together. The harness program's ID is visible and only a bound file can be supplied for execution. |
| §2.2, §3.2, §6 and §10 standalone examples | Exact minimal kind, item, sidebar and agent rules extracted from the specification. The sidebar test supplies its referenced local collections. |
| §16 Reading List and Appendix B | Reading List and Workouts compile. Calendar and Tasks refuse the deferred `lines:` key and their in-slice forms compile. |
| Appendix A H1–H9, H11–H22 | Covered across line grammar, item, record and formula vectors. H8 and H9 exercise grammar nesting. Installation of extensions and DAV mappings remains deferred. |
| Appendix A H10 | The format crate provides provisional record spans. The host owns the ledger refusal and author projection. |
| Standalone theme and profile files | §§11–12 are outside slice 0. |

Short line examples that require a surrounding schema are exercised in
context where the tests cover them. Capability examples such as reachability,
outside writes and process dispatch require an owning host lane.

## Verification and review gate

- First pass on `claude@calternal-dev-01-compute02` through
  `~/bin/lock-shared nix develop -c cargo test -j 3 -p calternal-format`:
  four tests passed for line positions, top-level lists, names and numbers.
- Integration candidate `9415922f` passed 16 tests with default features
  and with ASCII definitions, workspace formatting, crate Clippy and the
  whole documentation gate under the dev VM gate locks. The clean checkout
  needed workspace `npm ci` for the TypeScript stage.
- The engine is pure. Its hot paths make zero reads and zero flushes.
  It compiles definitions once per load. Item frontmatter and record spans
  each use one pass through their source region. The host owns durable
  writes, uniqueness, the authoritative history ledger and access checks.
- No runtime locks, I/O primitives, clocks or random IDs are introduced.
  Lane B consumes the item, query and operation API. Agent-rule enforcement
  and writing belongs to slice 8. All exported Rust items carry rustdoc.
- Reuse reviewed at `5e113206` (engine and sizing spike) and `39016818`
  (host spike). The engine follows the spike's civil-day arithmetic and
  compile-once evaluation. The host spike's frontmatter writer supplied the
  source-range editing design. Its JSON and line-only readers do not
  represent the format's recursive item values.

- The follow-up passed 22 native tests with default features and 21 with
  ASCII definitions on `claude@calternal-dev-01-compute02`. The Unicode-only
  name vector accounts for the difference. Workspace `cargo fmt --check`,
  format crate Clippy with `-D warnings` and `scripts/check-docs.sh` passed
  under `~/bin/lock-full nix develop -c …`. The docs gate checked 438 pages.

## Wasm size

The `wasm_size` example retains the folder installer, item and record
parsers, agent-rule parser, validator, query, operation and writer. It
was built under `lock-shared` and a Nix shell using the workspace's pinned
Rust overlay and nixpkgs on the dev VM. The target was
`wasm32-unknown-unknown`, using rustc 1.99.1, Cargo opt-level `z`, link-time
optimization, one code generation unit and abort-on-panic. Binaryen 132
`wasm-opt -Oz` and `wasm-strip` preceded Brotli quality 11 compression.

| Build | Raw bytes | Brotli bytes | Change from 82 KB spike | Wasm imports |
| --- | ---: | ---: | ---: | ---: |
| Default Unicode validation | 665,125 | 212,360 | +130,360 B | 0 |
| ASCII definition validation | 340,776 | 118,684 | +36,684 B | 0 |

The spike linked its own narrower set of engine exports. The comparison
above covers this crate's example entry points and keeps compression and
optimization settings aligned. The ASCII variant accepts only ASCII
definition text.

## Simplification opportunities

- Use the operation input schema for CLI help, slot wiring and checked calls.
- Index a read anchor's visible position once per ledger snapshot. Unread
  counts can use that index without walking the transcript.
- Let the client consume compiled definitions where the host can attest
  they were checked. Measure that client path separately before deciding
  whether it should ship the full Unicode install checks.
