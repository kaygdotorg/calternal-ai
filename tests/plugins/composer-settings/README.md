# Composer settings example

Target a composer's `composer_row` with a generated view. The space's Manager
can edit the `label` setting. The worker reads only its own setting through
`read_setting_v1`. Use `npm run build:plugins` at the repository root to build
the example and `npm run test:plugins` to test it with the SDK fixture host.

`composer-settings.label` reads the current label and returns a badge. It
requires `space.read`, has read risk, and has no durable effects. Set the
host's `plugins.settings.set` value to null to restore the default.
