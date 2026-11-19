# Calternal Plugin Guide

`calternal-plugin-guide.explain` returns the Markdown reference, walkthrough,
authoring skill, or the jobs and waits sections. `calternal-plugin-guide.scaffold` returns a validated plugin
manifest and its local authoring command. `calternal-plugin-guide.check`
checks a manifest against the SDK's declaration rules.
`calternal-plugin-guide.publish` puts the reference and walkthrough into the
current space as one Markdown item. A write uses normal request receipts.

The package worker exercises the same SDK transport and broker as other
plugins. Run `npm run build:plugins` and `npm run test:plugins` at the checkout root.

After editing a scaffold source, run
`npm run docs:examples --workspace @calternal/plugin-guide` to update the
checked-in guide examples. The build checks that they match their sources.
