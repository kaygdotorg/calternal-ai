# calternal-perf

Standalone benchmark binary; no product crate depends on it. See
[the measurement contract](../../perf/README.md). All driver I/O is synchronous;
there is no async runtime to block. Builds and unit tests belong on dev-01.
