---
name: plugin-authoring
description: Author and validate a Calternal Plugin API v2 package with the TypeScript SDK.
---

Read the [walkthrough](../../walkthrough.md) and [API reference](../../reference.md).
Use the first-list example as a starting point, and build it before adding
forms or actions. Keep one Bind per query and reference its ID from the view
and chrome.

Use the manifest to declare each kind, target, capability and worker method
the package needs. Keep network disabled unless a feature needs a declared
route. Use host operations for effects the catalogue already supplies.

Run the TypeScript check and the offline package build. Read the manifest
and its app templates to confirm that the emitted declarations express the
requested behavior. Compare advanced declarations with the Reading List
example. Record any feature requiring runtime verification, including effect
commit, refusal, retry, authorization, scheduling and job resume.
