# Format engine API for the host · 2026-11-27

Slice 0 compiles definitions once at install. Slice 1 supplies folder entries
as text and uses the resulting definitions for item validation, queries and
operations. All inputs are in memory. The host supplies already-authorized
rows and lookup data, authoritative item metadata, the current time and the
viewer's civil date. The engine performs no reads or flushes.

```rust
pub fn parse_plugin(
    files: &[PluginFile<'_>],
    options: &InstallOptions<'_>,
) -> Result<Plugin, Diagnostic>;

pub fn parse_item(source: &str) -> Result<Item<'_>, Diagnostic>;
pub fn check_item(
    kind: &Kind, item: &Item<'_>, context: &EvalContext<'_>,
) -> Result<(), Diagnostic>;

pub fn evaluate_query(
    collection: &Collection, kind: &Kind, rows: &[Row<'_>],
    page: Page, context: &EvalContext<'_>,
) -> Result<QueryResult, Diagnostic>;

pub fn apply_operation(
    kind: &Kind, operation: &str, input: &OperationInput<'_>,
    context: &EvalContext<'_>,
) -> Result<Vec<Change>, Diagnostic>;

pub fn write_item(item: &Item<'_>, fields: &Fields) -> Result<String, Diagnostic>;
```

- `PluginFile { path: &str, text: Option<&str> }` uses plugin-relative paths.
  Code and assets can be represented by their path alone. Definition files
  require text. `InstallOptions` carries the expected plugin ID, the operator's
  trusted official names and IDs, and any installed definitions needed to check
  references. The host attests the source and the safety of traversed file
  entries.
- `Plugin` owns its manifest, kinds, collections and sidebars, indexed by ID.
  `Kind` exposes its placement, editability, fields, state, records and
  operations; `Collection` exposes its sources, filter, sort, group and views.
  The host controls access and caches these compiled definitions.
- `Diagnostic` carries an optional plugin-relative path, a one-based line and
  column, and a plain-language message. `Item` retains the original bytes and
  ranges needed to write field changes without rewriting unknown data, comments
  or the body. `Fields` maps names to typed `Value`s. An empty value in the
  write overlay removes the named field. Omitted fields keep their bytes.
- `Row` exposes borrowed field values plus host metadata separately. Query
  results refer to input row indices and can include computed fields and totals.
  `Page` carries offset and limit for in-memory evaluation. The host maps rows
  back to its existing IDs and handles transport cursors. Host-owned metadata
  takes precedence over file content.
- `EvalContext` supplies viewer state and settings, authorized links and reverse
  links, the open item, civil date and clock value. Its default supplies empty
  lookup data and no clock.
- `OperationInput` supplies the item, submitted fields and any record target.
  `Change` describes item additions, field changes, removal, duplication,
  state updates, open and timer requests, record changes and dispatch to
  declared code. The host authorizes and commits the changes together, supplies
  IDs and stamps, and enforces uniqueness and record edit windows at commit.

Signature changes will be recorded here so dependent lanes can update their
stubs and callers.

## Revision 9 implementation

The four lane B function signatures above are unchanged. The published
field and row shapes use these Rust types.

```rust
pub struct InstallOptions<'a> {
    pub expected_id: Option<&'a str>,
    pub official: &'a [(&'a str, &'a str)],
    pub installed: &'a [Plugin],
}
pub struct Row<'a> {
    pub fields: &'a Fields,
    pub metadata: &'a Fields,
}
pub struct OperationInput<'a> {
    pub item: Option<Row<'a>>,
    pub fields: &'a Fields,
    pub record: Option<&'a str>,
}
```

- `InstallOptions::official` contains operator-approved plugin ID and display
  name pairs. The caller provides this list from source authentication.
- Use `Item::fields()`, `Item::body()` and `Item::source()` for the decoded
  fields and byte slices. The host passes stamps through `Row::metadata`.
- `EvalContext::rows` keys `QueryRowContext` by the authoritative item ID.
  Supply each row's state, authorized reverse candidates and records in ledger
  order. `count(records after state.<key>)` reads the host's anchor metadata.
- A program's `path: None` describes the current harness fixture's unbound ID.
  A runnable declaration binds the ID under `programs:` to a plugin-relative
  `executable <path>` or `image <path>`.
- A coded operation may nest one `inputs:` section with typed fields beneath
  its `code` line. Its `Operation::inputs` is the same schema used to prepare
  a dispatch and identify a slot's typed payload.

`parse_records(kind, &item)` returns provisional record candidates with their
original byte ranges, declared fields and Markdown bodies. Verify every
candidate against the host ledger before presenting it as authored work.

The common context and each `QueryRowContext` may supply authorized
`outgoing_links` for the `links to open item` predicate. Slot declarations are
available under `Manifest::slots`. A suffix `inputs <operation>` references
the slot owner's operation schema, and installed fillers must match its
names, types and required fields. `Manifest::refusals` holds the compiled
conditions for a checked operation. The host applies enabled rules in plugin
ID and file order after the ordinary check, excluding protected operations.
The revision 10 handoff below gives the checked evaluator for these rules.

## Follow-up after the integration receipt

The original four lane B signatures are unchanged.

- `parse_records` accepts plain revision 9 headers and headers whose anchor
  is followed by an HTML comment. `Record::range` includes the exact header
  and comment bytes. Only the host uses the comment for ledger verification.
- `EvalContext::record_positions` and `QueryRowContext::record_positions` are
  optional maps from an anchor to the number of visible records at or before
  it in ledger order. Supply a deleted anchor's position to preserve a read
  boundary. The kind declares this state as `read through: record anchor`.
- `Field::is_per_viewer()` identifies a compiled field that requires the
  viewer's authorized query context. It includes transitive dependencies.
  The shared search index may cache fields that can be evaluated from data
  visible to every reader. The host supplies the applicable read scope.

## Revision 10 next-app additions

The original entry points keep their signatures. The host continues to supply
authorized rows, identity, clock and civil-calendar inputs. The engine owns no
storage, permissions, clocks or time zone database.

| Surface | Compiled data and host call |
| --- | --- |
| Item frontmatter | `parse_item` folds multiline plain, single-quoted and double-quoted scalars, including scalars inside flow sequences. `write_item` reuses source ranges for unchanged values and writes replacement text as quoted text with exact escapes. |
| Note items | `Kind::line_claim` carries the form, eligible source kinds and a typed checkbox map. `scan_note_lines` returns source ranges, marks, titles and the raw targets of direct child links. `LineClaim::time_fields` combines a time line with the note's date. Resolve links and the calendar tag on the host. Check any linked task status edit through the usual change path. `Source::CheckboxLines` identifies the authorized plain-checkbox source. |
| Period fields | `Field::period` identifies day, ISO week, month or year. `Period::starts` lists civil buckets. Set `EvalContext::periods` for standalone or header evaluation and `QueryRowContext::periods` for each item's query history. `evaluate_period_field` evaluates one declared field over typed stored and already-computed inputs. Use a fresh snapshot for each bucket and omit failed historical values. |
| Whole-kind totals | `Collection::named_totals` and `named_total_order` describe the header values. Supply typed, authorized `KindRows` in `EvalContext::kinds` or a `PeriodSnapshot`. Whole-kind rows are taken before collection filters and paging. Supply an empty vector for a known empty kind. Missing kind input is a diagnostic. The result separates named values and isolated diagnostics, with an empty value for a failed total. |
| Recurrence | `parse_repeat` compiles one rule. `next_occurrence` takes the original seed and an exclusive lower bound. `expand_repeat` implements an individual series and `expand_calendar_window` expands authorized pre-page matches from an expanding collection or calendar view. Supply EXDATE and authorized overrides keyed by the original slot. Occurrences carry both the original slot and effective start. Provide `EvalContext::repeat_start` or per-row `repeat_start` when the original seed is separate from stored fields. |
| DAV metadata | `Kind::dav` carries checked calendar and contact mappings. Evaluate `DavProperty::unless` before either direction. The protocol lane applies the property and payload limits, app passwords, unmapped snapshots and built-in operation checks. |
| Rights | `Rights::change_right()` and `Rights::read_audience()` give the typed narrowing and role-or-group alternatives. The host intersects applicable folder, kind and space limits, and uses the existing membership role order. |
| Refusals | Pass individually enabled `(&Manifest, &Refusal)` pairs and a `RefusalCall` to `evaluate_refusals` after ordinary validation. Supply the checked inputs, authorized stored target fields and verified own-removal flag. Map a returned reason or evaluation failure to the host's permission error. Exempt Stop, undo, own removals and leaving in host routing as well. |
| Extensions | `Manifest::extensions` preserves contributions in source order. `KindSelector::matches` supports current and future visible kinds. `KindExtension::compute_fields` returns qualified computed values, including Boolean marks. `plan_operation` emits namespaced sets, scoped open filters or dispatch plans. Use the adding plugin's identity and preserve `Kind::protected`. The host owns opt-in, disclosure, target authorization, supervised code and commits. |

The recurrence engine works in civil time. Normalize each series and its
window to the same local timeline before expansion. An explicit offset on the
seed works as a fixed offset for arithmetic and UTC UNTIL comparison.
Resolve named time zones, DST gaps and fold selection in the calendar host.
Each window is half-open by start. A consumer displaying events that overlap
the visible boundary should include earlier starts based on their duration
and clip the projected intervals.

## Integration handoff

The host query and indexing lane consumes note claims, source rows, period
history and recurrence. The host operation and authorization lane consumes
rights, refusal results and extension plans. The calendar and contacts
protocol lane consumes the checked DAV bindings. The declaration projection
lane consumes extension presentation metadata. No interface changes ship
with this crate.
