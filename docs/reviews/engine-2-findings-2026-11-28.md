# Format lane A2 · 2026-11-28

Revision 10 support for the apps after Chat. The issue is
[Forgejo #122](https://git.kayg.org/kayg/calternal-ai/issues/122), and the
host entry points are in the [format API](format-engine-api-2026-11-27.md).
The baseline was `c59c9858`.

## Vectors

- Item frontmatter vectors cover CRLF, multiline plain and quoted values,
  escaped joins, empty physical lines, flow and block sequences, inline
  comments, exact body bytes and writing replacement text without value loss.
- Appendix B Calendar, Tasks and Workouts compile directly from the
  specification. Calendar checks typed DAV mappings, cross-midnight time
  lines, all-day values, EXDATE, moved slots and both expanding view forms.
  Tasks checks its four-way checkbox mapping and the state mirrored by
  uppercase X.
- The scanner passes all 70 inherited line vectors from the old app's
  `docs/plans/0067-vectors/lines.json`, plus the Gym example's standalone
  child links and ordered nesting.
- The §5.4 Money formula covers a carry-forward value, filtered whole-kind
  sums, historical values and a separate collection aggregate. A cap vector
  checks 1,000 accepted matches after 1,001 exclusions and isolates a sibling
  total over the cap.
- The §7.9 Money payment approval vector checks typed Money comparison, input
  precedence, plugin order, protected routes and an author's own removal.
- §16 Reading List still compiles. The §7.2 extensions check exact and dynamic
  selectors, sidebar and Home entries, local field precedence, qualified
  writes, Boolean marks and all three permitted action forms.
- Contacts exercises vCard mappings. The same fixture checks that a second
  claim for one notation and source kind refuses installation.

## Decisions and ambiguities

- **§3.2 scalar syntax.** Physical continuations fold the same way inside
  supported flow lists and block mappings. An untouched field retains its
  physical lines. Replacing a field preserves its closing inline comment
  and surrounding standalone comments. The encoding of a replacement is a
  quoted scalar with explicit escapes. Comments inside a replaced compound
  value belong to that value's source range.
- **§2.10 source resolution.** Bracketed qualifiers are normalized to IDs.
  Absent source kinds can bind later, which lets Appendix B compile on its
  own. Known hidden sources and duplicate claims are refused. Checkboxes
  support the old scanner's custom glyphs for detection. Linked status
  mappings accept the four declared revision 10 marks, with X folded into
  x. Plain boxes can still be surfaced without a linked item. The scanner
  returns both shapes for a timed box, preserving the inherited classifier
  vectors. Which shape becomes a durable item is the host's decision.
- **§2.10 line metadata.** Only direct standalone child links attach to the
  parent. Inline links remain prose. Time field projection uses the note's
  date, with a lower end clock carrying forward to the following civil day.
  A missing end stays absent. Time lines require `when` start and end
  fields to represent all-day and timed values in one kind. The scanner
  preserves title Markdown so the host can resolve tags, user-visible link
  labels and stored calendar identity. Limits are 32 MiB, 65,536 physical
  lines and 1,000 recognized items per note.
- **§§4.3–4.6 periodic history.** A field's period picks the current civil
  label. `previous()` needs an explicit `per` unit and sees the adjacent
  period's authorized fields and candidates. A missing bucket or a historical
  list over its cap is empty. The host owns oldest-item and edit invalidation,
  background recomputation and the current space date. Weeks are ISO weeks.
  When one snapshot is over the cap, other periods and header totals remain
  available. The Money prose mentions a kind-level total and §5.4 gives the
  collection grammar. Named totals use the collection grammar.
- **§4.4 candidate budgeting.** `all()` needs a visible installed kind.
  Foreign kinds require an explicit `reads:` declaration. The host supplies
  typed, authorized rows before evaluating the formula. The engine tests at
  most 10,000 supplied candidates and counts the 1,000 accepted matches
  after the formula's own condition. Indexed prefiltering may reduce the
  host's candidate input.
- **§4.5 recurrence rules.** Civil expansion implements daily, weekly,
  monthly, yearly and timed subdaily frequencies, the standard BY selectors
  and positions, week starts, count and until with a ten-year `next()` bound.
  The seed is the first slot even if it differs from an explicit BY filter.
  Invalid month days are skipped. When a scheduled formula has no supplied
  seed, it uses the item's start or scheduled field, then its lower bound.
  Completion mode phases from the supplied completion. Occurrence windows
  are half-open and sorted by effective start, with stable input order for
  ties. An authorized override wins over an EXDATE of its original slot.
  Civil seconds are 0–59, consistent with the crate's existing clock.
- **§§2.6, 7.9 authorization.** Rights store the original parsed vocabulary.
  Typed read entries classify the four reserved membership role names as
  roles and other IDs as groups. Refusals use a shared item-sized budget,
  ordered by plugin ID and then physical file position. Stored inputs are
  considered only where the resolved operation accepts them. A rule on a
  coded operation needs its typed inputs. Own removal requires a host-proven
  author fact and bypasses the rule before evaluation.
- **§7.2 extension matching.** Named targets bind exact visible installed
  kinds. Dynamic targets match the exact declared stored type and are tested
  again against newly available kinds. Local declared names shadow owner
  names inside the block, and public storage keys are flat dotted names.
  Named extensions may append unprotected owner fields. Dynamic extensions
  name only their added fields in presentation, keeping their projections
  valid across different matching schemas. The host preserves its protected
  placements. Coded extension operations receive only declared typed inputs.

## Morning questions

- **DAV allowlist and mapping direction.** Appendix B names a subset of
  the permitted properties. The compiler accepts explicit known calendar
  and contact fields, disallows ATTACH and URL and leaves DISPLAY-only
  alarm validation and the 64 KiB wire cap to the adapter. Should connected
  accounts be allowed to map address-bearing properties by default? For a
  shared birthday event, a mapped ATTENDEE could expose contact addresses.
  Keep the property opt-in and enforce rights at both directions before
  widening the mappings.
- **Named zones and unusual RRULEs.** A parsed fixed offset can compare UTC
  UNTIL deterministically, and an application must supply named-zone
  resolution for historical and future DST changes. For a weekly 09:00 event
  over a clock change, this decides whether the event stays at 09:00
  locally. Carry the wall clock and zone separately through the host, and
  keep leap seconds and opaque imported extensions out of writable
  projections until their provenance and round-trip behavior are specified.
- **Refusal data access.** The stricter implementation allows checked direct
  target field and input names, excluding arbitrary data members, links,
  viewer state, ambient clocks and broad queries. A payment rule can check
  amount and currency, while one that wants a linked account's approval
  flag needs a typed input instead. Keeping that boundary avoids making
  another plugin's denial a lookup oracle. Confirm before permitting member
  access to structured data or untyped coded calls.
- **Open from an extension.** A normal owner operation can combine
  `open where` and create-on-miss for a unique kind. An extension's open
  plan only describes the filter, so opening today's note without a match
  cannot silently write the owner's kind. Allow create-on-miss only through
  the owner's authorized add operation if that interaction is needed.
- **Folder and kind audiences.** Typed declarations expose the right inputs
  for the existing host membership check. The host must intersect the file
  path and kind policies across app, index, DAV and sync before enabling
  shared readers. For example, a manager allowed by a kind still needs the
  folder's audience. Keep role words reserved in group naming or treat
  them as roles in this clause until the two vocabularies can be
  disambiguated.

## Review gate

The crate introduces no dependencies, locks, storage, capabilities,
external access or random IDs. The hot paths perform zero reads and zero
flushes. Definitions and formula syntax compile during install, and the
host can retain checked recurrence rules for a view. Refusal and extension
plans reuse the ordinary field coercion and operation value preparation.
Civil arithmetic reuses the existing dates module, source ranges reuse the
item writer, and Sidebar extensions use the same entry parser as the base
sidebar. The host retains identity, authorize-first projections, ledger
provenance and all durable commits. The API lists the consumers for the new
entry points.

## Verification

All builds ran on `claude@calternal-dev-01-compute02` in
`~/Developer/lanes/engine-2`. The login has no `rsync` on PATH. Use the
pushed branch to identify the source and stream it into an empty directory.

```sh
set -e -o pipefail
commit=$(git ls-remote origin refs/heads/lane/engine-2 | cut -f1)
label=final-${commit:0:8}
git archive "$commit" | ssh claude@calternal-dev-01-compute02 \
  "cd ~/Developer/lanes/engine-2 && mkdir $label && tar -x -C $label"
ssh claude@calternal-dev-01-compute02 \
  "cd ~/Developer/lanes/engine-2/$label && ~/bin/lock-shared nix develop -c bash -euc '
    export CARGO_TARGET_DIR=\"\$PWD/target\" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
    npm ci --no-audit --no-fund
    cargo fmt --check
    cargo build --locked -j 3 -p calternal-format
    cargo test --locked -j 3 -p calternal-format
    cargo test --locked -j 3 -p calternal-format --no-default-features
    cargo clippy --locked -j 3 -p calternal-format --all-targets -- -D warnings
    scripts/check-docs.sh
  '"
```

The first workspace documentation attempt exhausted the shared VM disk.
Only this lane's generated output was removed before retrying. The retry
shares a Cargo target directory between the workspace and conformance stages
and disables debug information and incremental artifacts. The clean-export
receipt is on [Forgejo #122](https://git.kayg.org/kayg/calternal-ai/issues/122).

### Wasm size

Both `c59c9858` and the final code commit `49f734ac` were exported into
empty directories. The earlier review's unchanged `wasm_size` example
makes this a like-for-like comparison. The example retains its prior
entry points and any new logic reachable through them. It does not retain
standalone APIs unless an existing entry point reaches them.

| Definition validation | Baseline Wasm | A2 Wasm | Baseline Brotli | A2 Brotli | Brotli change |
| --- | ---: | ---: | ---: | ---: | ---: |
| Unicode | 692,801 B | 769,149 B | 218,675 B | 244,995 B | +26,320 B |
| ASCII | 368,683 B | 444,915 B | 126,687 B | 151,764 B | +25,077 B |

Wasm byte counts follow optimization and stripping. Both builds had zero
imports as checked with `wasm-objdump -h`. The shell used the repository's
pinned nixpkgs and Rust overlay with the `wasm32-unknown-unknown` target
added to the toolchain in `rust-toolchain.toml`. The versions were rustc
1.99.1 and Binaryen 132. Run the following inside that shell through
`~/bin/lock-shared nix develop --impure --expr … -c bash`, repeating the
commands with `--no-default-features` for ASCII definitions.

```sh
export CARGO_PROFILE_RELEASE_OPT_LEVEL=z CARGO_PROFILE_RELEASE_LTO=true
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_PANIC=abort
cargo build -j 3 -p calternal-format --release --target wasm32-unknown-unknown --example wasm_size
wasm=target/wasm32-unknown-unknown/release/examples/wasm_size.wasm
wasm-opt -Oz "$wasm" -o variant.opt.wasm
wasm-strip variant.opt.wasm
brotli -q 11 -f variant.opt.wasm
wc -c "$wasm" variant.opt.wasm variant.opt.wasm.br
wasm-objdump -h variant.opt.wasm
```

## Simplification opportunities

- Use the typed host's one authorized-row index for `all()`, `from()` and
  period history. Reject candidates before materializing result maps.
- Store a series start and original occurrence key once and let both views
  and CalDAV use the same window projection.
- Route both ordinary and extension field writes through one host permission
  and commit path, with the adding plugin's namespace as a parameter.
- Once host consumers use typed audience getters, consolidate role spelling
  at the shared membership boundary.
