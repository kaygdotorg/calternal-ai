# Revision 10 presentation declarations · 2026-11-28

## Specification choices

| Section | Choice and reason |
| --- | --- |
| §2.7, badges | Stored badges use `number`, including units. Computed badges require a numeric result provable from the compiled expression and available field schemas. Counts, numeric arithmetic, same-row dependencies and aggregates over known numeric records are examples. General formula return types, linked-kind type inference and period fields are outside the current engine. Badges belong to the kind's row fields. `mark` and `badge` are mutually exclusive. |
| §5.1 and §6, icons | The engine validates the token shape as a lowercase ASCII ID of up to 64 bytes. The proposal names a catalogue but does not identify a format-owned list. Devices can resolve their own icon catalogue without a second list in Rust. Chat declares `archive` on the collection so every entry can use it. The projection carries the explicit collection and entry tokens separately. |
| §5.2, strips | A strip uses the same expression grammar as compact columns. The engine keeps each argument intact, including any internal commas, and the host delivers it in order. |
| §7.4, composers | A typed composer needs a nonempty local operation input schema. The text input can be `text` or `long text`. Draft state on the operation's kind must be able to hold that text, so long text can also hold a single-line draft. A quote binding can reference any one record-link input in that schema. The input declaration continues to determine its target kind and requiredness. |
| §7.4, labels | Slot and filler labels share the existing 120-character bound for slot descriptions. Quoted punctuation and escapes survive the parse. The fallback is the provider's name, with the provider ID still used for selections. The host projection keeps its existing plugin-defined slot scope. |
| §21, Chat fixtures | Chat's harness slot has no local operation declaring the exact `chat` and `prompt` pair. It gains a label and keeps its descriptive `gives`. The harness's targetless `start` now declares both inputs and sends the original link as the first actor input. The executable is owned by lane runs-harness. Until its binding lands on `dev`, the fixture keeps the flat program declaration. |

## Morning questions

- **Run input on retry.** The worker gives run creation a stable receipt key.
  The separate send call has no such receipt. Retrying a gesture can therefore
  reopen the same run receipt and submit its first input again. Keep live run
  methods unavailable in this host until the runs integration checks delivery.
  Should that integration bind the initial frame to the run-start receipt, or
  give actor send its own scoped receipt? The first option covers Chat's initial
  message with one transaction. The second covers subsequent actor frames too
  and needs a distinct delivery receipt. Prefer the second if the service will
  support retryable interactive input.
- **Quote link issuance.** The typed binding names an input. It does not resolve
  record anchors to links. Keep link issuance in the host. Should the existing
  read response include links for all readable records, or should selection ask
  for one link? A response field saves an extra round trip but adds a link to
  every row. Prefer a request for the selected record until payload and policy
  requirements for bulk issuance are known.

## Review gate

- Reused the format engine's line scanner, expression parser, dependency order,
  field types and operation schema checks. Source positions now use the same
  physical-line mapping for continued clauses and formulas.
- No new crate, clock, storage, lock, blocking I/O, authority check or grant.
  Presentation metadata lives in the in-memory installed catalog and uses the
  bundle's existing admission bound. The owner projection remains cached.
  Catalog projection adds zero file reads and zero flushes. It reads compiled
  presentation definitions and retains the existing permission filters.
- Existing string and directive fields on the public engine API remain
  available. The host uses the typed views, page components, sidebar references,
  slot bindings and fillers. The TypeScript declarations have no renderer
  changes.
- The existing workspace review blockers and run-service integration gate
  remain tracked in their own records. This lane verifies presentation
  parsing and delivery, plus the worker's first-input callback in a mock.

## Simplification opportunities

- Once all engine consumers have migrated, derive source-facing views and page
  entries from the compiled form instead of storing parallel lists.
- Generalize the engine's formula return-type analysis when more features need
  it, so badges and other constraints can share one inference pass.
- Generate the browser declarations types from the host wire contract to keep
  optionality and view names aligned.
