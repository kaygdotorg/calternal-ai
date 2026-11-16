# Morning decisions and limitations

2026-11-16 overnight implementation. Product questions are deferred by the user's instruction.

- First real-data deployment profile (owner preview versus public cloud) remains undecided. Tonight uses disposable data and a loopback development server.
- Existing inherited tiering/FO1 guarantees are not waived. No production data migration tonight.
- First Apple app platform/reference device remains undecided. Portable core and binding scaffolding can proceed; do not claim native UI or Apple compilation without real validation.
- Credential handling/sandbox proof remains required before real harness accounts or arbitrary untrusted plugins are enabled. Prototype plugin workers are for repository-shipped trusted code only.
- Full voice, Honcho, importer, remote enrollment and PR lifecycle are later integrations, not simulated features in the preview.
