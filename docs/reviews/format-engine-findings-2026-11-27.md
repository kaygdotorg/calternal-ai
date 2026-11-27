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
  `read through: anchor`. Lane B supplies records in ledger order with
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

- **§4.3 and the eleventh set.** A stored read anchor can disappear when
  its record is removed. The current input is a visible ordered record list.
  Should lane B pass a tombstone position for the removed anchor, or should
  a missing anchor mark the visible list unread until the next mark-read?
  A tombstone preserves the prior position and needs ledger support.
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
- **§2.8, thirteenth set.** The candidate record reader uses the revision 9
  header. Lane B owns the exact syntax and verification of its added keyed
  tag. A header alone cannot stamp a record or admit a recovered append.
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

## Verification and review gate

- First pass on `claude@calternal-dev-01-compute02` through
  `~/bin/lock-shared nix develop -c cargo test -j 3 -p calternal-format`:
  four tests passed for line positions, top-level lists, names and numbers.
- The engine is pure. Its hot paths make zero reads and zero flushes.
  The host owns durable writes, uniqueness, the authoritative history ledger
  and access checks.
- Reuse reviewed at `5e113206` (engine and sizing spike) and `39016818`
  (host spike). The engine follows the spike's civil-day arithmetic and
  compile-once evaluation. The host spike's frontmatter writer supplied the
  source-range editing design. Its JSON and line-only readers do not
  represent the format's recursive item values.

## Simplification opportunities

- Use the operation input schema for CLI help, slot wiring and checked calls.
- Keep one position in the read ledger so read counts avoid timestamp queries.
