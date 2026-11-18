# calternal-perf

Standalone benchmark binary; no product crate depends on it. See
[the measurement contract](../../perf/README.md). All driver I/O is synchronous;
there is no async runtime to block. Builds and unit tests belong on dev-01.

The driver also provides a separate `--fsync-probe` diagnostic, read-only startup
writer-count evidence, post-acknowledgement file-durability observations, and
collection of debug test-plugin stage markers. The focused release delivery
gate uses nanosecond observations from an opt-in server build and keeps the
plain release matrix for the full round trip. See [the harness guide](../../perf/README.md).
