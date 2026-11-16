# Calternal-AI working rules

- All UI implementation, visual design and UI/UX work MUST be done by Claude agents, never Codex/GPT agents. Non-Claude agents may implement backend/protocol/CLI logic and report UI findings, but route UI changes to Claude.
- User-approved overnight task, 2026-11-16: build the bounded roadmap in docs/tonight-contract.md after independent contract review. Use disposable development data only; no production migration, email, or unapproved external messaging.
- Maximum concurrent implementation/review subagents: two Claude Opus/Sonnet 5.5 at medium plus two GPT-6.1-Sol medium / GPT-6-Luna max. Do not spawn descendants. Reuse or archive finished lanes.
- Defer product questions to docs/morning-decisions.md. Preserve existing protections and accepted decisions; missing safety proofs mean affected features remain disabled.
- Plugins are authored in TypeScript; portable GenUI first. Rust host and shared client core; Swift native UI through UniFFI; TypeScript browser. No Wasm.
- Full product architecture is in docs/architecture-proposal.md. The overnight build is a development preview, not proof of multi-tenant sandboxing, inherited full recovery/tiering, provider credential isolation, or production readiness.
- Respect shared-host resource limits. Ordinary bounded local build/test work is authorized tonight; use conservative job counts. Dedicated performance/Apple hosts require current allocation and locks before use. Do not claim benchmark passes from development-host timings.
- Do not overwrite another lane's files. Keep operation/GenUI contracts synchronized and commit only verified coherent work.
