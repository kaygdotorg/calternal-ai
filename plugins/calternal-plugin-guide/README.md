# Calternal Plugin Guide

The v2 authoring material is in the [walkthrough](walkthrough.md),
[reference](reference.md), and [authoring skill](skills/plugin-authoring/SKILL.md).
The plugin's `explain` and `publish` operations deliver these documents.

The running preview worker still serves its existing v1 `scaffold` and
`check` operations. For v2, start with the walkthrough and use the SDK's
offline build command.

After editing `examples/first-list.ts`, run
`npm run docs:examples -w @calternal/plugin-guide`. The guide build checks
that the embedded example matches the source and validates its output.
