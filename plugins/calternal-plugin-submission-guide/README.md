# Submit a plugin to the Calternal gallery

`calternal-plugin-submission-guide.explain` returns this guide in a space.

The gallery is at calternal.com/plugins. Submit a change to its separate public
submissions repository. Automated checks and maintainer approval come before
listing. Use the repository address and contribution instructions announced by
the gallery.

## Check your package

1. Run `calternal plugin check .` from your plugin directory. Give the README a
   heading and document every operation ID.
2. For a worker package, run `calternal plugin test .` to build and test the
   worker. Review its manifest and capture screenshots in a disposable space.
3. Run `calternal plugin review .` to see the exact worker checksum, operations,
   capabilities, broker calls and requested secret names.

Keep the package ID stable across releases. The gallery reads the ID, display
name, icon, version and API version from `manifest.json`. The same manifest
tells reviewers which kinds, operations, settings, slots and host calls the
package declares. Versions distinguish proposed updates.

## Prepare the listing

Add `gallery.json` beside the manifest. Replace the example text and source URL
with your package's details.

```json
{
  "summary": "Shows the next three checklist steps",
  "description": "Adds a read-only checklist view to an open space.",
  "author": "Example author",
  "source": "https://example.com/example/checklist-plugin",
  "license": "MIT",
  "permissions": "Reads checklist items in the active space when the view opens. No writes, secret handles or network access.",
  "screenshots": [
    { "file": "checklist.png", "caption": "Checklist view with three example steps" }
  ]
}
```

Save one to three PNG screenshots under `screenshots/`. Use plain filenames
and descriptive captions. Each image must fit within 2 MiB and 4096 pixels
per side. Use disposable data and check the screenshots for personal data,
credentials, notifications and browser tabs. The source link must be an HTTPS
repository URL without credentials, query parameters or a fragment.

The permissions statement should explain when each capability and host call
runs, what data or secrets it can access, what changes it makes, and what it
discloses to any destination. Explain why the plugin needs each permission and
what the user can deny. Do not include secret values.

## Package and validate

```sh
calternal plugin package . checklist.calsubmission
calternal plugin verify-package checklist.calsubmission --json
```

The archive is a versioned JSON envelope containing the existing installable
package, its SHA-256, listing and base64 PNG screenshots. The validator
checks the manifest, package checksum, listing and screenshot bounds and
headers offline. It does not execute the worker or contact the source host.
Use the same validator in the submissions repository and follow its instructions
for where to place the archive.

## Review checklist

- The stable ID, proposed version, source and licence match the package.
- The source includes reproducible build and test commands.
- The permission statement covers the actual manifest and user-visible effects.
- Screenshots show the proposed version with disposable data.
- The exact archive passes offline validation. The maintainer reviews the
  change and can request revisions before approval.
