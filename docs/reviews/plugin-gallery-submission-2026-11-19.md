# Offline gallery submission

The built-in submission guide explains the gallery fields, screenshots,
permissions statement and review checklist. The package command embeds the
existing installable package and its SHA-256 in a bounded submission envelope.
The verifier checks that artifact locally. It does not execute the worker,
resolve the source URL or submit the archive.

Dev-01 passed both guide fixtures, two Rust submission tests, the offline
CLI package and verification smoke, formatting, contract parity and 429
generated documentation checks.

The trial request for outbound title fetching needs a product decision.
The current Class B implementation sanitizes stored content and returns a
sealed frame. The registered route serves the document and performs no
outbound fetch. The public guide now documents working sealed frames and
concrete planned examples for Classes A, C and D.

## Simplification opportunities

The submission contains the existing package format so install and review
do not need a second worker manifest parser.
