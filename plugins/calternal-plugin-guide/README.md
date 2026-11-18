# Calternal Plugin Guide

`calternal-plugin-guide.explain` returns the Markdown reference, walkthrough
or authoring skill. `calternal-plugin-guide.scaffold` returns a validated plugin
manifest and its local authoring command. `calternal-plugin-guide.check`
checks a manifest against the SDK's declaration rules.
`calternal-plugin-guide.publish` puts the reference and walkthrough into the
current space as one Markdown item. A write uses normal request receipts.

The package worker exercises the same SDK transport and broker as other
plugins. Run `npm run build:plugins` and `npm run test:plugins` at the checkout root.
