# Fixture definition

`crates/perf/src/fixtures.rs` is the executable fixture specification. Default
seed: 20261117. Titles are ordered numeric indices; Markdown contains a heading,
tags, a task, and 10 repeated text fragments (100 every hundredth note). This
initial workload does not reproduce the reference app's large-note, attachment,
link and tier-move distribution; those remain coverage gaps. HTTP provisioning
uses the real write path and is excluded from timings. Both userdata roots are
created by the real server; the HDD root has no attachment payload yet.

Provisioning runs untimed on private tmpfs roots to avoid fixture-only disk fsync
costs. Stopped roots are copied to disk and tmpfs files removed before timed arms.
The first cold arm syncs and drops caches through the host helper.
