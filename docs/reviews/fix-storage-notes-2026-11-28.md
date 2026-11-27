# R1 storage and format lane

## Integration ownership

- S11 normalizes the host-supplied evaluation context at the format boundary.
  The declarations owner can also set nanoseconds to zero in
  `declarations::clock` before formatting RFC 3339. Ledger timestamps keep
  millisecond precision.
