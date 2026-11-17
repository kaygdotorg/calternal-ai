# Calternal-AI working rules

- All UI implementation, visual design and UI/UX work MUST be done by Claude agents, never Codex/GPT agents. Non-Claude agents may implement backend/protocol/CLI logic and report UI findings, but route UI changes to Claude.
- User-approved overnight task, 2026-11-16: build the bounded roadmap in docs/tonight-contract.md after independent contract review. Use disposable development data only; no production migration, email, or unapproved external messaging.
- Maximum concurrent implementation/review subagents: two Claude Opus/Sonnet 5.5 at medium plus two GPT-6.1-Sol medium / GPT-6-Luna max. Do not spawn descendants. Reuse or archive finished lanes.
- Defer product questions to docs/morning-decisions.md. Preserve existing protections and accepted decisions; missing safety proofs mean affected features remain disabled.
- Plugins are authored in TypeScript; portable GenUI first. Rust host and shared client core; Swift native UI through UniFFI; TypeScript browser. No Wasm.
- Full product architecture is in docs/architecture-proposal.md. The overnight build is a development preview, not proof of multi-tenant sandboxing, inherited full recovery/tiering, provider credential isolation, or production readiness.
- Respect shared-host resource limits. Ordinary bounded local build/test work is authorized tonight; use conservative job counts. Dedicated performance/Apple hosts require current allocation and locks before use. Do not claim benchmark passes from development-host timings.
- Do not overwrite another lane's files. Keep operation/GenUI contracts synchronized and commit only verified coherent work.
- Foundation work ports the existing app first (user rule, 2026-11-17). The reference is `/home/kayg/Developer/calternal` at `origin/dev`: its ADRs under `docs/adr/` are accepted decisions (storage layout ADR 0062 with the tiering amendment and `docs/plans/tiering-v3.md`, invites ADR 0059, roles as capability sets ADR 0058, groups ADR 0060), and its crates (`auth`, `store`, `fsio`, `notes`, …) are the starting code. Reuse and adapt them to spaces and the plugin architecture; do not redesign what they settle, and do not ask the user a question they answer. Read that repository with git only; never modify it.
- SQLite access is `rusqlite` only; never add `sqlx`.
- Every public item carries a doc comment complete enough to generate reference documentation; a code change updates its comment in the same edit. `scripts/check-docs.sh` is the gate.
- Backend lanes run on Codex GPT-6.1-Sol medium; UI stays with Claude. Bring design questions that neither repository settles to the user instead of deciding them.
