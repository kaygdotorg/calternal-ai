# Generated documentation site

`npm run docs:site` installs the isolated pinned tools, generates the Markdown tree,
checks Rust documentation, builds Starlight, emits `.md` twins, and scans all textual
output for private infrastructure identifiers and internal document paths. Run inside
`nix develop` on a development VM. Stable rustdoc JSON requires unstable flags, so
Rust uses `cargo doc --workspace --no-deps -j6` HTML and generated crate indexes.
The Rust HTML pages also have converted Markdown twins at `.html.md`; index
pages additionally support `/.md`. Documentation builds disable debug symbols and
incremental compilation by default to limit disposable build storage.

`npm run docs:site -- --check` regenerates in memory (TypeDoc uses an ignored scratch
directory) and fails for any missing, obsolete or changed generated page. Commit the
Markdown alongside source changes. The site is a static snapshot; output and scratch
are disposable and replaced on each build. No runtime storage, requests or routes
are added to the Calternal server.

Environment:

- `DOCS_BASE=/docs/` defaults to the preview path; use another slash-delimited base
  when publishing elsewhere. Generate/check using the same base.
- `DOCS_SITE_URL=https://preview.calternal.cloud` sets canonical/LLM link origin;
  set `https://calternal.com` for the future deployment.

Only explicitly listed repository documents are published. Internal reviews,
measurements, open questions and AGENTS.md are excluded. Local source links become
plain labels unless their target is also published. The site retains Starlight's
default styling; the UI lane owns later appearance changes.

The CLI shape reference includes exact human/JSON golden fixtures when available.
Commands without fixtures say so and show invocation patterns without fabricated
results. Source disagreements are labelled on the affected administration pages.
Recovery documentation is not a complete backup runbook.

Deploy only the output directory, on the assigned VM:

```sh
sudo -n -u cua rsync -a --delete apps/docs/dist/ /home/cua/preview/ai-docs/
```

If rsync is unavailable, use the bounded deployment script in this directory. It
validates page parity and links, then replaces only children of `/home/cua/preview/ai-docs/` after the publication scan.

## Simplification opportunities

Add CLI fixtures for commands currently represented only by shapes. Reconcile the
machine and secrets READMEs with accepted decisions upstream so public warnings can
be removed. Keep one documentation toolchain once the existing reference-doc build
can adopt the Markdown plugin; this lane keeps its dependencies isolated by scope.
