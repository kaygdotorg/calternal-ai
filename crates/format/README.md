# Calternal format engine

This crate compiles plugin definitions and evaluates them against in-memory
items on native and wasm32 targets. The host supplies authenticated source
policy, permitted rows and link targets, ledger metadata, viewer state and
time. The engine makes zero reads and zero flushes.

Use `parse_plugin` once per plugin definition set. Use `parse_item` to get a
byte-preserving item view, `check_item` to validate it against a kind, and
`write_item` to edit named fields. `parse_records` yields candidate record
spans for the host to verify against its ledger. The record header text
carries no authority.

`evaluate_query` takes authorized rows for one kind. `apply_operation`
returns logical changes for the host to authorize and commit as a unit.
The host resolves paths, allocates IDs, enforces retention and uniqueness,
and stamps all record writes. The compiled declarations are reusable.

Full Unicode definition validation is enabled by default. The build with
`--no-default-features` accepts ASCII definition text and uses the same
in-memory item and expression interfaces.
