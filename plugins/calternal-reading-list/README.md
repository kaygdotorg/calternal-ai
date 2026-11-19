# Reading List

An offline Plugin API v2 authoring example. From the repository root, build
the SDK with `npm run build -w @calternal/plugin-sdk`, then run
`npm test -w @calternal/reading-list`. The package uses only SDK builders.
The resulting manifest and template are in `dist/`.

The two-field Add form declares a worker operation because it uses supplied
title and URL values. Mark read uses the host metadata patch with a first
full swipe. The footer searches the same searchable kind. New blank item uses
generic host creation with the kind's declared `Untitled` default and
opens the host's created reference.

The reminder example is for the person's own space. Its selected item is a
host-resolved target. The declared due operation is eligible for a one-date
schedule. A future handler for a shared item must save the reminder in the
person's own space and use an authorized reference back to that item. The
person filter shapes the list and grants no access.

The SDK emits real asset digests, list and search window defaults, loading
and error messages, and a search empty message. The sealed description
supplies the custom empty messages and the window override for reminders.
The bundled worker fails closed when launched. This package verifies the
authoring surface without executing any operation or proving reminder
placement, scheduling, or delivery.
