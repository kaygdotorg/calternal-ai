---
title: "Fonts and account appearance"
---

<!-- Generated from: docs/fonts.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Signed-in accounts can use the same-origin font library. The browser loads
shipped typefaces and faces from the Calternal origin.

## Appearance preferences

Use `appearance.get` with empty input and `appearance.patch` with the
fields to change. Both return the full preference object in the operation
reply's `data`. The account comes from the session. Preference writes merge
only supplied fields, including independent keys in `fonts` and `materials`.
Explicit null clears `wallpaperId`, `themeId` and `presetId`. The browser
can keep an account-keyed copy for first paint and update it after the
server responds. Existing browser-local account values require a
client-initiated import by a signed-in person.

| Field | Type and meaning |
| --- | --- |
| `wallpaperId` | Library ID, `"auto"` for the default, or null for no wallpaper |
| `materials` | `blur`, `dim` and `tint` booleans |
| `themeId`, `presetId` | Preset ID or null |
| `accent` | `"wallpaper"`, `"custom"` or `"theme"` |
| `accentColor` | Custom color in `#rrggbb` form |
| `brand` | Wordmark text, at most 40 characters |
| `streamText` | Boolean |
| `fonts` | `ui`, `chat`, `code`, `terminal` and `brand`. Each is a `{family, source, id?}` object |
| `motion` | Boolean |

A downloaded or uploaded choice must carry the ID from the library along with
its matching display family and source. Use `{family:"System",source:"system"}`
for device typefaces. Shipped typefaces also omit the ID. The defaults are
Google Sans Flex for interface and chat, Cascadia Code for code and terminal,
and Dancing Script for the wordmark. Authorization binds each upload to
the account that owns it, even if an upload shares a display name with a
catalogue family.

### Device appearance

The shell overlays `DeviceAppearancePreferences` from this installation's
local store onto the cached account snapshot. This follows ADR 0055's
device scope, as with the device layer for shortcuts. Device values stay
on that installation. `appearance.patch` accepts only account fields.

| Device field | Type | Default |
| --- | --- | --- |
| `scheme` | `"system"`, `"light"` or `"dark"` | `"system"` (automatic) |
| `uiSize` | Integer, 80–150 percent | 100 |
| `appOrder` | App kinds in switcher order | Empty, use standard order |
| `chatWidth` | `"wide"` or `"centered"` | `"wide"` |
| `clarity` | Number from 0 to 1, or null to follow the device | null |

Wallpaper selection, materials, palette, accent, preset, font roles,
wordmark text, motion preference and text streaming belong to the account.

## Operations

The JSON operations below use `POST /api/v1/operations`.

- `fonts.catalogue` takes optional `query`, `source`, `mono`, `cursor`
  and `limit`. The limit defaults to 30 and accepts 1–100. The result is
  `{families, next_cursor, partial}`. The server merges duplicate names with
  Google, Fontsource and Fontshare precedence, refreshes a source daily,
  retries a failed source after five minutes and retains its last good
  entries in memory. A source failure sets `partial`. Restart paging if the
  snapshot has changed. Family rows contain `id`, `family`, `category`,
  `source` and `mono`.
- `fonts.install` takes a catalogue `id` and returns when a shared fill
  publishes the family. Concurrent requests for that ID join the fill.
  `fonts.status` takes the same ID and reads local state. Both return
  `{state, downloads_enabled}`. The states are `missing`, `installing` and
  `ready`. New attempts are limited to 12 per account and 48 per installation
  in a rolling hour, with at most two active fills. Successful warm lookups
  are free. Rates reset on restart.
- `fonts.faces` takes an optional local `id` to preview. With empty
  input it returns the signed-in account's selected families. The result
  is `{css, families}`, where `families` maps each ID to its CSS family name.
  Use that CSS alias in the stack, and preserve the URL as returned.
- `fonts.list` accepts optional `cursor` and `limit` with the catalogue
  page size bounds. It returns `{families, next_cursor}`, including shared
  installs and the caller's uploads. Each row contains `id`, `family`,
  `source`, `bytes` and `used_by` (the caller's roles).
- `fonts.remove` takes an uploaded family `id`. It deletes every face in
  the caller's family and resets roles using it to their defaults in one
  transaction. Its result is `{appearance}`.

Use `POST /api/v1/fonts/upload` with multipart form data. Send one UTF-8
`family` and 1–12 `file` fields. The family is 1–60 characters after
trimming, starts with a letter or number, and otherwise admits letters,
numbers, spaces, dots, underscores and hyphens. The server reserves
`System`. Filenames supply weight and italic hints. Files are checked by
signature and kept in their original WOFF2, WOFF, TTF or OTF format. Limits
are 8 MiB per file, 12 files per family and 50 MiB per person. The route
returns the same family object as a `fonts.list` row.

`GET /api/v1/fonts/face/{id}/{digest}` and HEAD require sign-in. Shared
faces are immutable WOFF2 with private browser caching and a digest ETag.
Uploaded faces use their original MIME type and `no-store`. Ownership and
session checks apply to conditional requests.

## Storage and configuration

`CALTERNAL_FONT_DOWNLOADS=off` disables all outbound fonts traffic. Local
installs, uploads and saved settings remain available. Set it in the
server's startup environment. Fonts and wallpapers share the bounded
response reader, HTTPS URL validation and public-DNS client, rolling request
budget, and atomic cache I/O. The font allow-list groups destinations by provider.

| Provider | Catalogue and metadata | Face bytes |
| --- | --- | --- |
| Google Fonts | `fonts.google.com`, `fonts.googleapis.com` | `fonts.gstatic.com` |
| Fontsource | `api.fontsource.org` | `cdn.jsdelivr.net` (Fontsource package paths) |
| Fontshare | `api.fontshare.com` | `cdn.fontshare.com` |

The browser receives generated CSS and same-origin URLs. Downloads prefer
variable faces, include all subsets supplied for the selected faces and
preserve provider Unicode ranges. Providers that deliver a single
unpartitioned face need no subset range.

The 500 MiB shared cache admits at most 32 MiB per family. The publisher
uses least-recently-used ordering for families unreferenced by any account's
roles, and refuses installs when the selected families fill the available
space. Install timestamp and ID determine ordering among families untouched
since restart. The cache also caps the number of families at 4,096. Account preferences, private upload bytes, pins and shared
manifests live in SQLite. Shared face files are under appdata
`fonts-cache-v1/`. The upgrade inventory
describes these stores and account erasure.
